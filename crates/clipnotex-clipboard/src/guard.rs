use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// LRU + time-windowed set of digests we recently wrote to the OS
/// clipboard ourselves. Used to suppress the change notifications that
/// our own writes trigger.
pub struct SelfWriteGuard {
    inner: Mutex<Inner>,
    ttl: Duration,
    cap: usize,
}

struct Inner {
    entries: VecDeque<(blake3::Hash, Instant)>,
}

impl SelfWriteGuard {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(Inner {
                entries: VecDeque::with_capacity(64),
            }),
            ttl,
            cap: 64,
        }
    }

    pub fn register(&self, digest: [u8; 32]) {
        let h = blake3::Hash::from_bytes(digest);
        let mut inner = self.inner.lock();
        inner.entries.push_back((h, Instant::now()));
        if inner.entries.len() > self.cap {
            inner.entries.pop_front();
        }
    }

    pub fn contains(&self, digest: &[u8; 32]) -> bool {
        let h = blake3::Hash::from_bytes(*digest);
        let now = Instant::now();
        let mut inner = self.inner.lock();
        // prune expired
        while let Some(&(_, ts)) = inner.entries.front() {
            if now.duration_since(ts) > self.ttl {
                inner.entries.pop_front();
            } else {
                break;
            }
        }
        inner.entries.iter().any(|(d, _)| d == &h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_and_recalls() {
        let g = SelfWriteGuard::new(Duration::from_millis(500));
        let d = [7u8; 32];
        assert!(!g.contains(&d));
        g.register(d);
        assert!(g.contains(&d));
    }

    #[test]
    fn expires_after_ttl() {
        let g = SelfWriteGuard::new(Duration::from_millis(10));
        let d = [9u8; 32];
        g.register(d);
        std::thread::sleep(Duration::from_millis(30));
        assert!(!g.contains(&d));
    }
}
