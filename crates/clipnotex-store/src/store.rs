use crate::aead::{DataKeys, KeySource, Sealer};
use crate::blobs::BlobStore;
use crate::migrations::apply_pending;
use crate::tables::{BY_DIGEST, BY_TIME, ITEMS};
use clipnotex_core::{
    model::{BlobId, ClipItem, PayloadData, PayloadStorage},
    CnxError, ClipId, Result,
};
use redb::{Database, ReadableTable};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// ClipItem 内に直接埋め込む上限。これを超える payload は BlobStore に
/// オフロードされ、ClipItem には BlobId のみが残る。
/// 暗号化済み redb の B-Tree が肥大化するのを防ぐための閾値。
pub const BLOB_OFFLOAD_THRESHOLD: usize = 256 * 1024; // 256 KiB

pub struct StoreService {
    db: Arc<Database>,
    history_sealer: Sealer,
    blobs: BlobStore,
}

#[derive(Clone, Copy, Debug)]
pub enum EvictionPolicy {
    UntilCount(u64),
    UntilBytes(u64),
}

impl StoreService {
    pub fn open(data_dir: PathBuf, key_source: KeySource) -> Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("history.redb");
        let db = Database::create(&db_path)
            .map_err(|e| CnxError::Storage(format!("open db: {e}")))?;
        apply_pending(&db)?;

