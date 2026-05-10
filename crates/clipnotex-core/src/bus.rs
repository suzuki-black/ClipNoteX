use crate::ids::{ClipId, HotkeyId};
use tokio::sync::broadcast;

/// Cross-crate event channel. Subscribers must keep up; lagged receivers
/// will silently drop events (intentional — events are advisory).
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<CoreEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn emit(&self, ev: CoreEvent) {
        let _ = self.tx.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.tx.subscribe()
    }
}

#[derive(Clone, Debug)]
pub enum CoreEvent {
    ClipboardCaptured(ClipId),
    ClipboardSkipped { reason: SkipReason },
    HotkeyPressed(HotkeyId),
    Quota(QuotaEvent),
    SettingsChanged,
}

#[derive(Clone, Debug)]
pub enum SkipReason {
    Excluded,
    Concealed,
    SelfWrite,
    Empty,
}

#[derive(Clone, Debug)]
pub enum QuotaEvent {
    Evicted { count: u64, bytes: u64 },
}
