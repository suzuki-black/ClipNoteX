//! Global app state. Initialized once in [`crate::api::cnx_init`].
//!
//! Holds every long-lived service the FFI exposes:
//!   - [`StoreService`] : encrypted clipboard history
//!   - [`DoneLogStore`] : DONE LOG entries
//!   - [`ExclusionFilter`] : password-manager block list
//!   - [`SelfWriteGuard`] : prevents capturing our own paste writes
//!   - clipboard writer (Arc<dyn ClipboardWriter>)
//!   - [`PasteController`]
//!   - [`HotkeyService`]
//!   - [`EventBus`] : cross-component event channel

use clipnotex_app::ExclusionFilter;
use clipnotex_clipboard::{ClipboardWatcher, ClipboardWriter, SelfWriteGuard};
use clipnotex_core::{bus::EventBus, settings::Settings};
use clipnotex_donelog::DoneLogStore;
use clipnotex_hotkey::HotkeyService;
use clipnotex_paste::PasteController;
use clipnotex_store::{KeySource, StoreService};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) struct AppState {
    pub store: Arc<StoreService>,
    pub donelog: Arc<DoneLogStore>,
    pub filter: Arc<ExclusionFilter>,
    pub guard: Arc<SelfWriteGuard>,
    pub paste: Arc<PasteController>,
    pub hotkey: Option<Arc<HotkeyService>>,
    pub bus: EventBus,
    pub settings: Mutex<Settings>,
    /// Set once when the capture loop is started; used to detect double-start.
    pub watcher_started: Mutex<bool>,
}

static STATE: OnceCell<AppState> = OnceCell::new();

pub(crate) fn state() -> Option<&'static AppState> {
    STATE.get()
}

pub(crate) fn try_state() -> Result<&'static AppState, String> {
    STATE
        .get()
        .ok_or_else(|| "cnx_init() not called or failed".to_string())
}

pub(crate) fn install(data_dir: PathBuf, ephemeral_keys: bool) -> Result<(), String> {
    if STATE.get().is_some() {
        return Ok(()); // idempotent
    }

    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("create data dir {}: {}", data_dir.display(), e))?;

    let make_key_source = || -> KeySource {
        if ephemeral_keys {
            KeySource::Ephemeral
        } else {
            KeySource::Keyring {
                service: "ClipNoteX".into(),
                account: "data_key".into(),
            }
        }
    };

    let store = StoreService::open(data_dir.clone(), make_key_source())
        .map(Arc::new)
        .map_err(|e| format!("open store: {e}"))?;

    let donelog_key = make_key_source();
    let donelog = DoneLogStore::open(data_dir.join("donelog"), &donelog_key)
        .map(Arc::new)
        .map_err(|e| format!("open donelog: {e}"))?;

    let settings = Settings::default();
    let bus = EventBus::new(256);
    let filter = ExclusionFilter::new(settings.exclusions.clone(), true);
    let guard = Arc::new(SelfWriteGuard::new(std::time::Duration::from_secs(5)));

    let (_watcher, writer) = clipnotex_clipboard::open(guard.clone())
        .map_err(|e| format!("open clipboard: {e}"))?;
    let writer: Arc<dyn ClipboardWriter> = Arc::from(writer);
    let paste = Arc::new(PasteController::new(writer, guard.clone()));

    // Hotkeys: non-fatal — frontend can still operate the menu directly.
    let hotkey = match HotkeyService::new(bus.clone()) {
        Ok(svc) => Some(svc),
        Err(e) => {
            tracing::warn!(?e, "hotkey service unavailable — global shortcuts disabled");
            None
        }
    };

    let watcher_holder: Mutex<Option<Box<dyn ClipboardWatcher>>> = Mutex::new(Some(_watcher));
    // The watcher is needed when the caller starts the capture loop.
    // Store it in a private global so api::cnx_start_capture_loop can take it.
    LATE_WATCHER.set(watcher_holder).ok();

    STATE
        .set(AppState {
            store,
            donelog,
            filter,
            guard,
            paste,
            hotkey,
            bus,
            settings: Mutex::new(settings),
            watcher_started: Mutex::new(false),
        })
        .map_err(|_| "state already installed".to_string())?;

    Ok(())
}

/// Watcher is held aside until the caller starts the capture loop.
pub(crate) static LATE_WATCHER: OnceCell<Mutex<Option<Box<dyn ClipboardWatcher>>>> =
    OnceCell::new();
