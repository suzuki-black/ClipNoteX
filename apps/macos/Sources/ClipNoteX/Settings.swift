// Settings.swift — User-facing preferences persisted via NSUserDefaults.
//
// Single source of truth for any UI/runtime setting the user can change.
// On change, we (a) save to UserDefaults and (b) push the new value into
// the Rust core via the corresponding cnx_* FFI.

import Foundation
import ServiceManagement
import ClipNoteXCore

enum Settings {

    // MARK: - Keys

    private enum Key {
        static let historyQuota = "history.maxItems"
        static let launchAtLogin = "app.launchAtLogin"
    }

    // MARK: - History quota

    static var historyQuota: Int {
        get {
            let v = UserDefaults.standard.integer(forKey: Key.historyQuota)
            return v > 0 ? v : 1000
        }
        set {
            let clamped = max(50, min(50_000, newValue))
            UserDefaults.standard.set(clamped, forKey: Key.historyQuota)
            _ = cnx_set_history_quota(UInt64(clamped))
        }
    }

    // MARK: - Launch at login (LSUIElement compatible)

    static var launchAtLogin: Bool {
        get { UserDefaults.standard.bool(forKey: Key.launchAtLogin) }
        set {
            UserDefaults.standard.set(newValue, forKey: Key.launchAtLogin)
            applyLaunchAtLogin(newValue)
        }
    }

    private static func applyLaunchAtLogin(_ enable: Bool) {
        // macOS 13+: SMAppService.mainApp
        if #available(macOS 13.0, *) {
            let svc = SMAppService.mainApp
            do {
                if enable {
                    if svc.status != .enabled {
                        try svc.register()
                    }
                } else {
                    if svc.status == .enabled {
                        try svc.unregister()
                    }
                }
            } catch {
                NSLog("launch-at-login: \(error)")
            }
        }
    }

    // MARK: - Push everything to the Rust core (called on app startup)

    static func pushToCore() {
        _ = cnx_set_history_quota(UInt64(historyQuota))
        // 起動時に LSUIElement の登録状態を反映 (UserDefaults と SMAppService の同期)
        applyLaunchAtLogin(launchAtLogin)
    }
}
