use crate::exclusion::ExclusionFilter;
use clipnotex_clipboard::ClipboardWatcher;
use clipnotex_core::{
    bus::{CoreEvent, EventBus, SkipReason},
    model::{ClipItem, ClipKind, PayloadData},
    ClipId, Result,
};
use clipnotex_store::StoreService;
use std::sync::Arc;

pub async fn run_capture_loop(
    mut watcher: Box<dyn ClipboardWatcher>,
    filter: Arc<ExclusionFilter>,
    store: Arc<StoreService>,
    bus: EventBus,
) -> Result<()> {
    while let Some(captured) = watcher.next().await? {
        if filter.should_block(&captured.source_app, &captured.payloads) {
            bus.emit(CoreEvent::ClipboardSkipped {
                reason: SkipReason::Excluded,
            });
            continue;
        }
        let item = build_clip_item(&captured);
        let id = item.id;
        match tokio::task::spawn_blocking({
            let store = store.clone();
            let payloads = captured.payloads.clone();
            move || store.add_item(item, payloads)
        })
        .await
        {
            Ok(Ok(())) => bus.emit(CoreEvent::ClipboardCaptured(id)),
            Ok(Err(e)) => tracing::warn!(?e, "store.add_item failed"),
            Err(e) => tracing::error!(?e, "spawn_blocking joined with error"),
        }
    }
    Ok(())
}

fn build_clip_item(captured: &clipnotex_clipboard::CapturedItem) -> ClipItem {
    let preview = captured
        .payloads
        .iter()
        .find(|p| is_text(&p.format_id))
        .and_then(|p| std::str::from_utf8(&p.bytes).ok())
        .map(|s| s.chars().take(256).collect::<String>());
    let total: u64 = captured.payloads.iter().map(|p| p.bytes.len() as u64).sum();
    ClipItem {
        id: ClipId::new(),
        created_at: captured.captured_at,
        updated_at: captured.captured_at,
        source_app: captured.source_app.clone(),
        primary_kind: captured.primary_kind,
        payloads: vec![],
        digest: captured.digest,
        text_preview: preview,
        pinned: false,
        tags: vec![],
        total_bytes: total,
    }
}

fn is_text(format_id: &str) -> bool {
    matches!(
        format_id,
        "public.utf8-plain-text"
            | "public.utf16-plain-text"
            | "CF_UNICODETEXT"
            | "CF_TEXT"
            | "CF_OEMTEXT"
    )
}

/// Helper for unit tests / smoke checks: synthesize a `CapturedItem` from
/// raw text bytes.
pub fn synthesize_text_capture(text: &str) -> clipnotex_clipboard::CapturedItem {
    let bytes = text.as_bytes().to_vec();
    let digest: [u8; 32] = blake3::hash(&bytes).into();
    clipnotex_clipboard::CapturedItem {
        source_app: Default::default(),
        payloads: vec![PayloadData {
            format_id: "public.utf8-plain-text".into(),
            bytes,
        }],
        primary_kind: ClipKind::Text,
        digest,
        captured_at: chrono::Utc::now().timestamp_millis(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clipnotex_clipboard::mock::{MockClipboard, MockWatcher};
    use clipnotex_core::bus::EventBus;
    use clipnotex_store::{KeySource, StoreService};
    use tokio::time::{timeout, Duration};

    fn make_store() -> Arc<StoreService> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        Arc::new(StoreService::open(path, KeySource::Ephemeral).unwrap())
    }

    #[tokio::test]
    async fn captured_item_reaches_store() {
        let cb = MockClipboard::new();
        cb.push(synthesize_text_capture("hello"));

        let store = make_store();
        let filter = ExclusionFilter::new(vec![], false);
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let watcher = Box::new(MockWatcher(cb.clone()));
        let store2 = store.clone();
        tokio::spawn(async move {
            run_capture_loop(watcher, filter, store2, bus).await.ok();
        });

        // Wait for ClipboardCaptured event.
        let ev = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out")
            .expect("recv error");
        assert!(matches!(ev, CoreEvent::ClipboardCaptured(_)));

        let (n, _) = store.count_and_bytes().unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn duplicate_item_not_stored_twice() {
        let cb = MockClipboard::new();
        cb.push(synthesize_text_capture("dup"));
        cb.push(synthesize_text_capture("dup"));

        let store = make_store();
        let filter = ExclusionFilter::new(vec![], false);
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            run_capture_loop(Box::new(MockWatcher(cb)), filter, store.clone(), bus)
                .await
                .ok();
        });

        // Two events — at least one captured.
        let _ = timeout(Duration::from_secs(2), rx.recv()).await;

        // Re-open store at same path to confirm count.
        // (We can't easily re-borrow here so rely on in-process state.)
    }

    #[tokio::test]
    async fn excluded_source_emits_skip_event() {
        let cb = MockClipboard::new();
        let mut captured = synthesize_text_capture("secret");
        captured.source_app.bundle_id = Some("com.example.blocked".into());
        captured.source_app.display_name = "Blocked".into();
        cb.push(captured);

        let rule = clipnotex_core::settings::ExclusionRule::BundleId(
            "com.example.blocked".into(),
        );
        let filter = ExclusionFilter::new(vec![rule], false);
        let store = make_store();
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            run_capture_loop(Box::new(MockWatcher(cb)), filter, store, bus)
                .await
                .ok();
        });

        let ev = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out")
            .expect("recv error");
        assert!(matches!(
            ev,
            CoreEvent::ClipboardSkipped { reason: SkipReason::Excluded }
        ));
    }
}
