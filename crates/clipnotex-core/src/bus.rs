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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscriber_receives_emitted_event() {
        let bus = EventBus::new(8);
        let mut rx = bus.subscribe();
        bus.emit(CoreEvent::SettingsChanged);
        match rx.try_recv() {
            Ok(CoreEvent::SettingsChanged) => {}
            other => panic!("expected SettingsChanged, got {other:?}"),
        }
    }

    #[test]
    fn emit_without_subscribers_does_not_panic() {
        let bus = EventBus::new(4);
        bus.emit(CoreEvent::HotkeyPressed(HotkeyId::ShowHistory));
    }

    #[test]
    fn multiple_subscribers_each_get_the_event() {
        let bus = EventBus::new(8);
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();
        bus.emit(CoreEvent::Quota(QuotaEvent::Evicted { count: 2, bytes: 100 }));
        assert!(matches!(a.try_recv(), Ok(CoreEvent::Quota(_))));
        assert!(matches!(b.try_recv(), Ok(CoreEvent::Quota(_))));
    }
}
