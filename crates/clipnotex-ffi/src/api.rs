//! Public FFI surface — these symbols appear in `ClipNoteX.h` via cbindgen
//! and are linked by the Swift frontend.

use crate::errors::{set_last_error, take_last_error, CnxStatus};
use crate::runtime::{self, rt};
use crate::state::{self, try_state, LATE_WATCHER};
use crate::strings::{cstr_to_str, into_c_string};
use clipnotex_app::{run_capture_loop, QuotaManager};
use clipnotex_core::{
    bus::CoreEvent,
    ids::HotkeyId,
    model::{PayloadData, PayloadStorage},
    ClipId,
};
use clipnotex_donelog::{CaptureRequest, ContentKind, DoneOverlay, DoneView};
use clipnotex_format::FormatOptions;
use clipnotex_paste::{FormatRequest, PasteMode};
use serde::Serialize;
use std::os::raw::{c_char, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use ulid::Ulid;

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Initialize the ClipNoteX core.
/// `data_dir` UTF-8 path. `ephemeral_keys` non-zero means use in-memory keys
/// (dev mode); zero means use OS keychain.
///
/// Returns 0 on success, negative on error (see `cnx_last_error`).
#[no_mangle]
pub unsafe extern "C" fn cnx_init(data_dir: *const c_char, ephemeral_keys: i32) -> i32 {
    init_tracing_once();

    let path = match cstr_to_str(data_dir) {
        Some(s) => PathBuf::from(s),
        None => {
            set_last_error("cnx_init: data_dir is null or invalid UTF-8");
            return CnxStatus::InvalidArgument as i32;
        }
    };

    if let Err(e) = runtime::init() {
        set_last_error(e);
        return CnxStatus::InternalError as i32;
    }

    if let Err(e) = state::install(path, ephemeral_keys != 0) {
        set_last_error(e);
        return CnxStatus::StorageError as i32;
    }

    tracing::info!("ClipNoteX FFI initialized");
    CnxStatus::Ok as i32
}

/// Shutdown — currently a no-op (Tokio runtime + state are leaked at exit).
#[no_mangle]
pub extern "C" fn cnx_shutdown() {
    tracing::info!("cnx_shutdown");
}

/// Last error message for the calling thread. Returns null if none.
/// Caller MUST free with [`cnx_free_string`].
#[no_mangle]
pub extern "C" fn cnx_last_error() -> *mut c_char {
    match take_last_error() {
        Some(s) => into_c_string(s),
        None => std::ptr::null_mut(),
    }
}

/// Free a string previously returned by ClipNoteX FFI.
#[no_mangle]
pub unsafe extern "C" fn cnx_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(std::ffi::CString::from_raw(s));
    }
}

// ---------------------------------------------------------------------------
// Clipboard history
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct HistoryItemJson<'a> {
    id: String,
    created_at: i64,
    kind: String,
    source_app: &'a str,
    preview: String,
    pinned: bool,
}

