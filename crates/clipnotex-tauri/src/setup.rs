//! Composition root: wires all services together during Tauri setup.
//!
//! # 重要: compose() は絶対に Err を返してはいけない
//! Tauri 2 は setup() が Err を返すと内部で panic! を呼ぶため、
//! Objective-C コールバック境界を越えて abort() になる。
//! すべてのエラーはログして続行する。
//!
//! # パニックについて
//! setup() は macOS の applicationDidFinishLaunching (ObjC コールバック) から
//! 呼ばれる。Rust のパニックは ObjC フレームをアンワインドできないため、
//! ObjC 境界に到達すると panic_cannot_unwind → abort() になる。
//! compose() は catch_unwind でラップし、パニックが境界を越えないようにする。
//!
//! # tokio::spawn 禁止
//! setup 内では tokio::spawn を使ってはいけない。
//! ObjC コールバック内では Tokio ランタイムが current でなく、
//! tokio::spawn が "no current runtime" パニックを起こす。
//! 代わりに tauri::async_runtime::spawn を使う。

use crate::state::AppState;
use clipnotex_app::{run_capture_loop, ExclusionFilter, QuotaManager};
use clipnotex_clipboard::SelfWriteGuard;
use clipnotex_core::{
    bus::{CoreEvent, EventBus},
    settings::Settings,
};
use clipnotex_donelog::DoneLogStore;
use clipnotex_hotkey::HotkeyService;
use clipnotex_paste::PasteController;
use clipnotex_store::{KeySource, StoreService};
use std::sync::Arc;
use tauri::Manager;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "clipnotex=debug,warn".into()),
        )
        .init();
}

/// dev ビルドはキーチェーン不要の Ephemeral キーを使う。
fn key_source() -> KeySource {
    #[cfg(debug_assertions)]
    {
        tracing::info!("dev build: using ephemeral (in-memory) encryption keys");
        KeySource::Ephemeral
    }
    #[cfg(not(debug_assertions))]
    {
        KeySource::Keyring {
            service: "ClipNoteX".into(),
            account: "data_key".into(),
        }
    }
}

/// **絶対に Err を返さず、パニックも伝播させない。**
/// catch_unwind で compose_inner を包み、内部でパニックが起きても
/// ObjC コールバック境界を越える前に捕まえて Ok(()) を返す。
pub fn compose(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Safety: AssertUnwindSafe は正当 — パニック後の部分初期化状態の
    // tauri::App を使わないことで安全性を担保する (Ok(()) を返すだけ)。
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compose_inner(app))) {
        Ok(result) => result,
        Err(e) => {
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_string());
            // Use eprintln too in case tracing isn't initialized yet.
            eprintln!("[CLIPNOTEX FATAL] compose() panicked: {msg}");
            tracing::error!(panic = %msg, "compose() panicked — app is running in degraded mode");
            Ok(())
        }
    }
}

