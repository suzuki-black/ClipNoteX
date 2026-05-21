//! Paste injection.
//!
//! v0.1 implements **Stage A** only (DESIGN §5.3): snapshot the OS
//! clipboard, write the chosen item, synthesize Cmd/Ctrl+V, then
//! restore the original clipboard.
//!
//! Stage B (v0.4 — direct AX/UIA injection) includes a **3-step fallback**:
//!   1. Direct inject (AXValue / IUIAutomationValuePattern::SetValue)
//!   2. Unicode keystroke (skipped when Windows IME is ON — concern §5)
//!   3. Stage A (clipboard route, always works)
//!
//! Stage C (named pasteboard / private clipboard) is a research item for v0.5+.

pub mod ime;

use clipnotex_clipboard::{ClipboardWriter, SelfWriteGuard};
use clipnotex_core::{model::PayloadData, CnxError, Result};
use clipnotex_format::{detect, FormatOptions, FormatService, Language};
use ime::{log_ime_state, ImeState};
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// PasteMode
// ---------------------------------------------------------------------------

/// Specifies how the payload should be processed before pasting.
#[derive(Debug)]
pub enum PasteMode {
    /// Write as-is (all original formats).
    Normal,
    /// Write only the plain-text representation.
    Plain,
    /// Format the text, then paste as plain text.
    Format(FormatRequest),
    /// Reserved for v0.2 (full multi-format restore).
    Full,
}

/// Parameters for `PasteMode::Format`.
#[derive(Debug, Default)]
pub struct FormatRequest {
    /// Explicit language override. `None` = auto-detect with `detect()`.
    pub lang: Option<Language>,
    /// Formatting options (indent width, line width, dialect hint).
    pub opts: FormatOptions,
}

// ---------------------------------------------------------------------------
// PasteController
// ---------------------------------------------------------------------------

pub struct PasteController {
    writer: Arc<dyn ClipboardWriter>,
    guard: Arc<SelfWriteGuard>,
    restore_delay: Duration,
    format_svc: FormatService,
}

impl PasteController {
    pub fn new(writer: Arc<dyn ClipboardWriter>, guard: Arc<SelfWriteGuard>) -> Self {
        Self {
            writer,
            guard,
            restore_delay: Duration::from_millis(150),
            format_svc: FormatService::new(),
        }
    }

    pub async fn paste(
        &self,
        payloads: Vec<PayloadData>,
        digest: [u8; 32],
        mode: PasteMode,
    ) -> Result<()> {
        match mode {
            PasteMode::Normal | PasteMode::Plain | PasteMode::Full => {
                self.paste_stage_a(payloads, digest).await
            }
            PasteMode::Format(req) => {
                let formatted = self.apply_format(&payloads, req)?;
                self.paste_stage_a(formatted, digest).await
            }
        }
    }

    // -----------------------------------------------------------------------
    // Format helper
    // -----------------------------------------------------------------------

    /// Extract text from payloads, run the formatter, return a single
    /// `public.utf8-plain-text` payload with the formatted result.
    pub fn apply_format(
        &self,
        payloads: &[PayloadData],
        req: FormatRequest,
    ) -> Result<Vec<PayloadData>> {
        let text = extract_text(payloads).ok_or_else(|| {
            CnxError::Other("format paste: no plain-text payload found".into())
        })?;

        // Determine language: explicit or auto-detected.
        let lang = if let Some(l) = req.lang {
            l
        } else {
            detect(&text).unwrap_or(Language::PlainText)
        };

        let formatted = self.format_svc.format_as(&lang, &text, &req.opts)?;

        Ok(vec![PayloadData {
            format_id: "public.utf8-plain-text".into(),
            bytes: formatted.into_bytes(),
        }])
    }

    /// Format text without pasting — used by the preview command.
    pub fn format_preview(
        &self,
        text: &str,
        lang_hint: Option<Language>,
        opts: &FormatOptions,
    ) -> Result<(Language, String)> {
        let lang = if let Some(l) = lang_hint {
            l
        } else {
            detect(text).unwrap_or(Language::PlainText)
        };
        let formatted = self.format_svc.format_as(&lang, text, opts)?;
        Ok((lang, formatted))
    }