/// List recent clipboard items.
/// `query` UTF-8 search string or null; `limit` max items (clamped to 200).
/// Returns JSON array; null on error.
#[no_mangle]
pub unsafe extern "C" fn cnx_list_history_json(
    query: *const c_char,
    limit: usize,
) -> *mut c_char {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return std::ptr::null_mut();
        }
    };
    let q = cstr_to_str(query).map(|s| s.to_string());
    let lim = limit.min(200).max(1);
    let items = match st.store.list_recent(lim, q.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("list_recent: {e}"));
            return std::ptr::null_mut();
        }
    };

    let summaries: Vec<HistoryItemJson> = items
        .iter()
        .map(|item| HistoryItemJson {
            id: item.id.to_string(),
            created_at: item.created_at,
            kind: format!("{:?}", item.primary_kind),
            source_app: &item.source_app.display_name,
            preview: item.text_preview.clone().unwrap_or_default(),
            pinned: item.pinned,
        })
        .collect();

    match serde_json::to_string(&summaries) {
        Ok(j) => into_c_string(j),
        Err(e) => {
            set_last_error(format!("serialize: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Paste an item by id.
/// `mode`: 0 = normal, 1 = plain, 2 = format (auto-detect), 3 = full.
#[no_mangle]
pub unsafe extern "C" fn cnx_paste_item(id: *const c_char, mode: i32) -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let id_str = match cstr_to_str(id) {
        Some(s) => s.to_string(),
        None => {
            set_last_error("paste_item: id null");
            return CnxStatus::InvalidArgument as i32;
        }
    };

    let store = st.store.clone();
    let paste = st.paste.clone();

    let result = rt().block_on(async move {
        // Locate item
        let item = tokio::task::spawn_blocking(move || {
            store
                .list_recent(200, None)
                .map(|v| v.into_iter().find(|i| i.id.to_string() == id_str))
        })
        .await
        .map_err(|e| format!("join: {e}"))?
        .map_err(|e| format!("lookup: {e}"))?
        .ok_or_else(|| "item not found".to_string())?;

        let mode = match mode {
            1 => PasteMode::Plain,
            2 => PasteMode::Format(FormatRequest {
                lang: None,
                opts: FormatOptions::default(),
            }),
            3 => PasteMode::Full,
            _ => PasteMode::Normal,
        };

        let payloads: Vec<PayloadData> = if item.payloads.is_empty() {
            let text = item
                .text_preview
                .clone()
                .ok_or_else(|| "no payload".to_string())?;
            vec![PayloadData {
                format_id: "public.utf8-plain-text".into(),
                bytes: text.into_bytes(),
            }]
        } else {
            item.payloads
                .iter()
                .filter_map(|p| match &p.storage {
                    PayloadStorage::Inline(bytes) => Some(PayloadData {
                        format_id: p.format_id.clone(),
                        bytes: bytes.clone(),
                    }),
                    _ => None,
                })
                .collect()
        };

        paste
            .paste(payloads, item.digest, mode)
            .await
            .map_err(|e| format!("paste: {e}"))
    });

    match result {
        Ok(_) => CnxStatus::Ok as i32,
        Err(e) => {
            set_last_error(e);
            CnxStatus::ClipboardError as i32
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cnx_pin_toggle(id: *const c_char) -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let id_str = match cstr_to_str(id) {
        Some(s) => s,
        None => {
            set_last_error("pin_toggle: id null");
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let cid = match id_str.parse::<Ulid>() {
        Ok(u) => ClipId(u),
        Err(e) => {
            set_last_error(format!("parse id: {e}"));
            return CnxStatus::InvalidArgument as i32;
        }
    };
    match st.store.pin_toggle(cid) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => {
            set_last_error(format!("pin_toggle: {e}"));
            CnxStatus::StorageError as i32
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cnx_delete_item(id: *const c_char) -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let id_str = match cstr_to_str(id) {
        Some(s) => s,
        None => {
            set_last_error("delete_item: id null");
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let cid = match id_str.parse::<Ulid>() {
        Ok(u) => ClipId(u),
        Err(e) => {
            set_last_error(format!("parse id: {e}"));
            return CnxStatus::InvalidArgument as i32;
        }
    };
    match st.store.delete_item(cid) {
        Ok(_) => CnxStatus::Ok as i32,
        Err(e) => {
            set_last_error(format!("delete_item: {e}"));
            CnxStatus::StorageError as i32
        }
    }
}

// ---------------------------------------------------------------------------
// DONE LOG
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DoneItemJson {
    id: String,
    date: String,
    time: String,
    source_app: String,
    kind: String,
    body: String,
    note: Option<String>,
    tags: Vec<String>,
}

impl From<DoneView> for DoneItemJson {
    fn from(v: DoneView) -> Self {
        let body = v.effective_body().to_string();
        let note = v.note().map(|s| s.to_string());
        let tags = v.tags().to_vec();
        Self {
            id: v.entry.id.to_string(),
            date: v.entry.date.format("%Y-%m-%d").to_string(),
            time: v.entry.time.format("%H:%M").to_string(),
            source_app: v.entry.source_app.display_name.clone(),
            kind: format!("{:?}", v.entry.kind),
            body,
            note,
            tags,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cnx_capture_done(body: *const c_char) -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let body_str = match cstr_to_str(body) {
        Some(s) => s.to_string(),
        None => {
            set_last_error("capture_done: body null");
            return CnxStatus::InvalidArgument as i32;
        }
    };

    use clipnotex_core::model::SourceApp;
    let id = ClipId(Ulid::new());
    let captured_at = chrono::Utc::now().timestamp_millis();
    let source_app = SourceApp {
        bundle_id: None,
        exe_basename: None,
        exe_path: None,
        display_name: "Manual".into(),
        window_title: None,
    };

    match st.donelog.capture(CaptureRequest {
        id,
        captured_at,
        source_app,
        kind: ContentKind::Text,
        body: body_str,
        attachment: None,
    }) {
        Ok(_) => CnxStatus::Ok as i32,
        Err(e) => {
            set_last_error(format!("capture_done: {e}"));
            CnxStatus::StorageError as i32
        }
    }
}

/// `date` is "YYYY-MM-DD" or null (= recent across all dates).
#[no_mangle]
pub unsafe extern "C" fn cnx_list_done_json(date: *const c_char, limit: usize) -> *mut c_char {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return std::ptr::null_mut();
        }
    };
    let lim = limit.min(200).max(1);

    let res = if let Some(d) = cstr_to_str(date) {
        match chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => st.donelog.list_by_date(date),
            Err(e) => {
                set_last_error(format!("invalid date: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        st.donelog.list_recent(lim)
    };

    let views = match res {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("list_done: {e}"));
            return std::ptr::null_mut();
        }
    };
    let items: Vec<DoneItemJson> = views.into_iter().map(Into::into).collect();
    match serde_json::to_string(&items) {
        Ok(j) => into_c_string(j),
        Err(e) => {
            set_last_error(format!("serialize: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cnx_delete_done(id: *const c_char) -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let id_str = match cstr_to_str(id) {
        Some(s) => s,
        None => {
            set_last_error("delete_done: id null");
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let cid = match id_str.parse::<Ulid>() {
        Ok(u) => ClipId(u),
        Err(e) => {
            set_last_error(format!("parse id: {e}"));
            return CnxStatus::InvalidArgument as i32;
        }
    };
    match st.donelog.delete(cid) {
        Ok(_) => CnxStatus::Ok as i32,
        Err(e) => {
            set_last_error(format!("delete_done: {e}"));
            CnxStatus::StorageError as i32
        }
    }
}

/// Update a DONE LOG entry's overlay.
/// `args_json` schema:
///   { "id": "...", "note": "..."|null, "body": "..."|null,
///     "add_tags": [...], "remove_tags": [...] }
#[no_mangle]
pub unsafe extern "C" fn cnx_update_done_overlay_json(args_json: *const c_char) -> i32 {
    #[derive(serde::Deserialize)]
    struct Args {
        id: String,
        note: Option<String>,
        body: Option<String>,
        #[serde(default)]
        add_tags: Vec<String>,
        #[serde(default)]
        remove_tags: Vec<String>,
    }

    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let json = match cstr_to_str(args_json) {
        Some(s) => s,
        None => {
            set_last_error("args_json null");
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let args: Args = match serde_json::from_str(json) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("parse args: {e}"));
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let cid = match args.id.parse::<Ulid>() {
        Ok(u) => ClipId(u),
        Err(e) => {
            set_last_error(format!("parse id: {e}"));
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let view = match st.donelog.get(cid) {
        Ok(Some(v)) => v,
        Ok(None) => {
            set_last_error("entry not found");
            return CnxStatus::InvalidArgument as i32;
        }
        Err(e) => {
            set_last_error(format!("get: {e}"));
            return CnxStatus::StorageError as i32;
        }
    };

    let mut overlay: DoneOverlay = view.overlay.clone();
    if let Some(n) = args.note {
        overlay.set_note(n);
    }
    if let Some(b) = args.body {
        overlay.set_body(b);
    }
    for t in args.add_tags {
        overlay.add_tag(t);
    }
    for t in &args.remove_tags {
        overlay.remove_tag(t);
    }
    if let Err(e) = st.donelog.update_overlay(cid, &overlay) {
        set_last_error(format!("update_overlay: {e}"));
        return CnxStatus::StorageError as i32;
    }
    CnxStatus::Ok as i32
}

#[no_mangle]
pub unsafe extern "C" fn cnx_export_done_markdown(date: *const c_char) -> *mut c_char {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return std::ptr::null_mut();
        }
    };
    let d = if let Some(s) = cstr_to_str(date) {
        match chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                set_last_error(format!("invalid date: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        chrono::Local::now().date_naive()
    };
    let views = match st.donelog.list_by_date(d) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("list_by_date: {e}"));
            return std::ptr::null_mut();
        }
    };
    let md = clipnotex_donelog::export::to_markdown(d, &views);
    into_c_string(md)
}

// ---------------------------------------------------------------------------
// Hotkeys
// ---------------------------------------------------------------------------

/// Hotkey id constants (mirrored in C header).
#[repr(i32)]
pub enum CnxHotkeyId {
    ShowHistory = 1,
    ShowSnippets = 2,
    PastePlain = 3,
    PasteFormat = 4,
    PasteFull = 5,
    DoneCapture = 6,
}

fn map_hk(id: i32) -> Option<HotkeyId> {
    match id {
        1 => Some(HotkeyId::ShowHistory),
        2 => Some(HotkeyId::ShowSnippets),
        3 => Some(HotkeyId::PastePlain),
        4 => Some(HotkeyId::PasteFormat),
        5 => Some(HotkeyId::PasteFull),
        6 => Some(HotkeyId::DoneCapture),
        _ => None,
    }
}
fn unmap_hk(id: HotkeyId) -> i32 {
    match id {
        HotkeyId::ShowHistory => 1,
        HotkeyId::ShowSnippets => 2,
        HotkeyId::PastePlain => 3,
        HotkeyId::PasteFormat => 4,
        HotkeyId::PasteFull => 5,
        HotkeyId::DoneCapture => 6,
    }
}

/// Register a hotkey. `accelerator` is e.g. "Cmd+Shift+V".
#[no_mangle]
pub unsafe extern "C" fn cnx_register_hotkey(id: i32, accelerator: *const c_char) -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    let svc = match &st.hotkey {
        Some(s) => s.clone(),
        None => {
            set_last_error("hotkey service unavailable");
            return CnxStatus::HotkeyError as i32;
        }
    };
    let hk = match map_hk(id) {
        Some(h) => h,
        None => {
            set_last_error(format!("unknown hotkey id {id}"));
            return CnxStatus::InvalidArgument as i32;
        }
    };
    let acc = match cstr_to_str(accelerator) {
        Some(s) => s.to_string(),
        None => {
            set_last_error("accelerator null");
            return CnxStatus::InvalidArgument as i32;
        }
    };

    for r in svc.register_all(&[(hk, acc.clone())]) {
        if let Err(f) = r.outcome {
            set_last_error(format!("register {}: {}", r.accelerator, f.reason));
            return CnxStatus::HotkeyError as i32;
        }
    }
    CnxStatus::Ok as i32
}

/// Drain pending hotkey OS events. Call this from the main thread runloop
/// (Swift: `Timer.scheduledTimer(withTimeInterval: 0.05, ...)` or a CFRunLoop source).
#[no_mangle]
pub extern "C" fn cnx_hotkey_pump() {
    if let Some(st) = state::state() {
        if let Some(svc) = &st.hotkey {
            svc.pump();
        }
    }
}

// Event callbacks ----------------------------------------------------------

/// C callback signature: `fn(hotkey_id: i32, ctx: *mut c_void)`.
pub type CnxHotkeyCallback = Option<unsafe extern "C" fn(i32, *mut c_void)>;
/// C callback signature: `fn(ctx: *mut c_void)` — fired on each clipboard capture.
pub type CnxCaptureCallback = Option<unsafe extern "C" fn(*mut c_void)>;

static HOTKEY_CB: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static HOTKEY_CTX: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static CAPTURE_CB: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static CAPTURE_CTX: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static LISTENER_STARTED: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub unsafe extern "C" fn cnx_set_hotkey_callback(cb: CnxHotkeyCallback, ctx: *mut c_void) {
    HOTKEY_CB.store(
        cb.map(|f| f as *mut ()).unwrap_or(std::ptr::null_mut()),
        Ordering::SeqCst,
    );
    HOTKEY_CTX.store(ctx, Ordering::SeqCst);
    spawn_event_listener_once();
}

#[no_mangle]
pub unsafe extern "C" fn cnx_set_capture_callback(cb: CnxCaptureCallback, ctx: *mut c_void) {
    CAPTURE_CB.store(
        cb.map(|f| f as *mut ()).unwrap_or(std::ptr::null_mut()),
        Ordering::SeqCst,
    );
    CAPTURE_CTX.store(ctx, Ordering::SeqCst);
    spawn_event_listener_once();
}

fn spawn_event_listener_once() {
    if LISTENER_STARTED.swap(1, Ordering::SeqCst) != 0 {
        return;
    }
    let Some(st) = state::state() else { return };
    let mut rx = st.bus.subscribe();
    rt().spawn(async move {
        while let Ok(ev) = rx.recv().await {
            match ev {
                CoreEvent::HotkeyPressed(hk) => {
                    let cb_ptr = HOTKEY_CB.load(Ordering::SeqCst);
                    let ctx = HOTKEY_CTX.load(Ordering::SeqCst);
                    if !cb_ptr.is_null() {
                        let cb: unsafe extern "C" fn(i32, *mut c_void) =
                            unsafe { std::mem::transmute(cb_ptr) };
                        unsafe { cb(unmap_hk(hk), ctx) };
                    }
                }
                CoreEvent::ClipboardCaptured(_) => {
                    let cb_ptr = CAPTURE_CB.load(Ordering::SeqCst);
                    let ctx = CAPTURE_CTX.load(Ordering::SeqCst);
                    if !cb_ptr.is_null() {
                        let cb: unsafe extern "C" fn(*mut c_void) =
                            unsafe { std::mem::transmute(cb_ptr) };
                        unsafe { cb(ctx) };
                    }
                }
                _ => {}
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Capture loop
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn cnx_start_capture_loop() -> i32 {
    let st = match try_state() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return CnxStatus::NotInitialized as i32;
        }
    };
    {
        let mut flag = st.watcher_started.lock();
        if *flag {
            return CnxStatus::Ok as i32; // idempotent
        }
        *flag = true;
    }
    let watcher = match LATE_WATCHER.get().and_then(|m| m.lock().take()) {
        Some(w) => w,
        None => {
            set_last_error("watcher not available");
            return CnxStatus::ClipboardError as i32;
        }
    };
    let filter = st.filter.clone();
    let store = st.store.clone();
    let bus = st.bus.clone();

    // Quota enforcement subscriber
    let quota = QuotaManager::new(st.store.clone(), st.settings.lock().history.clone());
    let _ = quota.enforce();
    {
        let quota2 = quota.clone();
        let mut rx = st.bus.subscribe();
        rt().spawn(async move {
            while let Ok(ev) = rx.recv().await {
                if let CoreEvent::ClipboardCaptured(_) = ev {
                    let q = quota2.clone();
                    tokio::task::spawn_blocking(move || {
                        let _ = q.enforce();
                    });
                }
            }
        });
    }

    rt().spawn(async move {
        if let Err(e) = run_capture_loop(watcher, filter, store, bus).await {
            tracing::error!(?e, "capture loop exited");
        }
    });

    spawn_event_listener_once();
    CnxStatus::Ok as i32
}

// ---------------------------------------------------------------------------
// Tracing
// ---------------------------------------------------------------------------

fn init_tracing_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let filter = std::env::var("RUST_LOG")
            .unwrap_or_else(|_| "clipnotex=debug,warn".into());
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init();
    });
}
