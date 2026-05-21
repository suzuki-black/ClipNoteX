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
    ids::HotkeyId,
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
    // HotkeyService を Arc で保持し、ポンプループ内で生かし続ける。
    // Arc が drop されると OS のホットキー登録も解除されるため重要。
    let hotkey_svc_opt: Option<Arc<HotkeyService>> =
        match HotkeyService::new(bus.clone()) {
            Ok(svc) => {
                let shortcuts: Vec<_> = settings
                    .shortcuts
                    .iter()
                    .filter_map(|(id, binding)| {
                        clipnotex_hotkey::platform_accel(binding)
                            .map(|a| (*id, a.to_string()))
                    })
                    .collect();
                for r in svc.register_all(&shortcuts) {
                    if let Err(ref f) = r.outcome {
                        tracing::warn!(
                            id = ?r.id,
                            accelerator = %r.accelerator,
                            reason = %f.reason,
                            "hotkey registration failed"
                        );
                    }
                }
                Some(svc)
            }
            Err(e) => {
                tracing::warn!(
                    ?e,
                    "hotkey service unavailable — global shortcuts disabled. \
                     Grant Accessibility permission in System Settings if needed."
                );
                None
            }
        };

    // ホットキーポンプループ:
    // global-hotkey は OS イベントをチャネルに積む。
    // pump() を定期的に呼んで CoreEvent::HotkeyPressed に変換する。
    if let Some(svc) = hotkey_svc_opt {
        tauri::async_runtime::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_millis(50));
            loop {
                interval.tick().await;
                svc.pump();
            }
        });
    }

    // ホットキーイベントリスナー: ShowHistory でウィンドウをトグル
    // (Clipy 同様、押すたび開閉。隠れている時はグローバルホットキーとして発火する)
    {
        let app_handle = app.handle().clone();
        let mut hotkey_rx = bus.subscribe();
        tauri::async_runtime::spawn(async move {
            while let Ok(ev) = hotkey_rx.recv().await {
                if let CoreEvent::HotkeyPressed(HotkeyId::ShowHistory) = ev {
                    toggle_history_window(&app_handle);
                }
            }
        });
    }

    // ---------- System tray (non-fatal) ----------
    setup_tray(app);

    // macOS: 当面 Regular ポリシー (Dock に出るが、キーボード入力を確実に受け取る)。
    // Accessory にすると set_focus 後も WebView がキーボード入力を受け取れない
    // 場合があり、Enter キーで Paste 発火に失敗する事象を確認したため一旦戻す。
    // TODO: ステータスバー型を実現したい場合は NSPanel ベースの実装が必要 (v0.2 検討)。
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Regular);

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
// Tray icon setup
// ---------------------------------------------------------------------------

/// メニューバーアイコンを作成する。
/// 失敗してもアプリは継続できるので、エラーはすべてログして無視する。
fn setup_tray(app: &tauri::App) {
    use tauri::{
        menu::{Menu, MenuItem, PredefinedMenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    };

    let show = match MenuItem::with_id(app, "show", "Show ClipNoteX", true, None::<&str>) {
        Ok(m) => m,
        Err(e) => { tracing::warn!(?e, "tray: failed to create show menu item"); return; }
    };
    let sep = match PredefinedMenuItem::separator(app) {
        Ok(s) => s,
        Err(e) => { tracing::warn!(?e, "tray: failed to create separator"); return; }
    };
    let quit = match MenuItem::with_id(app, "quit", "Quit ClipNoteX", true, None::<&str>) {
        Ok(m) => m,
        Err(e) => { tracing::warn!(?e, "tray: failed to create quit menu item"); return; }
    };
    let menu = match Menu::with_items(app, &[&show, &sep, &quit]) {
        Ok(m) => m,
        Err(e) => { tracing::warn!(?e, "tray: failed to create menu"); return; }
    };

    let icon = match app.default_window_icon().cloned() {
        Some(i) => i,
        None => { tracing::warn!("tray: no app icon available, skipping tray"); return; }
    };

    let result = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("ClipNoteX")
        // 左クリックはウィンドウ表示、右クリックはメニュー
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_history_window(app),
            "quit" => {
                tracing::info!("quit requested from tray menu");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_history_window(tray.app_handle());
            }
        })
        .build(app);

    match result {
        Ok(_) => tracing::info!("system tray icon created"),
        Err(e) => tracing::warn!(?e, "failed to create system tray icon"),
    }
}

/// 履歴ウィンドウを表示する (Clipy 風)。
/// マウスカーソル付近に配置し、検索入力できるようフォーカスを与える。
fn show_history_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("history") {
        position_window_near_cursor(&window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 履歴ウィンドウの表示/非表示をトグルする。
/// 注意: ActivationPolicy はトグルしない (起動時 Accessory のまま)。
fn toggle_history_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("history") else {
        tracing::error!("toggle: history window not found");
        return;
    };
    let visible = window.is_visible();
    let focused = window.is_focused();
    tracing::info!(?visible, ?focused, "toggle_history_window called");

    match visible {
        Ok(true) => {
            // 表示中だが裏に隠れているケース: hide ではなく前面に出す
            if matches!(focused, Ok(false)) {
                tracing::info!("visible but not focused → bringing to front");
                let _ = window.set_focus();
                return;
            }
            let r = window.hide();
            tracing::info!(?r, "hide called");
        }
        _ => {
            position_window_near_cursor(&window);
            let r_show = window.show();
            let r_focus = window.set_focus();
            tracing::info!(?r_show, ?r_focus, "show + set_focus called");
        }
    }
}

/// マウスカーソル付近にウィンドウを移動する (Clipy 風配置)。
/// 画面外にはみ出さないようクランプする。
fn position_window_near_cursor(window: &tauri::WebviewWindow) {
    use tauri::PhysicalPosition;

    let size = match window.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let cursor = match window.cursor_position() {
        Ok(p) => p,
        Err(_) => return,
    };
    let monitor = match window.current_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };

    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let mon_right = mon_pos.x + mon_size.width as i32;
    let mon_bottom = mon_pos.y + mon_size.height as i32;

    // カーソルの少し右下に出す (Clipy 風)
    let mut x = cursor.x as i32 + 8;
    let mut y = cursor.y as i32 + 8;

    // 画面外にはみ出さないようクランプ
    if x + size.width as i32 > mon_right {
        x = mon_right - size.width as i32 - 8;
    }
    if y + size.height as i32 > mon_bottom {
        y = mon_bottom - size.height as i32 - 8;
    }
    if x < mon_pos.x { x = mon_pos.x + 8; }
    if y < mon_pos.y { y = mon_pos.y + 8; }

    let _ = window.set_position(PhysicalPosition::new(x, y));
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
