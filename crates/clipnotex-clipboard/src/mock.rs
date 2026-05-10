//! In-memory mock for unit tests in dependent crates.

use crate::platform::{CapturedItem, ClipboardWatcher, ClipboardWriter};
use async_trait::async_trait;
use clipnotex_core::{model::PayloadData, Result};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Default)]
pub struct MockClipboard {
    pub queue: Mutex<VecDeque<CapturedItem>>,
    pub current: Mutex<Vec<PayloadData>>,
}

impl MockClipboard {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn push(&self, item: CapturedItem) {
        self.queue.lock().push_back(item);
    }
}

pub struct MockWatcher(pub Arc<MockClipboard>);

#[async_trait]
impl ClipboardWatcher for MockWatcher {
    async fn next(&mut self) -> Result<Option<CapturedItem>> {
        loop {
            if let Some(it) = self.0.queue.lock().pop_front() {
                return Ok(Some(it));
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }
}

pub struct MockWriter(pub Arc<MockClipboard>);

impl ClipboardWriter for MockWriter {
    fn write(&self, payloads: &[PayloadData]) -> Result<()> {
        *self.0.current.lock() = payloads.to_vec();
        Ok(())
    }
    fn snapshot_for_restore(&self) -> Result<Vec<PayloadData>> {
        Ok(self.0.current.lock().clone())
    }
}