    // -----------------------------------------------------------------------
    // Stage A
    // -----------------------------------------------------------------------

    /// Stage A: snapshot → write → synthesize Cmd/Ctrl+V → restore.
    /// Always works; may briefly expose the item to other clipboard watchers.
    async fn paste_stage_a(
        &self,
        payloads: Vec<PayloadData>,
        digest: [u8; 32],
    ) -> Result<()> {
        tracing::info!("paste_stage_a: snapshot clipboard for restore");
        let backup = self.writer.snapshot_for_restore()?;
        tracing::info!(backup_n = backup.len(), "paste_stage_a: backup ready");
        self.guard.register(digest);
        tracing::info!("paste_stage_a: writing payloads to clipboard");
        self.writer.write(&payloads)?;
        tracing::info!("paste_stage_a: synthesize keystroke (Cmd+V)");
        synthesize_paste_keystroke()?;
        tracing::info!("paste_stage_a: keystroke sent, waiting before restore");
        tokio::time::sleep(self.restore_delay).await;
        tracing::info!("paste_stage_a: restoring original clipboard");
        if let Err(e) = self.writer.write(&backup) {
            tracing::warn!(?e, "failed to restore clipboard after Stage A paste");
        }
        tracing::info!("paste_stage_a: DONE");
        Ok(())
    }

