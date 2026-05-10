use serde::{Deserialize, Serialize};

use crate::ids::ClipId;

/// A captured clipboard item, persisted to the history store.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipItem {
    pub id: ClipId,
    pub created_at: i64,
    pub updated_at: i64,
    pub source_app: SourceApp,
    pub primary_kind: ClipKind,
    pub payloads: Vec<PayloadRef>,
    /// blake3 of the primary payload, used for de-duplication.
    pub digest: [u8; 32],
    pub text_preview: Option<String>,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ClipKind {
    Text,
    Image,
    Rtf,
    Html,
    Pdf,
    Files,
    Mixed,
    Custom,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceApp {
    pub bundle_id: Option<String>,
    pub exe_basename: Option<String>,
    pub exe_path: Option<std::path::PathBuf>,
    pub display_name: String,
    pub window_title: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PayloadRef {
    /// e.g. "public.utf8-plain-text" / "CF_UNICODETEXT" / "public.png".
    pub format_id: String,
    pub compression: Compression,
    pub storage: PayloadStorage,
    pub raw_size: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Compression {
    None,
    Zstd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PayloadStorage {
    /// Small payloads stored inline (post-encryption ciphertext).
    Inline(Vec<u8>),
    /// Large payloads stored as content-addressed blob files.
    Blob(BlobId),
    /// Reserved for v0.2+ monthly pack files. Not produced by v0.1 writers.
    Pack {
        pack_id: String,
        offset: u64,
        len: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlobId(pub [u8; 32]);

/// Materialized payload bytes carried between layers (never persisted as-is).
#[derive(Clone, Debug)]
pub struct PayloadData {
    pub format_id: String,
    pub bytes: Vec<u8>,
}
