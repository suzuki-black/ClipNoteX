//! IME state detection for Stage B paste (Unicode keystroke path).
//!
//! Concern §5 — Windows で IME ON の場合、KEYEVENTF_UNICODE が変換候補に
//! 吸われてペースト結果が壊れる。
//!
//! 対応方針:
//! - Windows: ImmGetContext → ImmGetOpenStatus で IME ON/OFF を確認。
//!   IME が ON なら Stage B (Unicode keystroke) をスキップして Stage A に降格。
//! - macOS: Stage B は AXValue への直接書込なので IME の影響を受けない。
//!   ただし AXSetAttributeValue が失敗した場合も Stage A に降格。
//! - どちらの場合も IME 状態をログに残しデバッグを支援する。

/// Whether the focused input field currently has an IME active.
/// Callers use this to decide whether `Unicode keystroke` is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeState {
    On,
    Off,
    /// Cannot determine (unsupported context, non-IME locale).
    Unknown,
}

/// Query the IME state of the thread/window that currently owns input focus.
pub fn query_focused_ime() -> ImeState {
    #[cfg(target_os = "windows")]
    return win_ime_state();

    #[cfg(target_os = "macos")]
    return ImeState::Unknown; // macOS Stage B uses AXValue, not keystrokes.

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return ImeState::Unknown;
}

#[cfg(target_os = "windows")]
fn win_ime_state() -> ImeState {
    use windows::Win32::{
        Foundation::HWND,
        UI::{
            Input::Ime::{ImmGetContext, ImmGetOpenStatus, ImmReleaseContext},
            WindowsAndMessaging::GetForegroundWindow,
        },
    };

    // SAFETY: GetForegroundWindow is always safe; returns NULL when no window.
    let hwnd: HWND = unsafe { GetForegroundWindow() };
    if hwnd.0 == 0 {
        tracing::debug!("win_ime_state: no foreground window");
        return ImeState::Unknown;
    }

    // SAFETY: hwnd is valid; ImmGetContext returns NULL for non-IME windows.
    let himc = unsafe { ImmGetContext(hwnd) };
    if himc.is_invalid() {
        // Non-IME window or IME not available — Unicode keystrokes are safe.
        tracing::debug!("win_ime_state: ImmGetContext invalid → IME not active");
        return ImeState::Off;
    }

    // SAFETY: himc is valid, ImmGetOpenStatus returns 0 (false) or non-zero.
    let open = unsafe { ImmGetOpenStatus(himc) };
    // SAFETY: must be released to avoid leaking the IMC handle.
    unsafe { ImmReleaseContext(hwnd, himc) };

    let state = if open.as_bool() {
        ImeState::On
    } else {
        ImeState::Off
    };
    tracing::debug!(?state, "win_ime_state");
    state
}

/// Log the IME state for debugging (called before every Stage B attempt).
pub fn log_ime_state() -> ImeState {
    let s = query_focused_ime();
    match s {
        ImeState::On => tracing::info!("IME is ON — Stage B (Unicode keystroke) will be skipped; using Stage A"),
        ImeState::Off => tracing::debug!("IME is OFF — Stage B (Unicode keystroke) is safe"),
        ImeState::Unknown => tracing::debug!("IME state unknown — proceeding with Stage B attempt"),
    }
    s
}