        let keys = DataKeys::load(&key_source)?;
        let history_sealer = Sealer::new(&keys.history);
        let blobs = BlobStore::new(data_dir.join("blobs"))?;
        Ok(Self {
            db: Arc::new(db),
            history_sealer,
            blobs,
        })
    }

    /// Insert a new ClipItem; if a record with the same digest already
    /// exists, only its `updated_at` is bumped.
    pub fn add_item(&self, mut item: ClipItem, _payloads: Vec<PayloadData>) -> Result<()> {
        let started = Instant::now();
        let id_bytes = item.id.as_bytes();
        let digest = item.digest;

        // 大きな Inline payload は BlobStore に振り分けて ClipItem から外す。
        self.offload_large_payloads(&mut item)?;

        // De-dup probe in a short read tx.
        if let Some(existing) = self.lookup_by_digest(&digest)? {
            return self.touch(existing, item.updated_at);
        }

        // TODO(M2): write payload bytes to BlobStore here, then patch
        // item.payloads with the resulting BlobIds before sealing.
        let serialized = bincode::serialize(&item)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;
        let aad = item.created_at.to_be_bytes();
        let sealed = self.history_sealer.seal(&serialized, &aad)?;

        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
        {
            let mut items = write
                .open_table(ITEMS)
                .map_err(|e| CnxError::Storage(format!("open items: {e}")))?;
            let mut by_time = write
                .open_table(BY_TIME)
                .map_err(|e| CnxError::Storage(format!("open by_time: {e}")))?;
            let mut by_digest = write
                .open_table(BY_DIGEST)
                .map_err(|e| CnxError::Storage(format!("open by_digest: {e}")))?;

            items
                .insert(id_bytes.as_slice(), sealed.as_slice())
                .map_err(|e| CnxError::Storage(format!("insert items: {e}")))?;
            by_time
                .insert((item.created_at, id_bytes.as_slice()), ())
                .map_err(|e| CnxError::Storage(format!("insert by_time: {e}")))?;
            by_digest
                .insert(digest.as_slice(), id_bytes.as_slice())
                .map_err(|e| CnxError::Storage(format!("insert by_digest: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("commit: {e}")))?;

        let elapsed = started.elapsed();
        if elapsed.as_millis() > 50 {
            tracing::warn!(?elapsed, "add_item exceeded 50ms budget");
        }
        item.updated_at = item.created_at;
        Ok(())
    }

    pub fn lookup_by_digest(&self, digest: &[u8; 32]) -> Result<Option<ClipId>> {
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("begin_read: {e}")))?;
        let table = match read.open_table(BY_DIGEST) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let v = table
            .get(digest.as_slice())
            .map_err(|e| CnxError::Storage(format!("get by_digest: {e}")))?;
        let Some(v) = v else { return Ok(None) };
        let bytes = v.value();
        if bytes.len() != 16 {
            return Ok(None);
        }
        let mut id = [0u8; 16];
        id.copy_from_slice(bytes);
        Ok(Some(ClipId(ulid::Ulid::from_bytes(id))))
    }

    /// Bump an existing item's timestamp to "now" so it sorts to the top.
    /// Used by `add_item` when the same content is captured again (dedup hit).
    ///
    /// Clipy/Maccy 互換: 同じ内容を再コピーしたら履歴の最上位に来るのが自然。
    /// 旧実装は no-op だったため、再コピーが「無視されたように見える」現象を
    /// 起こしていた (古い位置で更新されず、ユーザは「キャプチャされなかった」と誤認)。
    pub fn touch(&self, id: ClipId, _now: i64) -> Result<()> {
        self.bump_to_top(id)
    }

    /// Bump an item's `created_at` to "now" so that it sorts to the top of
    /// `list_recent`. This is Clipy-compatible behaviour: pasting a history
    /// entry promotes it to the most-recent slot.
    ///
    /// Implementation:
    ///   1. Decrypt with the OLD created_at AAD
    ///   2. Rewrite created_at + updated_at = now
    ///   3. Re-seal with the NEW AAD
    ///   4. Replace in ITEMS, remove old BY_TIME row, insert new BY_TIME row
    pub fn bump_to_top(&self, id: ClipId) -> Result<()> {
        let id_bytes = id.as_bytes();

        // 1) Find the existing entry and its current created_at via BY_TIME scan
        //    (get_item already does this; reuse it for the heavy lifting).
        let mut item = match self.get_item(id)? {
            Some(i) => i,
            None => return Ok(()), // gone
        };
        let old_created_at = item.created_at;
        let now = chrono::Utc::now().timestamp_millis();

        // Re-seal with new AAD.
        item.created_at = now;
        item.updated_at = now;
        let serialized = bincode::serialize(&item)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;
        let new_aad = now.to_be_bytes();
        let new_sealed = self.history_sealer.seal(&serialized, &new_aad)?;

        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
        {
            let mut items_tbl = write
                .open_table(ITEMS)
                .map_err(|e| CnxError::Storage(format!("open items: {e}")))?;
            items_tbl
                .insert(id_bytes.as_slice(), new_sealed.as_slice())
                .map_err(|e| CnxError::Storage(format!("insert items: {e}")))?;

            let mut by_time = write
                .open_table(BY_TIME)
                .map_err(|e| CnxError::Storage(format!("open by_time: {e}")))?;
            by_time
                .remove((old_created_at, id_bytes.as_slice()))
                .map_err(|e| CnxError::Storage(format!("remove old by_time: {e}")))?;
            by_time
                .insert((now, id_bytes.as_slice()), ())
                .map_err(|e| CnxError::Storage(format!("insert by_time: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("commit: {e}")))?;
        Ok(())
    }

    /// Fetch and decrypt a single item by its ULID.
    /// Returns `None` if the item doesn't exist.
    pub fn get_item(&self, id: ClipId) -> Result<Option<ClipItem>> {
        let id_bytes = id.as_bytes();
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("begin_read: {e}")))?;
        let items_tbl = match read.open_table(ITEMS) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let sealed = match items_tbl
            .get(id_bytes.as_slice())
            .map_err(|e| CnxError::Storage(format!("get item: {e}")))?
        {
            Some(v) => v.value().to_vec(),
            None => return Ok(None),
        };
        // Look up created_at from BY_TIME for the AAD.
        // As a shortcut, try all BY_TIME entries with this id; or just try
        // a zero AAD and fail gracefully (created_at is needed for open).
        // Better: scan BY_TIME for this id (secondary lookup via by_digest).
        // Simplest correct path: store created_at in the id itself isn't possible.
        // We store created_at in BY_TIME key, so scan and find the entry.
        let by_time = match read.open_table(BY_TIME) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let mut created_at_found: Option<i64> = None;
        for entry in by_time
            .iter()
            .map_err(|e| CnxError::Storage(format!("iter by_time: {e}")))?
        {
            let (key, _) = entry.map_err(|e| CnxError::Storage(format!("entry: {e}")))?;
            let (ts, candidate_id) = key.value();
            if candidate_id == id_bytes.as_slice() {
                created_at_found = Some(ts);
                break;
            }
        }
        let created_at = created_at_found
            .ok_or_else(|| CnxError::Storage("item not in BY_TIME index".into()))?;
        let aad = created_at.to_be_bytes();
        let plain = self.history_sealer.open(&sealed, &aad)?;
        let item: ClipItem = bincode::deserialize(&plain)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;
        Ok(Some(item))
    }

    /// Permanently delete an item by its ULID.
    /// Removes from ITEMS, BY_TIME, and BY_DIGEST.
    pub fn delete_item(&self, id: ClipId) -> Result<()> {
        let item = match self.get_item(id)? {
            Some(i) => i,
            None => return Ok(()), // already gone
        };
        let id_bytes = id.as_bytes();
        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
        {
            let mut items_tbl = write
                .open_table(ITEMS)
                .map_err(|e| CnxError::Storage(format!("open items: {e}")))?;
            let mut by_time = write
                .open_table(BY_TIME)
                .map_err(|e| CnxError::Storage(format!("open by_time: {e}")))?;
            let mut by_digest = write
                .open_table(BY_DIGEST)
                .map_err(|e| CnxError::Storage(format!("open by_digest: {e}")))?;

            items_tbl
                .remove(id_bytes.as_slice())
                .map_err(|e| CnxError::Storage(format!("remove items: {e}")))?;
            by_time
                .remove((item.created_at, id_bytes.as_slice()))
                .map_err(|e| CnxError::Storage(format!("remove by_time: {e}")))?;
            by_digest
                .remove(item.digest.as_slice())
                .map_err(|e| CnxError::Storage(format!("remove by_digest: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("commit: {e}")))?;
        Ok(())
    }

    /// **DESTRUCTIVE**: remove ALL clipboard history entries and blobs.
    /// Use this to recover from corrupted encryption state (mass decrypt-failures).
    /// The redb file is kept (empty tables remain) so subsequent writes work
    /// without a re-open / re-migrate cycle.
    pub fn reset_all(&self) -> Result<()> {
        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
        {
            // 各テーブルを開いて中身を全削除 (drain) する。redb には
            // truncate API がないので、key を集めて remove する。
            let mut items_tbl = write
                .open_table(ITEMS)
                .map_err(|e| CnxError::Storage(format!("open items: {e}")))?;
            let keys: Vec<Vec<u8>> = items_tbl
                .iter()
                .map_err(|e| CnxError::Storage(format!("iter items: {e}")))?
                .filter_map(|r| r.ok())
                .map(|(k, _)| k.value().to_vec())
                .collect();
            for k in keys {
                items_tbl
                    .remove(k.as_slice())
                    .map_err(|e| CnxError::Storage(format!("remove items: {e}")))?;
            }

            let mut by_time = write
                .open_table(BY_TIME)
                .map_err(|e| CnxError::Storage(format!("open by_time: {e}")))?;
            let keys: Vec<(i64, Vec<u8>)> = by_time
                .iter()
                .map_err(|e| CnxError::Storage(format!("iter by_time: {e}")))?
                .filter_map(|r| r.ok())
                .map(|(k, _)| {
                    let (ts, id) = k.value();
                    (ts, id.to_vec())
                })
                .collect();
            for (ts, id) in keys {
                by_time
                    .remove((ts, id.as_slice()))
                    .map_err(|e| CnxError::Storage(format!("remove by_time: {e}")))?;
            }

            let mut by_digest = write
                .open_table(BY_DIGEST)
                .map_err(|e| CnxError::Storage(format!("open by_digest: {e}")))?;
            let keys: Vec<Vec<u8>> = by_digest
                .iter()
                .map_err(|e| CnxError::Storage(format!("iter by_digest: {e}")))?
                .filter_map(|r| r.ok())
                .map(|(k, _)| k.value().to_vec())
                .collect();
            for k in keys {
                by_digest
                    .remove(k.as_slice())
                    .map_err(|e| CnxError::Storage(format!("remove by_digest: {e}")))?;
            }
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("commit: {e}")))?;

        // 物理 blob ファイルもすべて消す。中身は暗号化済みだが、ClipItem 側を
        // 消した今となっては参照不能なのでディスク領域を解放する。
        if let Some(parent) = self.blobs.root().parent() {
            // best-effort
            let _ = parent; // keep self.blobs.root() valid
        }
        let blob_root = self.blobs.root().to_path_buf();
        if blob_root.exists() {
            for entry in std::fs::read_dir(&blob_root)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                } else {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    /// Toggle the `pinned` flag on an item. Returns the new value.
    pub fn pin_toggle(&self, id: ClipId) -> Result<bool> {
        let mut item = self
            .get_item(id)?
            .ok_or_else(|| CnxError::Storage(format!("item {id:?} not found")))?;
        item.pinned = !item.pinned;
        let new_pinned = item.pinned;
        item.updated_at = chrono::Utc::now().timestamp_millis();

        let serialized = bincode::serialize(&item)
            .map_err(|e| CnxError::Serialize(e.to_string()))?;
        let aad = item.created_at.to_be_bytes();
        let sealed = self.history_sealer.seal(&serialized, &aad)?;

        let id_bytes = id.as_bytes();
        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
        {
            let mut items_tbl = write
                .open_table(ITEMS)
                .map_err(|e| CnxError::Storage(format!("open items: {e}")))?;
            items_tbl
                .insert(id_bytes.as_slice(), sealed.as_slice())
                .map_err(|e| CnxError::Storage(format!("insert items: {e}")))?;
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("commit: {e}")))?;
        Ok(new_pinned)
    }

    /// Scan BY_TIME in reverse (newest first), decrypt each ITEMS entry,
    /// optionally filter by `text_preview.contains(query)`, and return up to
    /// `limit` results.
    pub fn list_recent(&self, limit: usize, query: Option<&str>) -> Result<Vec<ClipItem>> {
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("begin_read: {e}")))?;

        // BY_TIME key = (created_at_ms: i64, ulid: &[u8]) — iterate descending.
        let by_time = match read.open_table(BY_TIME) {
            Ok(t) => t,
            Err(_) => return Ok(vec![]),
        };
        let items_tbl = match read.open_table(ITEMS) {
            Ok(t) => t,
            Err(_) => return Ok(vec![]),
        };

        let mut results = Vec::with_capacity(limit.min(64));
        for entry in by_time
            .iter()
            .map_err(|e| CnxError::Storage(format!("iter by_time: {e}")))?
            .rev()
        {
            if results.len() >= limit {
                break;
            }
            let (key, _) = entry.map_err(|e| CnxError::Storage(format!("entry: {e}")))?;
            let (_ts, id_bytes) = key.value();

            let sealed = match items_tbl
                .get(id_bytes)
                .map_err(|e| CnxError::Storage(format!("get item: {e}")))?
            {
                Some(v) => v.value().to_vec(),
                None => continue,
            };

            // AAD = created_at big-endian, same as in add_item.
            let aad = _ts.to_be_bytes();

            let plain = match self.history_sealer.open(&sealed, &aad) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(?e, "decrypt failed for item, skipping");
                    continue;
                }
            };

            let item: ClipItem = match bincode::deserialize(&plain) {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!(?e, "deserialize failed for item, skipping");
                    continue;
                }
            };

            // Optional full-text filter on text_preview.
            if let Some(q) = query {
                let q_lower = q.to_lowercase();
                let matches = item
                    .text_preview
                    .as_deref()
                    .map(|p| p.to_lowercase().contains(&q_lower))
                    .unwrap_or(false);
                if !matches {
                    continue;
                }
            }

            results.push(item);
        }
        // Pinned items always sort to the top, preserving recency within
        // each group. Stable sort keeps the original BY_TIME order intact
        // for ties.
        results.sort_by(|a, b| b.pinned.cmp(&a.pinned));
        Ok(results)
    }

    pub fn count_and_bytes(&self) -> Result<(u64, u64)> {
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("begin_read: {e}")))?;
        let items = match read.open_table(ITEMS) {
            Ok(t) => t,
            Err(_) => return Ok((0, 0)),
        };
        let mut count = 0u64;
        let mut bytes = 0u64;
        for entry in items
            .iter()
            .map_err(|e| CnxError::Storage(format!("iter items: {e}")))?
        {
            let (_, v) = entry.map_err(|e| CnxError::Storage(format!("entry: {e}")))?;
            count += 1;
            bytes += v.value().len() as u64;
        }
        Ok((count, bytes))
    }

    /// Evict oldest unpinned items until the policy target is met.
    /// Returns the number of items removed.
    pub fn evict(&self, policy: EvictionPolicy) -> Result<u64> {
        // First, check whether eviction is even needed.
        let (total_count, total_bytes) = self.count_and_bytes()?;
        let needs_evict = match policy {
            EvictionPolicy::UntilCount(target) => total_count > target,
            EvictionPolicy::UntilBytes(target) => total_bytes > target,
        };
        if !needs_evict {
            return Ok(0);
        }

        // Collect candidates: BY_TIME ascending (oldest first), skip pinned.
        // We read all candidates first so we can compute running totals.
        let read = self
            .db
            .begin_read()
            .map_err(|e| CnxError::Storage(format!("begin_read: {e}")))?;
        let by_time = match read.open_table(BY_TIME) {
            Ok(t) => t,
            Err(_) => return Ok(0),
        };
        let items_tbl = match read.open_table(ITEMS) {
            Ok(t) => t,
            Err(_) => return Ok(0),
        };

        // Gather (created_at, id_bytes, sealed_len) for eviction candidates.
        let mut candidates: Vec<(i64, [u8; 16], usize, [u8; 32])> = vec![];
        for entry in by_time
            .iter()
            .map_err(|e| CnxError::Storage(format!("iter by_time: {e}")))?
        {
            let (key, _) = entry.map_err(|e| CnxError::Storage(format!("entry: {e}")))?;
            let (ts, id_slice) = key.value();
            if id_slice.len() != 16 {
                continue;
            }
            let mut id_bytes = [0u8; 16];
            id_bytes.copy_from_slice(id_slice);

            // Peek at the sealed blob to get size and decrypt to check pinned.
            let sealed = match items_tbl
                .get(id_slice)
                .map_err(|e| CnxError::Storage(format!("get item: {e}")))?
            {
                Some(v) => v.value().to_vec(),
                None => continue,
            };
            let sealed_len = sealed.len();

            // Decrypt to read pinned flag.
            let aad = ts.to_be_bytes();
            let plain = match self.history_sealer.open(&sealed, &aad) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let item: ClipItem = match bincode::deserialize(&plain) {
                Ok(i) => i,
                Err(_) => continue,
            };
            if item.pinned {
                continue; // never evict pinned items
            }
            candidates.push((ts, id_bytes, sealed_len, item.digest));
        }
        drop(items_tbl);
        drop(by_time);
        drop(read);

        // Decide how many to remove.
        let to_remove: Vec<_> = match policy {
            EvictionPolicy::UntilCount(target) => {
                let excess = total_count.saturating_sub(target) as usize;
                candidates.into_iter().take(excess).collect()
            }
            EvictionPolicy::UntilBytes(target) => {
                let mut acc = total_bytes;
                let mut out = vec![];
                for c in candidates {
                    if acc <= target {
                        break;
                    }
                    acc = acc.saturating_sub(c.2 as u64);
                    out.push(c);
                }
                out
            }
        };

        if to_remove.is_empty() {
            return Ok(0);
        }

        // Batch delete.
        let write = self
            .db
            .begin_write()
            .map_err(|e| CnxError::Storage(format!("begin_write: {e}")))?;
        let removed = to_remove.len() as u64;
        {
            let mut items_tbl = write
                .open_table(ITEMS)
                .map_err(|e| CnxError::Storage(format!("open items: {e}")))?;
            let mut by_time = write
                .open_table(BY_TIME)
                .map_err(|e| CnxError::Storage(format!("open by_time: {e}")))?;
            let mut by_digest = write
                .open_table(BY_DIGEST)
                .map_err(|e| CnxError::Storage(format!("open by_digest: {e}")))?;

            for (ts, id_bytes, _, digest) in &to_remove {
                items_tbl
                    .remove(id_bytes.as_slice())
                    .map_err(|e| CnxError::Storage(format!("remove items: {e}")))?;
                by_time
                    .remove((*ts, id_bytes.as_slice()))
                    .map_err(|e| CnxError::Storage(format!("remove by_time: {e}")))?;
                by_digest
                    .remove(digest.as_slice())
                    .map_err(|e| CnxError::Storage(format!("remove by_digest: {e}")))?;
            }
        }
        write
            .commit()
            .map_err(|e| CnxError::Storage(format!("commit: {e}")))?;

        tracing::info!(removed, "evicted items");
        Ok(removed)
    }

    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }
    pub fn db(&self) -> &Database {
        &self.db
    }

    // -----------------------------------------------------------------------
    // Blob offload / expand
    // -----------------------------------------------------------------------

    /// Scan `item.payloads` and move any `Inline(bytes)` larger than
    /// [`BLOB_OFFLOAD_THRESHOLD`] into the [`BlobStore`], replacing the
    /// storage variant with `Blob(BlobId)`. The blob file is encrypted with
    /// the history sealer using the BlobId itself as AAD (binds the
    /// ciphertext to the address).
    fn offload_large_payloads(&self, item: &mut ClipItem) -> Result<()> {
        for p in item.payloads.iter_mut() {
            if let PayloadStorage::Inline(bytes) = &p.storage {
                if bytes.len() > BLOB_OFFLOAD_THRESHOLD {
                    let id = BlobId(blake3::hash(bytes).into());
                    if !self.blobs.path(&id).exists() {
                        let sealed = self.history_sealer.seal(bytes, &id.0)?;
                        self.blobs.write(&id, &sealed)?;
                    }
                    p.storage = PayloadStorage::Blob(id);
                }
            }
        }
        Ok(())
    }

    /// Inverse of `offload_large_payloads`: materialise every payload back
    /// to raw bytes so callers (e.g. the paste pipeline) can write them
    /// straight to the OS clipboard.
    pub fn materialize_payloads(&self, item: &ClipItem) -> Result<Vec<PayloadData>> {
        let mut out = Vec::with_capacity(item.payloads.len());
        for p in &item.payloads {
            let bytes = match &p.storage {
                PayloadStorage::Inline(b) => b.clone(),
                PayloadStorage::Blob(id) => {
                    let sealed = self.blobs.read(id)?;
                    self.history_sealer.open(&sealed, &id.0)?
                }
                PayloadStorage::Pack { .. } => {
                    // Reserved for v0.3+ pack files; treat as empty for now.
                    continue;
                }
            };
            out.push(PayloadData {
                format_id: p.format_id.clone(),
                bytes,
            });
        }
        Ok(out)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use clipnotex_core::model::{ClipKind, SourceApp};

    fn make_item(text: &str) -> ClipItem {
        let now = chrono::Utc::now().timestamp_millis();
        let digest: [u8; 32] = blake3::hash(text.as_bytes()).into();
        ClipItem {
            id: ClipId::new(),
            created_at: now,
            updated_at: now,
            source_app: SourceApp::default(),
            primary_kind: ClipKind::Text,
            payloads: vec![],
            digest,
            text_preview: Some(text.into()),
            pinned: false,
            tags: vec![],
            total_bytes: text.len() as u64,
        }
    }

    #[test]
    fn open_and_insert() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        svc.add_item(make_item("hello"), vec![]).unwrap();
        let (n, _) = svc.count_and_bytes().unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn duplicate_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        svc.add_item(make_item("dup"), vec![]).unwrap();
        svc.add_item(make_item("dup"), vec![]).unwrap();
        let (n, _) = svc.count_and_bytes().unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn list_recent_returns_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        // Insert with distinct timestamps (1ms apart to guarantee ordering).
        for text in ["alpha", "beta", "gamma"] {
            std::thread::sleep(std::time::Duration::from_millis(2));
            svc.add_item(make_item(text), vec![]).unwrap();
        }
        let items = svc.list_recent(10, None).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].text_preview.as_deref(), Some("gamma"));
        assert_eq!(items[2].text_preview.as_deref(), Some("alpha"));
    }

    #[test]
    fn list_recent_filters_by_query() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        svc.add_item(make_item("hello world"), vec![]).unwrap();
        svc.add_item(make_item("foo bar"), vec![]).unwrap();

        let hits = svc.list_recent(10, Some("hello")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text_preview.as_deref(), Some("hello world"));
    }

    #[test]
    fn list_recent_respects_limit() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        for i in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            svc.add_item(make_item(&format!("item{i}")), vec![]).unwrap();
        }
        let items = svc.list_recent(3, None).unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn delete_item_removes_from_all_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        let item = make_item("delete-me");
        let id = item.id;
        svc.add_item(item, vec![]).unwrap();
        assert_eq!(svc.count_and_bytes().unwrap().0, 1);

        svc.delete_item(id).unwrap();
        assert_eq!(svc.count_and_bytes().unwrap().0, 0);
        // Should not appear in list anymore.
        assert!(svc.list_recent(10, None).unwrap().is_empty());
    }

    #[test]
    fn pin_toggle_flips_flag() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        let item = make_item("pin-me");
        let id = item.id;
        svc.add_item(item, vec![]).unwrap();

        let pinned = svc.pin_toggle(id).unwrap();
        assert!(pinned, "first toggle should pin");

        let unpinned = svc.pin_toggle(id).unwrap();
        assert!(!unpinned, "second toggle should unpin");
    }

    #[test]
    fn evict_by_count_removes_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        for i in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            svc.add_item(make_item(&format!("evict{i}")), vec![]).unwrap();
        }
        let removed = svc.evict(EvictionPolicy::UntilCount(3)).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(svc.count_and_bytes().unwrap().0, 3);
        // Newest 3 should remain.
        let items = svc.list_recent(10, None).unwrap();
        assert_eq!(items[0].text_preview.as_deref(), Some("evict4"));
    }

    #[test]
    fn evict_skips_pinned_items() {
        let dir = tempfile::tempdir().unwrap();
        let svc = StoreService::open(dir.path().into(), KeySource::Ephemeral).unwrap();
        let pinned_item = make_item("pinned");
        let pinned_id = pinned_item.id;
        svc.add_item(pinned_item, vec![]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        svc.add_item(make_item("normal"), vec![]).unwrap();
        svc.pin_toggle(pinned_id).unwrap();

        // Evict down to 1 — only "normal" should be removed, not "pinned".
        let removed = svc.evict(EvictionPolicy::UntilCount(1)).unwrap();
        assert_eq!(removed, 1);
        let items = svc.list_recent(10, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text_preview.as_deref(), Some("pinned"));
    }
}
