// AppDelegate — アプリのライフサイクル管理。

import AppKit
import ClipNoteXCore
import Foundation

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusController: StatusBarController?
    private var hotkeyTimer: Timer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // 1) Rust コア初期化
        let dataDir = Self.appSupportDir().path
        // Ephemeral keys (in-memory) only when CLIPNOTEX_EPHEMERAL=1.
        // Default is to use the macOS Keychain so history survives restarts.
        let ephemeral: Int32 =
            (ProcessInfo.processInfo.environment["CLIPNOTEX_EPHEMERAL"] == "1") ? 1 : 0
        let status = dataDir.withCString { cstr in
            cnx_init(cstr, ephemeral)
        }
        guard status == 0 else {
            NSLog("ClipNoteX: cnx_init failed (status=\(status))")
            if let msg = cnx_last_error() {
                NSLog("  reason: \(String(cString: msg))")
                cnx_free_string(msg)
            }
            NSApp.terminate(nil)
            return
        }
        NSLog("ClipNoteX: core initialized at \(dataDir)")

        // 2) 永続化設定 (UserDefaults) を core に push
        Settings.pushToCore()

        // 3) クリップボード監視ループ開始
        _ = cnx_start_capture_loop()

        // 3) ホットキー (バックグラウンドからの呼び出し可)
        Settings.registerHotkeys()
        setHotkeyCallback()

        // global-hotkey は内部チャネルを持っていて pump() を定期的に呼ぶ必要がある。
        // メインスレッドの Timer で 50ms 毎に叩く。
        hotkeyTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { _ in
            cnx_hotkey_pump()
        }
        RunLoop.main.add(hotkeyTimer!, forMode: .common)

        // 4) ステータスバー UI
        statusController = StatusBarController()
    }

    private func setHotkeyCallback() {
        // 静的 C 関数 (キャプチャ不可) を渡す。コールバック内では即メインキューに dispatch。
        cnx_set_hotkey_callback({ (hotkeyId: Int32, _: UnsafeMutableRawPointer?) in
            DispatchQueue.main.async {
                NotificationCenter.default.post(
                    name: .cnxHotkeyPressed,
                    object: nil,
                    userInfo: ["id": hotkeyId]
                )
            }
        }, nil)
    }

    private static func appSupportDir() -> URL {
        let fm = FileManager.default
        let base = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let dir = base.appendingPathComponent("com.clipnotex.app", isDirectory: true)
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }
}

extension Notification.Name {
    /// userInfo["id"] = Int32 (CnxHotkeyId)
    static let cnxHotkeyPressed = Notification.Name("CnxHotkeyPressed")
}
