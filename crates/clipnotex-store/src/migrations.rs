//! Schema versioning. The current schema is v1.
//!
//! When the on-disk layout changes, bump `CURRENT_SCHEMA_VERSION` and
//! add a migration step in `apply_pending`.

use crate::tables::META;
use clipnotex_core::{CnxError, Result};
use redb::Database;

pub const CURRENT_SCHEMA_VERSION: u64 = 1;
pub const META_KEY_VERSION: &str = "schema_version";

pub fn apply_pending(db: &Database) -> Result<()> {
    let read = db
        .begin_read()
        .map_err(|e| CnxError::Storage(format!("begin_read: {e}")))?;
    let current = match read.open_table(META) {
        Ok(t) => t
            .get(META_KEY_VERSION)
            .map_err(|e| CnxError::Storage(format!("read meta: {e}")))?
            .map(|v| v.value())
            .unwrap_or(0),
        Err(_) => 0,
    };
    drop(read);

    if current == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    if current > CURRENT_SCHEMA_VERSION {
        return Err(CnxError::Storage(format!(
            "database was created by a newer ClipNoteX (v{current}); refusing to downgrade"
        )));
    }

    // v0 -> v1: nothing to migrate; just stamp the version.
    let write = db
        .begin_write()
        .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
    {
        let mut meta = write
            .open_table(META)
            .map_err(|e| CnxError::Storage(format!("open meta: {e}")))?;
        meta.insert(META_KEY_VERSION, CURRENT_SCHEMA_VERSION)
            .map_err(|e| CnxError::Storage(format!("write meta: {e}")))?;
    }
    write
        .commit()
        .map_err(|e| CnxError::Storage(format!("commit meta: {e}")))?;
    Ok(())
}
