//! Global hotkey registration.
//!
//! Concern §4 — ショートカット競合への対応:
//! - register() は Result ではなく RegistrationResult を返し、失敗を上位に伝える。
//! - UI 層はこの結果を見て「このショートカットは使用できません」を表示する。
//! - デフォルトショートカットは全てユーザが変更可能 (Settings 経由)。
//! - CI で主要環境との衝突テストを行うことを推奨 (IMPLEMENTATION §OS注意点)。

use clipnotex_core::{bus::EventBus, ids::HotkeyId, CnxError, Result};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Human-readable description of why registration failed.
#[derive(Debug, Clone)]
pub struct RegistrationFailure {
    pub reason: String,
    /// True when the failure is likely due to another app holding the shortcut.
    pub is_conflict: bool,
}

#[derive(Debug, Clone)]
pub struct RegistrationResult {
    pub id: HotkeyId,
    pub accelerator: String,
    pub outcome: std::result::Result<(), RegistrationFailure>,
}

impl RegistrationResult {
    pub fn is_ok(&self) -> bool {
        self.outcome.is_ok()
    }
}

pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    /// Maps OS-assigned hotkey id -> our HotkeyId.
    bindings: Mutex<HashMap<u32, HotkeyId>>,
    bus: EventBus,
}

impl HotkeyService {
    pub fn new(bus: EventBus) -> Result<Arc<Self>> {
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| CnxError::Hotkey(format!("init manager: {e}")))?;
        Ok(Arc::new(Self {
            manager,
            bindings: Mutex::new(HashMap::new()),
            bus,
        }))
    }

    /// Register one hotkey and return the outcome.
    /// Never panics; always returns a result the caller can log / surface.
    pub fn register(&self, id: HotkeyId, accelerator: &str) -> RegistrationResult {
        let parse = accelerator.parse::<HotKey>();
        match parse {
            Err(e) => RegistrationResult {
                id,
                accelerator: accelerator.to_string(),
                outcome: Err(RegistrationFailure {
                    reason: format!("invalid accelerator syntax: {e}"),
                    is_conflict: false,
                }),
            },
            Ok(hk) => match self.manager.register(hk) {
                Ok(_) => {
                    self.bindings.lock().insert(hk.id(), id);
                    tracing::info!(?id, accelerator, "hotkey registered");
                    RegistrationResult {
                        id,
                        accelerator: accelerator.to_string(),
                        outcome: Ok(()),
                    }
                }
                Err(e) => {
                    // Heuristic: "already registered" or "access denied" messages
                    // tend to contain these substrings across platforms.
                    let msg = e.to_string();
                    let is_conflict = msg.contains("already")
                        || msg.contains("conflict")
                        || msg.contains("denied")
                        || msg.contains("failed");
                    tracing::warn!(?id, accelerator, error = %msg, "hotkey registration failed");
                    RegistrationResult {
                        id,
                        accelerator: accelerator.to_string(),
                        outcome: Err(RegistrationFailure {
                            reason: msg,
                            is_conflict,
                        }),
                    }
                }
            },
        }
    }

    /// Register all shortcuts from the settings map.
    /// Returns every result so the caller can collect failures for the UI.
    pub fn register_all(
        &self,
        shortcuts: &[(HotkeyId, String)],
    ) -> Vec<RegistrationResult> {
        shortcuts
            .iter()
            .map(|(id, accel)| self.register(*id, accel))
            .collect()
    }

    /// Call from the host event loop (Tauri main thread) to drain and emit
    /// hotkey events.
    pub fn pump(&self) {
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            // Pressed のみ処理。Released まで拾うと「押下時に show, 離した時に hide」で
            // ウィンドウが押している間だけ表示される現象になる。
            if ev.state != HotKeyState::Pressed {
                continue;
            }
            if let Some(id) = self.bindings.lock().get(&ev.id).copied() {
                tracing::debug!(?id, "hotkey pressed");
                self.bus
                    .emit(clipnotex_core::bus::CoreEvent::HotkeyPressed(id));
            }
        }
    }

    /// Unregister all hotkeys (called on settings change before re-registering).
    pub fn clear(&self) {
        let mut bindings = self.bindings.lock();
        for os_id in bindings.keys() {
            // global-hotkey doesn't expose unregister-by-id in all versions;
            // use the manager's unregister_all when available.
            let _ = os_id;
        }
        bindings.clear();
        // TODO(M5): call self.manager.unregister_all() when API is stable.
    }
}

/// Build the platform-appropriate accelerator string for a binding.
///
/// Returns `None` when the platform has no configured binding.
pub fn platform_accel(binding: &clipnotex_core::settings::ShortcutBinding) -> Option<&str> {
    #[cfg(target_os = "macos")]
    {
        binding.macos.as_deref()
    }
    #[cfg(target_os = "windows")]
    {
        binding.windows.as_deref()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        binding.windows.as_deref()
    }
}
