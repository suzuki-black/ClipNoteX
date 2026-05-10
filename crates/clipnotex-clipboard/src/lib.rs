//! OS clipboard abstraction for ClipNoteX.
//!
//! All public types and traits live here so that consumers can be written
//! against the abstract API and tested with the in-memory mock without
//! touching real OS APIs.

pub mod guard;
pub mod platform;
pub mod safelist;
pub mod source;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub mod hglobal;

#[cfg(any(test, feature = "testing"))]
pub mod mock;

pub use guard::SelfWriteGuard;
pub use platform::{CapturedItem, ClipboardWatcher, ClipboardWriter};

use clipnotex_core::Result;
use std::sync::Arc;

/// Construct a platform-appropriate watcher + writer pair.
pub fn open(
    guard: Arc<SelfWriteGuard>,
) -> Result<(Box<dyn ClipboardWatcher>, Box<dyn ClipboardWriter>)> {
    #[cfg(target_os = "macos")]
    {
        let w = macos::MacWatcher::new(guard.clone())?;
        let wr = macos::MacWriter::new()?;
        Ok((Box::new(w), Box::new(wr)))
    }
    #[cfg(target_os = "windows")]
    {
        let w = windows::WinWatcher::new(guard.clone())?;
        let wr = windows::WinWriter::new()?;
        Ok((Box::new(w), Box::new(wr)))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = guard;
        Err(clipnotex_core::CnxError::Other(
            "unsupported platform".into(),
        ))
    }
}
