//! Windows clipboard backend (skeleton — see DESIGN §5.2, IMPLEMENTATION M3-win).
//!
//! HGLOBAL handling will be confined to a future `hglobal` submodule with
//! a `// SAFETY:` comment on every unsafe block.

use crate::guard::SelfWriteGuard;
use crate::platform::{CapturedItem, ClipboardWatcher, ClipboardWriter};
use async_trait::async_trait;
use clipnotex_core::{
    model::{PayloadData, SourceApp},
    CnxError, Result,
};
use std::sync::Arc;

pub struct WinWatcher {
    _guard: Arc<SelfWriteGuard>,
}

impl WinWatcher {
    pub fn new(guard: Arc<SelfWriteGuard>) -> Result<Self> {
        Ok(Self { _guard: guard })
    }
}

#[async_trait]
impl ClipboardWatcher for WinWatcher {
    async fn next(&mut self) -> Result<Option<CapturedItem>> {
        // TODO(M3-win): hidden message-only window + AddClipboardFormatListener,
        // mpsc to async, EnumClipboardFormats with safelist::classify_windows,
        // HGLOBAL safe copy, GetClipboardOwner-based source detection.
        Err(CnxError::Clipboard(
            "WinWatcher::next not yet implemented".into(),
        ))
    }
}

pub struct WinWriter;

impl WinWriter {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl ClipboardWriter for WinWriter {
    fn write(&self, _payloads: &[PayloadData]) -> Result<()> {
        Err(CnxError::Clipboard("WinWriter::write not yet implemented".into()))
    }
    fn snapshot_for_restore(&self) -> Result<Vec<PayloadData>> {
        Err(CnxError::Clipboard(
            "WinWriter::snapshot_for_restore not yet implemented".into(),
        ))
    }
}

pub fn detect_source() -> Option<SourceApp> {
    // TODO(M3-win): GetClipboardOwner -> GetWindowThreadProcessId ->
    // OpenProcess(QUERY_LIMITED) -> QueryFullProcessImageName.
    None
}
