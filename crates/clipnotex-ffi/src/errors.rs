//! Thread-local last-error storage.
//!
//! ABI rule: every fallible FFI returns a non-zero status code.
//! Detailed error message can be fetched via [`crate::api::cnx_last_error`].

use std::cell::RefCell;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn set_last_error<S: Into<String>>(msg: S) {
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(msg.into()));
}

pub(crate) fn take_last_error() -> Option<String> {
    LAST_ERROR.with(|e| e.borrow_mut().take())
}

/// Status codes (mirrored in `ClipNoteX.h`).
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CnxStatus {
    Ok = 0,
    NotInitialized = -1,
    InvalidArgument = -2,
    StorageError = -3,
    ClipboardError = -4,
    HotkeyError = -5,
    InternalError = -99,
}
