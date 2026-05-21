// ClipNoteX — macOS native shell.
//
// アーキテクチャ:
//   - NSStatusItem  : メニューバーアイコン
//   - NSMenu        : クリップボード履歴のポップアップ (Clipy / Maccy と同型)
//   - DONE LOG       : 別ウィンドウ (NSWindow)
//   - Rust FFI       : ClipNoteXCore モジュール経由で呼び出す
//
// Rust 側がアプリをアクティブ化せずペーストを叩く前提なので、メインスレッドの
// NSMenu イベントループはそのまま使ってよい。NSPanel 等のトリッキーな仕掛けは不要。

import AppKit
import ClipNoteXCore

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // メニューバー常駐 (Dock 非表示)

app.run()
