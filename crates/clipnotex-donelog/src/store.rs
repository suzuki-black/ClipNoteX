//! Encrypted persistent storage for DONE LOG entries and overlays.
//!
//! Tables (in `donelog.redb`):
//!
//! | Table        | Key                       | Value                     |
//! |--------------|---------------------------|---------------------------|
//! | DONE_ENTRIES | `[u8; 16]` (ClipId bytes) | `Vec<u8>` (sealed entry)  |
//! | DONE_OVERLAYS| `[u8; 16]` (ClipId bytes) | `Vec<u8>` (sealed overlay)|
//! | DONE_BY_DATE | `(i32, i64, [u8;16])`     | `()`                      |
//!
//! DONE_BY_DATE key: `(year_doy as i32, captured_at_ms, id_bytes)` — allows
//! efficient forward/backward scanning by date and time within a day.

use crate::{CaptureRequest, DoneEntry, DoneOverlay, DoneView};
use clipnotex_core::{CnxError, ClipId, Result};
use clipnotex_store::{KeySource, Sealer};
use redb::{
    Database, ReadableTable, TableDefinition,
};
use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Table definitions
// ---------------------------------------------------------------------------

/// Primary entry store: ClipId bytes → sealed DoneEntry
const DONE_ENTRIES: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("done_entries_v1");

/// Overlay store: ClipId bytes → sealed DoneOverlay
const DONE_OVERLAYS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("done_overlays_v1");

/// Date index: (year_doy, captured_at_ms, id_bytes) → ()
/// year_doy = year * 1000 + day_of_year (1-indexed)
const DONE_BY_DATE: TableDefinition<(i32, i64, &[u8]), ()> =
    TableDefinition::new("done_by_date_v1");

// ---------------------------------------------------------------------------
// DoneLogStore
// ---------------------------------------------------------------------------

pub struct DoneLogStore {
    db: Arc<Database>,
    sealer: Arc<Sealer>,
}

impl DoneLogStore {
    /// Open (or create) the donelog database at `data_dir/donelog.redb`.
    pub fn open(data_dir: PathBuf, key_source: &KeySource) -> Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("donelog.redb");
        let db = Database::create(&db_path)
            .map_err(|e| CnxError::Storage(format!("donelog open db: {e}")))?;

        // Run migrations (create tables if not present).
        {
            let write = db
                .begin_write()
                .map_err(|e| CnxError::Storage(format!("donelog begin_write: {e}")))?;
            write
                .open_table(DONE_ENTRIES)
                .map_err(|e| CnxError::Storage(format!("create done_entries: {e}")))?;
            write
                .open_table(DONE_OVERLAYS)
                .map_err(|e| CnxError::Storage(format!("create done_overlays: {e}")))?;
            write
                .open_table(DONE_BY_DATE)
                .map_err(|e| CnxError::Storage(format!("create done_by_date: {e}")))?;
            write
                .commit()
                .map_err(|e| CnxError::Storage(format!("donelog commit: {e}")))?;
        }

        let keys = clipnotex_store::DataKeys::load(key_source)?;
        let sealer = Arc::new(Sealer::new(&keys.donelog));

