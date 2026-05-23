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
        static let historyQuota   = "history.maxItems"
        static let launchAtLogin  = "app.launchAtLogin"
        static let showHistoryHK  = "hotkey.showHistory"   // String accel e.g. "Cmd+Shift+V"
        static let doneCaptureHK  = "hotkey.doneCapture"
        static let exclusionsJSON = "exclusions.json"      // JSON array of ExclusionRule
    }

    // MARK: - Defaults

    static let defaultShowHistoryHK  = "Cmd+Shift+V"
    static let defaultDoneCaptureHK  = "Cmd+Shift+D"
    static let defaultExclusionsJSON = #"""
    [
      {"match":"bundle_id","value":"com.1password.1password"},
      {"match":"bundle_id","value":"com.bitwarden.desktop"},
      {"match":"bundle_id","value":"org.keepassxc.keepassxc"}
    ]
    """#

    // MARK: - Hotkeys

    static var showHistoryHotkey: String {
        get { UserDefaults.standard.string(forKey: Key.showHistoryHK) ?? defaultShowHistoryHK }
        set { UserDefaults.standard.set(newValue, forKey: Key.showHistoryHK) }
    }

    static var doneCaptureHotkey: String {
        get { UserDefaults.standard.string(forKey: Key.doneCaptureHK) ?? defaultDoneCaptureHK }
        set { UserDefaults.standard.set(newValue, forKey: Key.doneCaptureHK) }
    }

    // MARK: - Exclusions

    static var exclusionsJSON: String {
        get { UserDefaults.standard.string(forKey: Key.exclusionsJSON) ?? defaultExclusionsJSON }
        set {
            UserDefaults.standard.set(newValue, forKey: Key.exclusionsJSON)
            applyExclusionsJSON(newValue)
        }
    }

    private static func applyExclusionsJSON(_ json: String) {
        json.withCString { _ = cnx_set_exclusions_json($0) }
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
        applyLaunchAtLogin(launchAtLogin)
        applyExclusionsJSON(exclusionsJSON)
        // ホットキーは AppDelegate がこの後 registerHotkeys() で登録するので
        // ここでは何もしない (UserDefaults からの読み出しは register 側で行う)
    }

    /// Re-register every hotkey from current UserDefaults values.
    /// Called from AppDelegate on startup and from Preferences after change.
    static func registerHotkeys() {
        _ = cnx_clear_hotkeys()
        // ShowHistory = id 1, DoneCapture = id 6 (see CnxHotkeyId in FFI)
        showHistoryHotkey.withCString { _ = cnx_register_hotkey(1, $0) }
        doneCaptureHotkey.withCString { _ = cnx_register_hotkey(6, $0) }
    }
}