/// 実際のセットアップロジック。compose() の catch_unwind の中から呼ばれる。
/// Err を返しても compose() がそれを処理するので構わないが、
/// ここでもエラーはできるだけログして Ok(()) で続行することを推奨する。
fn compose_inner(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // このメッセージが表示されれば panic-free ビルドが動いている。
    tracing::info!("=== ClipNoteX compose_inner() starting — tauri::async_runtime build ===");

    // ---------- Data directory ----------
    let data_dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(?e, "cannot resolve app_data_dir, using temp dir");
            std::env::temp_dir().join("clipnotex-data")
        }
    };
    tracing::info!(?data_dir, "app data directory");

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        tracing::error!(?e, "cannot create data dir");
    }

    // ---------- Storage ----------
    let store = match StoreService::open(data_dir.clone(), key_source()) {
        Ok(s) => {
            tracing::info!("history store opened");
            Arc::new(s)
        }
        Err(e) => {
            tracing::error!(?e, "failed to open history store — using temp fallback");
            let pid = std::process::id();
            let tmp = std::env::temp_dir().join(format!("clipnotex-fallback-{pid}"));
            match StoreService::open(tmp, KeySource::Ephemeral) {
                Ok(s) => Arc::new(s),
                Err(e2) => {
                    tracing::error!(?e2, "temp history store also failed — no history available");
                    let tmp2 = std::env::temp_dir().join(format!("clipnotex-last-{pid}"));
                    // 最終手段: ここが失敗するなら OS が壊れており catch_unwind が処理する。
                    Arc::new(StoreService::open(tmp2, KeySource::Ephemeral)
                        .expect("OS temp dir is not writable"))
                }
            }
        }
    };

    let donelog = match DoneLogStore::open(data_dir.join("donelog"), &key_source()) {
        Ok(d) => {
            tracing::info!("donelog store opened");
            Arc::new(d)
        }
        Err(e) => {
            tracing::error!(?e, "failed to open donelog — using temp fallback");
            let pid = std::process::id();
            let tmp = std::env::temp_dir().join(format!("clipnotex-donelog-fallback-{pid}"));
            match DoneLogStore::open(tmp, &KeySource::Ephemeral) {
                Ok(d) => Arc::new(d),
                Err(e2) => {
                    tracing::error!(?e2, "temp donelog also failed — no donelog available");
                    let tmp2 = std::env::temp_dir().join(format!("clipnotex-donelog-last-{pid}"));
                    Arc::new(DoneLogStore::open(tmp2, &KeySource::Ephemeral)
                        .expect("OS temp dir is not writable"))
                }
            }
        }
    };

    // ---------- Event bus + Filter ----------
    let settings = Settings::default();
    let bus = EventBus::new(256);
    let filter = ExclusionFilter::new(settings.exclusions.clone(), true);
    let guard = Arc::new(SelfWriteGuard::new(std::time::Duration::from_secs(5)));

    // ---------- Clipboard (non-fatal) ----------
    let paste: Arc<PasteController> = match clipnotex_clipboard::open(guard.clone()) {
        Ok((watcher, writer)) => {
            tracing::info!("clipboard opened");
            let writer: Arc<dyn clipnotex_clipboard::ClipboardWriter> = Arc::from(writer);

            // Capture loop
            // ⚠️  tauri::async_runtime::spawn を使うこと。
            //    tokio::spawn は ObjC コールバック内で "no current runtime" パニックを起こす。
            //    tauri::async_runtime は内部に Handle を保持しており、
            //    ランタイムが current でなくても安全に spawn できる。
            {
                let store2 = store.clone();
                let filter2 = filter.clone();
                let bus2 = bus.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = run_capture_loop(watcher, filter2, store2, bus2).await {
                        tracing::error!(?e, "capture loop exited with error");
                    }
                });
            }

            // Quota enforcement on each capture
            let quota = QuotaManager::new(store.clone(), settings.history.clone());
            let _ = quota.enforce();
            {
                let quota2 = quota.clone();
                let mut rx = bus.subscribe();
                tauri::async_runtime::spawn(async move {
                    while let Ok(ev) = rx.recv().await {
                        if let CoreEvent::ClipboardCaptured(_) = ev {
                            let q = quota2.clone();
                            // spawn_blocking はタスク内 (= Tokio ランタイム内) なので安全。
                            tokio::task::spawn_blocking(move || {
                                if let Err(e) = q.enforce() {
                                    tracing::warn!(?e, "quota enforcement failed");
                                }
                            });
                        }
                    }
                });
            }

            Arc::new(PasteController::new(writer, guard.clone()))
        }
        Err(e) => {
            tracing::warn!(
                ?e,
                "clipboard unavailable — read-only mode. \
                 Grant Accessibility permission in System Settings if needed."
            );
            let dummy: Arc<dyn clipnotex_clipboard::ClipboardWriter> = Arc::new(DummyWriter);
            Arc::new(PasteController::new(dummy, guard.clone()))
        }
    };

    // ---------- Hotkeys (non-fatal) ----------
    match HotkeyService::new(bus.clone()) {
        Ok(hotkey_svc) => {
            let shortcuts: Vec<_> = settings
                .shortcuts
                .iter()
                .filter_map(|(id, binding)| {
                    clipnotex_hotkey::platform_accel(binding).map(|a| (*id, a.to_string()))
                })
                .collect();
            for r in hotkey_svc.register_all(&shortcuts) {
                if let Err(ref f) = r.outcome {
                    tracing::warn!(
                        id = ?r.id,
                        accelerator = %r.accelerator,
                        reason = %f.reason,
                        "hotkey registration failed"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                ?e,
                "hotkey service unavailable — global shortcuts disabled. \
                 Grant Accessibility permission in System Settings if needed."
            );
        }
    }

    // ---------- Register AppState ----------
    app.manage(AppState {
        store,
        donelog,
        filter,
        paste,
    });

    tracing::info!("compose_inner() completed successfully");
    Ok(())
}

// ---------------------------------------------------------------------------
// DummyWriter — no-op clipboard writer for read-only mode
// ---------------------------------------------------------------------------

struct DummyWriter;

impl clipnotex_clipboard::ClipboardWriter for DummyWriter {
    fn write(
        &self,
        _payloads: &[clipnotex_core::model::PayloadData],
    ) -> clipnotex_core::Result<()> {
        Err(clipnotex_core::CnxError::Other(
            "clipboard unavailable (read-only mode)".into(),
        ))
    }

    fn snapshot_for_restore(
        &self,
    ) -> clipnotex_core::Result<Vec<clipnotex_core::model::PayloadData>> {
        Ok(vec![])
    }
}