        Ok(Self {
            db: Arc::new(db),
            sealer,
        })
    }

    // -----------------------------------------------------------------------
    // Write operations
    // -----------------------------------------------------------------------

    /// Persist a new DONE LOG entry.  Idempotent — re-capturing the same
    /// `id` is a no-op (overlay is preserved).
    pub fn capture(&self, req: CaptureRequest) -> Result<()> {
        use chrono::{Datelike, Local, TimeZone};

        let id_bytes = req.id.as_bytes();

        // Build DoneEntry.
        let entry = DoneEntry::new(
            req.id,
            req.captured_at,
            req.source_app,
            req.kind,
            req.body,
            req.attachment,
        );

        // Compute date index key.
        let dt = Local
            .timestamp_millis_opt(req.captured_at)
            .single()
            .unwrap_or_else(|| Local::now());
        let year_doy = dt.year() * 1000 + dt.ordinal() as i32;

        let aad = req.captured_at.to_be_bytes();
        let serialized = bincode::serialize(&entry)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;
        let sealed = self.sealer.seal(&serialized, &aad)?;

        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("donelog begin_write: {e}")))?;
        {
            let mut entries = write
                .open_table(DONE_ENTRIES)
                .map_err(|e| CnxError::Storage(format!("open done_entries: {e}")))?;

            // Idempotency check.
            if entries
                .get(id_bytes.as_slice())
                .map_err(|e| CnxError::Storage(format!("get done_entries: {e}")))?
                .is_some()
            {
                return Ok(());
            }

            entries
                .insert(id_bytes.as_slice(), sealed.as_slice())
                .map_err(|e| CnxError::Storage(format!("insert done_entries: {e}")))?;

            let mut by_date = write
                .open_table(DONE_BY_DATE)
                .map_err(|e| CnxError::Storage(format!("open done_by_date: {e}")))?;
            by_date
                .insert((year_doy, req.captured_at, id_bytes.as_slice()), ())
                .map_err(|e| CnxError::Storage(format!("insert done_by_date: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("donelog commit: {e}")))?;
        Ok(())
    }

    /// Update (or create) the overlay for an entry.
    pub fn update_overlay(&self, id: ClipId, overlay: &DoneOverlay) -> Result<()> {
        let id_bytes = id.as_bytes();

        // AAD = entry's captured_at — we must read it first.
        let captured_at = self.get_captured_at(id)?
            .ok_or_else(|| CnxError::Other(format!("donelog: entry {id:?} not found")))?;
        let aad = captured_at.to_be_bytes();

        let serialized = bincode::serialize(overlay)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;
        let sealed = self.sealer.seal(&serialized, &aad)?;

        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("donelog begin_write: {e}")))?;
        {
            let mut overlays = write
                .open_table(DONE_OVERLAYS)
                .map_err(|e| CnxError::Storage(format!("open done_overlays: {e}")))?;
            overlays
                .insert(id_bytes.as_slice(), sealed.as_slice())
                .map_err(|e| CnxError::Storage(format!("insert done_overlays: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("donelog commit: {e}")))?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Read operations
    // -----------------------------------------------------------------------

    /// List done entries for a specific date, newest first.
    /// `date` is a NaiveDate (local time).
    pub fn list_by_date(&self, date: chrono::NaiveDate) -> Result<Vec<DoneView>> {
        use chrono::Datelike;
        let year_doy = date.year() * 1000 + date.ordinal() as i32;

        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("donelog begin_read: {e}")))?;
        let by_date = read
            .open_table(DONE_BY_DATE)
            .map_err(|e| CnxError::Storage(format!("open done_by_date: {e}")))?;
        let entries_table = read
            .open_table(DONE_ENTRIES)
            .map_err(|e| CnxError::Storage(format!("open done_entries: {e}")))?;
        let overlays_table = read
            .open_table(DONE_OVERLAYS)
            .map_err(|e| CnxError::Storage(format!("open done_overlays: {e}")))?;

        // Scan the day's range in DONE_BY_DATE.
        let range_start = (year_doy, i64::MIN, b"".as_slice());
        let range_end = (year_doy, i64::MAX, b"".as_slice());

        let mut views: Vec<DoneView> = by_date
            .range(range_start..=range_end)
            .map_err(|e| CnxError::Storage(format!("range done_by_date: {e}")))?
            .filter_map(|kv| kv.ok())
            .filter_map(|(key, _)| {
                let (_yd, captured_at, id_bytes) = key.value();
                let aad = captured_at.to_be_bytes();

                // Load entry.
                let entry = entries_table
                    .get(id_bytes)
                    .ok()??;
                let plain = self.sealer.open(entry.value(), &aad).ok()?;
                let entry: DoneEntry = bincode::deserialize(&plain).ok()?;

                // Load overlay (default if absent).
                let overlay = overlays_table
                    .get(id_bytes)
                    .ok()
                    .flatten()
                    .and_then(|ag| {
                        let plain = self.sealer.open(ag.value(), &aad).ok()?;
                        bincode::deserialize::<DoneOverlay>(&plain).ok()
                    })
                    .unwrap_or_default();

                Some(DoneView::new(entry, overlay))
            })
            .collect();

        // Newest first.
        views.sort_by(|a, b| b.entry.captured_at.cmp(&a.entry.captured_at));
        Ok(views)
    }

    /// List recent entries across all dates, newest first.
    pub fn list_recent(&self, limit: usize) -> Result<Vec<DoneView>> {
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("donelog begin_read: {e}")))?;
        let by_date = read
            .open_table(DONE_BY_DATE)
            .map_err(|e| CnxError::Storage(format!("open done_by_date: {e}")))?;
        let entries_table = read
            .open_table(DONE_ENTRIES)
            .map_err(|e| CnxError::Storage(format!("open done_entries: {e}")))?;
        let overlays_table = read
            .open_table(DONE_OVERLAYS)
            .map_err(|e| CnxError::Storage(format!("open done_overlays: {e}")))?;

        let mut views: Vec<DoneView> = by_date
            .iter()
            .map_err(|e| CnxError::Storage(format!("iter done_by_date: {e}")))?
            .filter_map(|kv| kv.ok())
            .filter_map(|(key, _)| {
                let (_yd, captured_at, id_bytes) = key.value();
                let aad = captured_at.to_be_bytes();

                let entry_ag = entries_table.get(id_bytes).ok()??;
                let plain = self.sealer.open(entry_ag.value(), &aad).ok()?;
                let entry: DoneEntry = bincode::deserialize(&plain).ok()?;

                let overlay = overlays_table
                    .get(id_bytes)
                    .ok()
                    .flatten()
                    .and_then(|ag| {
                        let plain = self.sealer.open(ag.value(), &aad).ok()?;
                        bincode::deserialize::<DoneOverlay>(&plain).ok()
                    })
                    .unwrap_or_default();

                Some(DoneView::new(entry, overlay))
            })
            .collect();

        views.sort_by(|a, b| b.entry.captured_at.cmp(&a.entry.captured_at));
        views.truncate(limit);
        Ok(views)
    }

    /// Get a single entry by ID.
    pub fn get(&self, id: ClipId) -> Result<Option<DoneView>> {
        let id_bytes = id.as_bytes();
        let captured_at = match self.get_captured_at(id)? {
            Some(ts) => ts,
            None => return Ok(None),
        };
        let aad = captured_at.to_be_bytes();

        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("donelog begin_read: {e}")))?;
        let entries_table = read
            .open_table(DONE_ENTRIES)
            .map_err(|e| CnxError::Storage(format!("open done_entries: {e}")))?;
        let overlays_table = read
            .open_table(DONE_OVERLAYS)
            .map_err(|e| CnxError::Storage(format!("open done_overlays: {e}")))?;

        let entry_ag = match entries_table
            .get(id_bytes.as_slice())
            .map_err(|e| CnxError::Storage(format!("get done_entries: {e}")))?
        {
            Some(ag) => ag,
            None => return Ok(None),
        };
        let plain = self.sealer.open(entry_ag.value(), &aad)?;
        let entry: DoneEntry = bincode::deserialize(&plain)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;

        let overlay = overlays_table
            .get(id_bytes.as_slice())
            .ok()
            .flatten()
            .and_then(|ag| {
                let plain = self.sealer.open(ag.value(), &aad).ok()?;
                bincode::deserialize::<DoneOverlay>(&plain).ok()
            })
            .unwrap_or_default();

        Ok(Some(DoneView::new(entry, overlay)))
    }

    // -----------------------------------------------------------------------
    // Delete operation
    // -----------------------------------------------------------------------

    /// Delete a DONE LOG entry and its overlay from all three tables atomically.
    /// Returns `Ok(())` if the entry did not exist (idempotent).
    pub fn delete(&self, id: ClipId) -> Result<()> {
        let id_bytes = id.as_bytes();

        // We need captured_at to reconstruct the DONE_BY_DATE composite key.
        let captured_at = match self.get_captured_at(id)? {
            Some(ts) => ts,
            None => return Ok(()), // already absent — idempotent
        };

        use chrono::{Datelike, Local, TimeZone};
        let dt = Local
            .timestamp_millis_opt(captured_at)
            .single()
            .unwrap_or_else(|| Local::now());
        let year_doy = dt.year() * 1000 + dt.ordinal() as i32;

        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("donelog begin_write: {e}")))?;
        {
            let mut entries = write
                .open_table(DONE_ENTRIES)
                .map_err(|e| CnxError::Storage(format!("open done_entries: {e}")))?;
            entries
                .remove(id_bytes.as_slice())
                .map_err(|e| CnxError::Storage(format!("remove done_entries: {e}")))?;

            let mut overlays = write
                .open_table(DONE_OVERLAYS)
                .map_err(|e| CnxError::Storage(format!("open done_overlays: {e}")))?;
            overlays
                .remove(id_bytes.as_slice())
                .map_err(|e| CnxError::Storage(format!("remove done_overlays: {e}")))?;

            let mut by_date = write
                .open_table(DONE_BY_DATE)
                .map_err(|e| CnxError::Storage(format!("open done_by_date: {e}")))?;
            by_date
                .remove((year_doy, captured_at, id_bytes.as_slice()))
                .map_err(|e| CnxError::Storage(format!("remove done_by_date: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("donelog commit: {e}")))?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Scan DONE_ENTRIES to find captured_at for AAD reconstruction.
    /// This is a linear scan over entries (small in practice for DONE LOG).
    fn get_captured_at(&self, id: ClipId) -> Result<Option<i64>> {
        let id_bytes = id.as_bytes();
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("donelog begin_read: {e}")))?;
        let by_date = read
            .open_table(DONE_BY_DATE)
            .map_err(|e| CnxError::Storage(format!("open done_by_date: {e}")))?;

        for kv in by_date
            .iter()
            .map_err(|e| CnxError::Storage(format!("iter done_by_date: {e}")))?
            .flatten()
        {
            let (_yd, captured_at, key_id) = kv.0.value();
            if key_id == id_bytes.as_slice() {
                return Ok(Some(captured_at));
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentKind;
    use clipnotex_core::{ids::ClipId, model::SourceApp};
    use clipnotex_store::KeySource;
    use ulid::Ulid;

    fn test_store() -> (DoneLogStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = DoneLogStore::open(dir.path().to_path_buf(), &KeySource::Ephemeral).unwrap();
        (store, dir)
    }

    fn make_source() -> SourceApp {
        SourceApp {
            bundle_id: Some("com.test.app".into()),
            exe_basename: None,
            exe_path: None,
            display_name: "TestApp".into(),
            window_title: None,
        }
    }

    fn make_id() -> ClipId {
        ClipId(Ulid::new())
    }

    #[test]
    fn capture_and_retrieve() {
        let (store, _dir) = test_store();
        let id = make_id();
        let now = chrono::Utc::now().timestamp_millis();

        store
            .capture(CaptureRequest {
                id,
                captured_at: now,
                source_app: make_source(),
                kind: ContentKind::Text,
                body: "hello world".into(),
                attachment: None,
            })
            .unwrap();

        let view = store.get(id).unwrap().unwrap();
        assert_eq!(view.entry.body, "hello world");
        assert!(view.tags().is_empty());
    }

    #[test]
    fn capture_is_idempotent() {
        let (store, _dir) = test_store();
        let id = make_id();
        let now = chrono::Utc::now().timestamp_millis();
        let req = || CaptureRequest {
            id,
            captured_at: now,
            source_app: make_source(),
            kind: ContentKind::Text,
            body: "body".into(),
            attachment: None,
        };
        store.capture(req()).unwrap();
        store.capture(req()).unwrap(); // second call should be no-op

        let recent = store.list_recent(10).unwrap();
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn overlay_updates_persist() {
        let (store, _dir) = test_store();
        let id = make_id();
        let now = chrono::Utc::now().timestamp_millis();

        store
            .capture(CaptureRequest {
                id,
                captured_at: now,
                source_app: make_source(),
                kind: ContentKind::Text,
                body: "original".into(),
                attachment: None,
            })
            .unwrap();

        let mut overlay = DoneOverlay::default();
        overlay.set_note("important meeting");
        overlay.add_tag("work");
        store.update_overlay(id, &overlay).unwrap();

        let view = store.get(id).unwrap().unwrap();
        assert_eq!(view.note(), Some("important meeting"));
        assert_eq!(view.tags(), &["work"]);
        assert_eq!(view.effective_body(), "original");
    }

    #[test]
    fn list_by_date_filters_correctly() {
        let (store, _dir) = test_store();

        // Two entries on "today".
        let now = chrono::Utc::now().timestamp_millis();
        for body in ["first", "second"] {
            store
                .capture(CaptureRequest {
                    id: make_id(),
                    captured_at: now,
                    source_app: make_source(),
                    kind: ContentKind::Text,
                    body: body.into(),
                    attachment: None,
                })
                .unwrap();
        }

        let today = chrono::Local::now().date_naive();
        let views = store.list_by_date(today).unwrap();
        assert_eq!(views.len(), 2);
    }

    #[test]
    fn list_recent_newest_first() {
        let (store, _dir) = test_store();
        let base = chrono::Utc::now().timestamp_millis();

        for i in 0u64..3 {
            store
                .capture(CaptureRequest {
                    id: make_id(),
                    captured_at: base + i as i64 * 1000,
                    source_app: make_source(),
                    kind: ContentKind::Text,
                    body: format!("entry {i}"),
                    attachment: None,
                })
                .unwrap();
        }

        let views = store.list_recent(10).unwrap();
        assert_eq!(views.len(), 3);
        assert!(
            views[0].entry.captured_at >= views[1].entry.captured_at,
            "should be newest first"
        );
    }
}
