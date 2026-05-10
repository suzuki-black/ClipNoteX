//! DONE LOG — capture clipboard entries as work-diary items.
//!
//! ## Architecture
//!
//! * **`DoneEntry`** — immutable, encrypted at rest (XChaCha20-Poly1305).
//!   Created by `DoneLogStore::capture()`.
//! * **`DoneOverlay`** — mutable user annotations (note, tags, body override).
//!   Stored separately, also encrypted.  History of `EditOp`s is kept.
//! * **`DoneView`** — read-only composite returned by `list_done()` / `get_done()`.
//!
//! The store lives in its own `donelog.redb` file so it is never mixed with
//! the clipboard history database.

use chrono::{NaiveDate, NaiveTime};
use clipnotex_core::{ids::ClipId, model::SourceApp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod export;
pub mod store;

pub use store::DoneLogStore;

// ---------------------------------------------------------------------------
// DoneEntry — immutable record
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoneEntry {
    pub id: ClipId,
    /// Calendar date when the entry was captured (local time).
    pub date: NaiveDate,
    /// Wall-clock time of capture (local time).
    pub time: NaiveTime,
    /// Unix timestamp in milliseconds (UTC) — used as AEAD AAD.
    pub captured_at: i64,
    pub source_app: SourceApp,
    pub kind: ContentKind,
    /// Primary text body. For images this is an empty string.
    pub body: String,
    /// Optional screenshot / image attachment.
    pub attachment: Option<Attachment>,
}

impl DoneEntry {
    pub fn new(
        id: ClipId,
        captured_at: i64,
        source_app: SourceApp,
        kind: ContentKind,
        body: String,
        attachment: Option<Attachment>,
    ) -> Self {
        use chrono::{Local, TimeZone};
        let dt = Local
            .timestamp_millis_opt(captured_at)
            .single()
            .unwrap_or_else(|| Local::now());
        Self {
            id,
            date: dt.date_naive(),
            time: dt.time(),
            captured_at,
            source_app,
            kind,
            body,
            attachment,
        }
    }
}

// ---------------------------------------------------------------------------
// Attachment
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub path: PathBuf,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
}

// ---------------------------------------------------------------------------
// ContentKind
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ContentKind {
    Text,
    Url,
    Json,
    Code,
    Rtf,
    Html,
    Image,
}

// ---------------------------------------------------------------------------
// DoneOverlay — mutable user edits
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DoneOverlay {
    pub user_note: Option<String>,
    pub user_body: Option<String>,
    pub tags: Vec<String>,
    /// Unix ms timestamp of last edit.
    pub edited_at: Option<i64>,
    /// Full edit history.
    pub history: Vec<EditOp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EditOp {
    SetNote(String),
    SetBody(String),
    AddTag(String),
    RemoveTag(String),
}

impl DoneOverlay {
    fn touch(&mut self) {
        self.edited_at = Some(chrono::Utc::now().timestamp_millis());
    }

    pub fn set_note(&mut self, note: impl Into<String>) {
        let s = note.into();
        self.history.push(EditOp::SetNote(s.clone()));
        self.user_note = Some(s);
        self.touch();
    }

    pub fn set_body(&mut self, body: impl Into<String>) {
        let s = body.into();
        self.history.push(EditOp::SetBody(s.clone()));
        self.user_body = Some(s);
        self.touch();
    }

    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let s = tag.into();
        if !self.tags.contains(&s) {
            self.history.push(EditOp::AddTag(s.clone()));
            self.tags.push(s);
            self.touch();
        }
    }

    pub fn remove_tag(&mut self, tag: &str) {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.history.push(EditOp::RemoveTag(tag.to_string()));
            self.tags.remove(pos);
            self.touch();
        }
    }

    /// Returns `user_body` if set, otherwise `None` (caller falls back to entry.body).
    pub fn effective_body<'a>(&'a self, entry_body: &'a str) -> &'a str {
        self.user_body.as_deref().unwrap_or(entry_body)
    }
}

// ---------------------------------------------------------------------------
// DoneView — combined read model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoneView {
    pub entry: DoneEntry,
    pub overlay: DoneOverlay,
}

impl DoneView {
    pub fn new(entry: DoneEntry, overlay: DoneOverlay) -> Self {
        Self { entry, overlay }
    }

    pub fn effective_body(&self) -> &str {
        self.overlay.effective_body(&self.entry.body)
    }

    pub fn note(&self) -> Option<&str> {
        self.overlay.user_note.as_deref()
    }

    pub fn tags(&self) -> &[String] {
        &self.overlay.tags
    }
}

// ---------------------------------------------------------------------------
// CaptureRequest — passed to DoneLogStore::capture
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CaptureRequest {
    pub id: ClipId,
    pub captured_at: i64,
    pub source_app: SourceApp,
    pub kind: ContentKind,
    pub body: String,
    pub attachment: Option<Attachment>,
}
