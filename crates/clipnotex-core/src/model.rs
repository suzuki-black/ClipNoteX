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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ClipId;

    fn sample_item() -> ClipItem {
        ClipItem {
            id: ClipId::new(),
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
            source_app: SourceApp {
                bundle_id: Some("com.apple.Safari".into()),
                exe_basename: None,
                exe_path: None,
                display_name: "Safari".into(),
                window_title: Some("Example".into()),
            },
            primary_kind: ClipKind::Text,
            payloads: vec![PayloadRef {
                format_id: "public.utf8-plain-text".into(),
                compression: Compression::Zstd,
                storage: PayloadStorage::Inline(vec![1, 2, 3]),
                raw_size: 3,
            }],
            digest: [7u8; 32],
            text_preview: Some("hello".into()),
            pinned: false,
            tags: vec!["work".into()],
            total_bytes: 3,
        }
    }

    #[test]
    fn clip_item_json_roundtrip_preserves_fields() {
        let item = sample_item();
        let json = serde_json::to_string(&item).unwrap();
        let back: ClipItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, item.id);
        assert_eq!(back.primary_kind, ClipKind::Text);
        assert_eq!(back.digest, item.digest);
        assert_eq!(back.text_preview.as_deref(), Some("hello"));
        assert_eq!(back.tags, vec!["work".to_string()]);
        assert_eq!(back.payloads.len(), 1);
        assert_eq!(back.payloads[0].compression, Compression::Zstd);
    }

    #[test]
    fn payload_storage_variants_roundtrip() {
        let inline = PayloadStorage::Inline(vec![9, 8, 7]);
        let blob = PayloadStorage::Blob(BlobId([3u8; 32]));
        for s in [inline, blob] {
            let json = serde_json::to_string(&s).unwrap();
            let back: PayloadStorage = serde_json::from_str(&json).unwrap();
            match (s, back) {
                (PayloadStorage::Inline(a), PayloadStorage::Inline(b)) => assert_eq!(a, b),
                (PayloadStorage::Blob(a), PayloadStorage::Blob(b)) => assert_eq!(a, b),
                _ => panic!("variant changed across serde roundtrip"),
            }
        }
    }

    #[test]
    fn clip_kind_equality() {
        assert_eq!(ClipKind::Image, ClipKind::Image);
        assert_ne!(ClipKind::Image, ClipKind::Text);
    }
}
