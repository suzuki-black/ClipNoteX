use async_trait::async_trait;
use clipnotex_core::{
    model::{ClipKind, PayloadData, SourceApp},
    Result,
};

#[derive(Clone, Debug)]
pub struct CapturedItem {
    pub source_app: SourceApp,
    pub payloads: Vec<PayloadData>,
    pub primary_kind: ClipKind,
    pub digest: [u8; 32],
    pub captured_at: i64,
}

#[async_trait]
pub trait ClipboardWatcher: Send + Sync {
    /// Block until the next non-self, non-concealed clipboard change.
    /// Returns `None` if the watcher has been shut down.
    async fn next(&mut self) -> Result<Option<CapturedItem>>;
}

pub trait ClipboardWriter: Send + Sync {
    /// Write the given payloads to the OS clipboard, replacing whatever
    /// is currently there.
    fn write(&self, payloads: &[PayloadData]) -> Result<()>;

    /// Snapshot the current OS clipboard for later restoration (used by
    /// the Stage A "preserve and restore" paste mode).
    fn snapshot_for_restore(&self) -> Result<Vec<PayloadData>>;
}
