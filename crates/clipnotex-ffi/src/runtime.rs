//! Shared Tokio runtime — single multi-thread runtime created in `cnx_init`,
//! reused for all async operations (capture loop, paste, etc).

use once_cell::sync::OnceCell;
use tokio::runtime::{Builder, Runtime};

static RUNTIME: OnceCell<Runtime> = OnceCell::new();

pub(crate) fn init() -> Result<(), String> {
    if RUNTIME.get().is_some() {
        return Ok(());
    }
    let rt = Builder::new_multi_thread()
        .enable_all()
        .thread_name("cnx-rt")
        .build()
        .map_err(|e| format!("tokio runtime build: {e}"))?;
    RUNTIME
        .set(rt)
        .map_err(|_| "runtime already set".to_string())?;
    Ok(())
}

pub(crate) fn rt() -> &'static Runtime {
    RUNTIME
        .get()
        .expect("ClipNoteX FFI not initialized — call cnx_init() first")
}