    /// Stage B (v0.4+): 3-step fallback for plain-text direct injection.
    #[allow(dead_code)]
    async fn paste_stage_b_text(
        &self,
        text: &str,
        digest: [u8; 32],
    ) -> Result<PasteAttempt> {
        if try_direct_inject(text).is_ok() {
            tracing::debug!("Stage B: direct inject succeeded");
            return Ok(PasteAttempt::DirectInject);
        }

        let ime = log_ime_state();
        if ime != ImeState::On {
            if try_unicode_keystroke(text).is_ok() {
                tracing::debug!("Stage B: unicode keystroke succeeded");
                return Ok(PasteAttempt::UnicodeKeystroke);
            }
            tracing::warn!("Stage B: unicode keystroke failed");
        } else {
            tracing::info!("Stage B: skipping unicode keystroke (IME is ON)");
        }

        tracing::info!("Stage B: falling back to Stage A (clipboard route)");
        let plain_payload = vec![PayloadData {
            format_id: "public.utf8-plain-text".into(),
            bytes: text.as_bytes().to_vec(),
        }];
        self.paste_stage_a(plain_payload, digest).await?;
        Ok(PasteAttempt::ClipboardFallback)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Try to extract a UTF-8 plain text string from a payload list.
/// Prefers `public.utf8-plain-text` / `CF_UNICODETEXT`, falls back to the
/// first payload that decodes cleanly as UTF-8.
fn extract_text(payloads: &[PayloadData]) -> Option<String> {
    const PLAIN_FORMATS: &[&str] = &[
        "public.utf8-plain-text",
        "public.plain-text",
        "CF_UNICODETEXT",
        "text/plain",
    ];

    // Priority lookup.
    for fmt in PLAIN_FORMATS {
        if let Some(p) = payloads.iter().find(|p| p.format_id.as_str() == *fmt) {
            if let Ok(s) = std::str::from_utf8(&p.bytes) {
                return Some(s.to_string());
            }
        }
    }

    // Fallback: first UTF-8-decodable payload.
    payloads
        .iter()
        .find_map(|p| std::str::from_utf8(&p.bytes).ok().map(|s| s.to_string()))
}

/// Result of a Stage B attempt (telemetry for adaptive fallback).
#[derive(Debug, Clone, Copy)]
pub enum PasteAttempt {
    DirectInject,
    UnicodeKeystroke,
    ClipboardFallback,
}

fn try_direct_inject(_text: &str) -> Result<()> {
    Err(CnxError::Paste("direct inject not yet implemented".into()))
}

fn try_unicode_keystroke(text: &str) -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut e = Enigo::new(&Settings::default())
        .map_err(|e| CnxError::Paste(format!("enigo init: {e}")))?;
    for ch in text.chars() {
        e.key(Key::Unicode(ch), Direction::Click)
            .map_err(|e| CnxError::Paste(format!("unicode keystroke '{ch}': {e}")))?;
    }
    Ok(())
}

fn synthesize_paste_keystroke() -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    tracing::info!("synthesize_paste_keystroke: creating Enigo");
    let mut e = Enigo::new(&Settings::default())
        .map_err(|e| CnxError::Paste(format!("enigo init: {e}")))?;
    tracing::info!("synthesize_paste_keystroke: Enigo ready");
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(target_os = "windows")]
    let modifier = Key::Control;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let modifier = Key::Control;

    // ⚠️ クラッシュ防止 (macOS):
    //   Key::Unicode('v') を使うと enigo 内部で `TSMGetInputSourceProperty`
    //   (現在のキーボードレイアウトから 'v' のキーコードを引く API) が呼ばれる。
    //   この API は main dispatch queue からしか呼べないため、tokio worker
    //   からのペースト呼び出しで dispatch_assert_queue_fail → SIGTRAP で死ぬ。
    //   → 物理キーコードを直接指定して TSM を経由しない経路に切り替える。
    #[cfg(target_os = "macos")]
    let v_key = Key::Other(0x09); // kVK_ANSI_V
    #[cfg(target_os = "windows")]
    let v_key = Key::Other(0x56); // VK_V
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let v_key = Key::Unicode('v');

    tracing::info!("synthesize_paste_keystroke: Cmd press");
    e.key(modifier, Direction::Press)
        .map_err(|e| CnxError::Paste(e.to_string()))?;
    tracing::info!("synthesize_paste_keystroke: V click");
    e.key(v_key, Direction::Click)
        .map_err(|e| CnxError::Paste(e.to_string()))?;
    tracing::info!("synthesize_paste_keystroke: Cmd release");
    e.key(modifier, Direction::Release)
        .map_err(|e| CnxError::Paste(e.to_string()))?;
    tracing::info!("synthesize_paste_keystroke: COMPLETE");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn plain_payload(text: &str) -> Vec<PayloadData> {
        vec![PayloadData {
            format_id: "public.utf8-plain-text".into(),
            bytes: text.as_bytes().to_vec(),
        }]
    }

    #[test]
    fn extract_text_prefers_plain() {
        let payloads = vec![
            PayloadData {
                format_id: "public.png".into(),
                bytes: vec![0x89, 0x50],
            },
            PayloadData {
                format_id: "public.utf8-plain-text".into(),
                bytes: b"hello".to_vec(),
            },
        ];
        assert_eq!(extract_text(&payloads), Some("hello".into()));
    }

    #[test]
    fn format_json_pretty() {
        // We can't easily instantiate PasteController without clipboard hardware,
        // but FormatService is pure — test it directly here as a sanity check.
        let svc = FormatService::new();
        let out = svc
            .format_as(
                &Language::Json,
                r#"{"b":2,"a":1}"#,
                &FormatOptions::default(),
            )
            .unwrap();
        assert!(out.contains("  \"a\": 1"), "got: {out}");
    }

    #[test]
    fn format_auto_detects_json() {
        let text = r#"{"key":"value"}"#;
        let lang = detect(text).unwrap();
        assert_eq!(lang, Language::Json);
    }

    #[test]
    fn format_auto_detects_sql() {
        let text = "SELECT id FROM users WHERE id = 1";
        let lang = detect(text).unwrap();
        assert_eq!(lang, Language::Sql);
    }

    #[test]
    fn format_fallback_to_plaintext() {
        let text = "just some prose";
        assert_eq!(detect(text), None);
    }

    #[test]
    fn plain_payload_extraction_fallback() {
        // Non-preferred format ID still works if it's valid UTF-8.
        let payloads = vec![PayloadData {
            format_id: "com.example.custom".into(),
            bytes: b"fallback text".to_vec(),
        }];
        assert_eq!(extract_text(&payloads), Some("fallback text".into()));
    }
}
