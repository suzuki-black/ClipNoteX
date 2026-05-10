use redb::TableDefinition;

/// Encrypted ClipItem record. Key = ULID bytes, value = AEAD ciphertext.
pub const ITEMS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("items");

/// Time-ordered index. Key = (created_at_ms, ulid), value = ().
pub const BY_TIME: TableDefinition<(i64, &[u8]), ()> = TableDefinition::new("by_time");

/// Digest -> ULID, for de-duplication.
pub const BY_DIGEST: TableDefinition<&[u8], &[u8]> = TableDefinition::new("by_digest");

/// Blob refcount + size + on-disk path. Value is bincode-encoded.
#[allow(dead_code)]
pub const BLOBS_META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blobs_meta");

/// `version` -> u32 schema version.
pub const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
