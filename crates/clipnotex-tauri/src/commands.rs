use crate::state::AppState;
use clipnotex_core::{model::PayloadStorage, ClipId};
use clipnotex_donelog::{ContentKind, DoneView};
use clipnotex_format::{detect, FormatOptions, Language};
use clipnotex_paste::FormatRequest;
use serde::{Deserialize, Serialize};
use tauri::State;

fn parse_clip_id(s: &str) -> Result<ClipId, String> {
    let ulid = s.parse::<ulid::Ulid>().map_err(|e| format!("invalid id: {e}"))?;
    Ok(ClipId(ulid))
}

#[derive(Serialize)]
pub struct ClipItemSummary {
    pub id: String,
    pub created_at: i64,
    pub kind: String,
    pub source_app: String,
    pub preview: String,
    pub pinned: bool,
}

#[derive(Deserialize)]
pub struct PasteArgs {
    pub id: String,
    /// "normal" | "plain" | "format"
    pub mode: String,
    /// Only for mode="format": "auto" | "json" | "sql" | "markdown" | "plaintext"
    pub format_lang: Option<String>,
    /// Only for mode="format": indentation width (spaces).
    pub format_indent: Option<u8>,
}

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ClipItemSummary>, String> {
    let lim = limit.unwrap_or(50).min(200);
    let q = query.as_deref();
    let items = tokio::task::spawn_blocking({
        let store = state.store.clone();
        let q = q.map(|s| s.to_string());
        move || store.list_recent(lim, q.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let summaries = items
        .into_iter()
        .map(|item| ClipItemSummary {
            id: item.id.to_string(),
            created_at: item.created_at,
            kind: format!("{:?}", item.primary_kind),
            source_app: item.source_app.display_name.clone(),
            preview: item.text_preview.unwrap_or_default(),
            pinned: item.pinned,
        })
        .collect();
    Ok(summaries)
}

#[tauri::command]
pub async fn paste_item(
    state: State<'_, AppState>,
    window: tauri::WebviewWindow,
    args: PasteArgs,
) -> Result<(), String> {
    use clipnotex_core::model::PayloadData;
    use clipnotex_paste::PasteMode;

    tracing::info!(id = %args.id, mode = %args.mode, "paste_item: ENTRY");

    // Fetch item from store to get digest + payloads.
    let id_str = args.id.clone();
    let store = state.store.clone();
    let item = tokio::task::spawn_blocking(move || {
        // list_recent doesn't filter by id; search by iterating until found.
        // For v0.1 history is small enough this is fine.
        store
            .list_recent(200, None)
            .map(|v| v.into_iter().find(|i| i.id.to_string() == id_str))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("item {} not found", args.id))?;

    let mode = match args.mode.as_str() {
        "plain" => PasteMode::Plain,
        "format" => {
            let lang = args.format_lang.as_deref().and_then(parse_lang);
            let opts = FormatOptions {
                indent: args.format_indent,
                ..Default::default()
            };
            PasteMode::Format(FormatRequest { lang, opts })
        }
        _ => PasteMode::Normal,
    };

    // Build payloads: inline data, or fall back to text_preview.
    let payloads: Vec<PayloadData> = if item.payloads.is_empty() {
        let text = item
            .text_preview
            .ok_or("item has no payload and no preview")?;
        vec![PayloadData {
            format_id: "public.utf8-plain-text".into(),
            bytes: text.into_bytes(),
        }]
    } else {
        item.payloads
            .into_iter()
            .filter_map(|p| match p.storage {
                PayloadStorage::Inline(bytes) => Some(PayloadData {
                    format_id: p.format_id,
                    bytes,
                }),
                _ => None,
            })
            .collect()
    };

    tracing::info!(payloads_n = payloads.len(), "paste_item: hiding window");
    // Clipy 風挙動: ペースト前にウィンドウを隠す。
    let r_hide = window.hide();
    tracing::info!(?r_hide, "paste_item: window.hide returned");
    // フォーカス遷移に必要な短いウェイト
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;

    tracing::info!("paste_item: calling paste.paste()");
    let result = state.paste.paste(payloads, item.digest, mode).await;
    tracing::info!(ok = result.is_ok(), "paste_item: paste.paste() returned");
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pin_toggle(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let clip_id = parse_clip_id(&id)?;
    tokio::task::spawn_blocking({
        let store = state.store.clone();
        move || store.pin_toggle(clip_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_item(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let clip_id = parse_clip_id(&id)?;
    tokio::task::spawn_blocking({
        let store = state.store.clone();
        move || store.delete_item(clip_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Format helpers (shared by paste_item and format_preview)
// ---------------------------------------------------------------------------

/// Parse a language string from the frontend into a `Language` variant.
/// Returns `None` for "auto" or unrecognised values (triggers auto-detect).
fn parse_lang(s: &str) -> Option<Language> {
    match s.to_lowercase().as_str() {
        "json" => Some(Language::Json),
        "sql" => Some(Language::Sql),
        "markdown" | "md" => Some(Language::Markdown),
        "plaintext" | "plain" | "text" => Some(Language::PlainText),
        "html" => Some(Language::Html),
        "css" => Some(Language::Css),
        "javascript" | "js" => Some(Language::JavaScript),
        "typescript" | "ts" => Some(Language::TypeScript),
        _ => None,
    }
}

#[derive(Serialize)]
pub struct FormatPreviewResult {
    /// The formatted text.
    pub formatted: String,
    /// The language that was actually used (after auto-detection).
    pub detected_lang: String,
}

/// Format text without pasting — used by the preview UI.
/// `text` is the raw clipboard text; `lang` is "auto" or a language name.
#[tauri::command]
pub async fn format_preview(
    state: State<'_, AppState>,
    text: String,
    lang: Option<String>,
    indent: Option<u8>,
) -> Result<FormatPreviewResult, String> {
    let lang_hint = lang.as_deref().and_then(parse_lang);
    let opts = FormatOptions {
        indent,
        ..Default::default()
    };

    let paste = state.paste.clone();
    tokio::task::spawn_blocking(move || {
        paste
            .format_preview(&text, lang_hint, &opts)
            .map(|(detected, formatted)| FormatPreviewResult {
                formatted,
                detected_lang: format!("{detected:?}").to_lowercase(),
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Auto-detect the language of a text snippet.
#[tauri::command]
pub fn detect_lang(text: String) -> Option<String> {
    detect(&text).map(|l| format!("{l:?}").to_lowercase())
}

// ---------------------------------------------------------------------------
// DONE LOG commands
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct DoneViewSummary {
    pub id: String,
    pub date: String,
    pub time: String,
    pub source_app: String,
    pub kind: String,
    pub body: String,
    pub note: Option<String>,
    pub tags: Vec<String>,
}

impl From<DoneView> for DoneViewSummary {
    fn from(v: DoneView) -> Self {
        let body = v.effective_body().to_string();
        let note = v.note().map(|s| s.to_string());
        let tags = v.tags().to_vec();
        Self {
            id: v.entry.id.to_string(),
            date: v.entry.date.format("%Y-%m-%d").to_string(),
            time: v.entry.time.format("%H:%M").to_string(),
            source_app: v.entry.source_app.display_name.clone(),
            kind: format!("{:?}", v.entry.kind),
            body,
            note,
            tags,
        }
    }
}

/// Manually capture the current clipboard text as a DONE LOG entry.
#[tauri::command]
pub async fn capture_done(
    state: State<'_, AppState>,
    body: String,
) -> Result<(), String> {
    use clipnotex_core::model::SourceApp;
    use clipnotex_donelog::CaptureRequest;
    use ulid::Ulid;

    let id = ClipId(Ulid::new());
    let captured_at = chrono::Utc::now().timestamp_millis();
    let source_app = SourceApp {
        bundle_id: None,
        exe_basename: None,
        exe_path: None,
        display_name: "Manual".into(),
        window_title: None,
    };

    let donelog = state.donelog.clone();
    tokio::task::spawn_blocking(move || {
        donelog.capture(CaptureRequest {
            id,
            captured_at,
            source_app,
            kind: ContentKind::Text,
            body,
            attachment: None,
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// List recent DONE LOG entries (newest first).
#[tauri::command]
pub async fn list_done(
    state: State<'_, AppState>,
    date: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<DoneViewSummary>, String> {
    let donelog = state.donelog.clone();
    let lim = limit.unwrap_or(50).min(200);

    tokio::task::spawn_blocking(move || {
        if let Some(date_str) = date {
            let d = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| format!("invalid date '{date_str}': {e}"))?;
            donelog.list_by_date(d).map_err(|e| e.to_string())
        } else {
            donelog.list_recent(lim).map_err(|e| e.to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map(|views| views.into_iter().map(DoneViewSummary::from).collect())
}

#[derive(Deserialize)]
pub struct UpdateOverlayArgs {
    pub id: String,
    pub note: Option<String>,
    pub body: Option<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
}

/// Apply overlay edits to a DONE LOG entry.
#[tauri::command]
pub async fn update_done_overlay(
    state: State<'_, AppState>,
    args: UpdateOverlayArgs,
) -> Result<(), String> {
    use clipnotex_donelog::DoneOverlay;

    let clip_id = parse_clip_id(&args.id)?;
    let donelog = state.donelog.clone();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // Load existing overlay (or default).
        let view = donelog
            .get(clip_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("donelog entry {} not found", clip_id))?;
        let mut overlay: DoneOverlay = view.overlay.clone();

        if let Some(note) = args.note {
            overlay.set_note(note);
        }
        if let Some(body) = args.body {
            overlay.set_body(body);
        }
        for tag in args.add_tags {
            overlay.add_tag(tag);
        }
        for tag in &args.remove_tags {
            overlay.remove_tag(tag);
        }

        donelog
            .update_overlay(clip_id, &overlay)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete a DONE LOG entry and its overlay.
#[tauri::command]
pub async fn delete_done(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let clip_id = parse_clip_id(&id)?;
    let donelog = state.donelog.clone();
    tokio::task::spawn_blocking(move || donelog.delete(clip_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Export DONE LOG as Markdown for a given date (default: today).
#[tauri::command]
pub async fn export_done_markdown(
    state: State<'_, AppState>,
    date: Option<String>,
) -> Result<String, String> {
    let donelog = state.donelog.clone();

    tokio::task::spawn_blocking(move || -> Result<String, String> {
        let d = if let Some(date_str) = date {
            chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| format!("invalid date: {e}"))?
        } else {
            chrono::Local::now().date_naive()
        };

        let views = donelog.list_by_date(d).map_err(|e| e.to_string())?;
        Ok(clipnotex_format_markdown(&d, &views))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn clipnotex_format_markdown(date: &chrono::NaiveDate, views: &[DoneView]) -> String {
    clipnotex_donelog::export::to_markdown(*date, views)
}

/// Called by the search bar on the first character keystroke (DESIGN §7.2).
/// Promotes the panel from non-activating to a regular key window so that
/// the IME input method can attach to it.
#[tauri::command]
pub fn enable_input_focus(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // TODO(M9): toggle off NSNonactivatingPanelMask, call makeKeyAndOrderFront
        // via objc2-app-kit. For now, just focus the window normally.
        window.set_focus().map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}
