//! Tauri command surface + composition root for ClipNoteX.
//!
//! `run()` は apps/desktop/src-tauri/src/lib.rs から呼ばれる。
//! `#[tauri::command]` と `generate_handler!` を同一クレートに置くことで
//! proc-macro の生成コードが正しく解決される。

pub mod commands;
pub mod state;
pub mod setup;

pub use state::AppState;

/// Entry point called from apps/desktop/src-tauri.
pub fn run() {
    setup::init_tracing();

    tauri::Builder::default()
        .setup(|app| setup::compose(app))
        // ×ボタンはアプリを終了させない — ウィンドウを隠すだけ。
        // トレイアイコンまたは「Quit ClipNoteX」メニューで終了する。
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_history,
            commands::paste_item,
            commands::pin_toggle,
            commands::delete_item,
            commands::enable_input_focus,
            // Format paste
            commands::format_preview,
            commands::detect_lang,
            // DONE LOG
            commands::capture_done,
            commands::list_done,
            commands::update_done_overlay,
            commands::export_done_markdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
