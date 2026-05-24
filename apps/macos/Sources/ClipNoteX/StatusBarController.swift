// StatusBarController — メニューバーアイコン + 動作制御。
//
// UX (v0.2):
//   - 左クリック / ホットキー  → SearchPanel (検索可能なポップアップ)
//   - 右クリック              → 簡易メニュー (DONE LOG / Quit)
//   - ⌘⇧V                      → SearchPanel をトグル
//   - ⌘⇧D                      → 現在のクリップボードを DONE LOG にキャプチャ

import AppKit
import Foundation
import ClipNoteXCore

final class StatusBarController {
    let statusItem: NSStatusItem

    init() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let btn = statusItem.button {
            btn.image = NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: "ClipNoteX")
            btn.image?.isTemplate = true
            btn.target = self
            btn.action = #selector(buttonClicked(_:))
            btn.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        NotificationCenter.default.addObserver(
            forName: .cnxHotkeyPressed,
            object: nil,
            queue: .main
        ) { [weak self] note in
            let id = note.userInfo?["id"] as? Int32 ?? 0
            switch id {
            case 1: self?.toggleSearchPanel() // ShowHistory (Cmd+Shift+V) → toggle
            case 6: self?.captureCurrentClipboardAsDoneEntry() // Cmd+Shift+D
            default: break
            }
        }
    }

    // MARK: - Button click handling

    @objc private func buttonClicked(_ sender: NSStatusBarButton) {
        let event = NSApp.currentEvent
        if event?.type == .rightMouseUp {
            showContextMenu()
        } else {
            // ステータスアイコン左クリックもトグル動作 (同じアイコンを再度押したら閉じる)
            toggleSearchPanel()
        }
    }

    func openSearchPanel() {
        SearchPanel.shared.show(near: statusItem)
    }

    /// グローバルホットキー / ステータスアイコンクリック共通のトグル動作。
    func toggleSearchPanel() {
        _ = SearchPanel.shared.toggle(near: statusItem)
    }

    private func showContextMenu() {
        let menu = NSMenu()
        let donelog = NSMenuItem(title: "Open DONE LOG…", action: #selector(openDoneLog), keyEquivalent: "")
        donelog.target = self
        menu.addItem(donelog)
        menu.addItem(.separator())
        let prefs = NSMenuItem(title: "Preferences…", action: #selector(openPreferences), keyEquivalent: ",")
        prefs.target = self
        menu.addItem(prefs)
        let about = NSMenuItem(title: "About ClipNoteX", action: #selector(showAbout), keyEquivalent: "")
        about.target = self
        menu.addItem(about)
        menu.addItem(.separator())
        let quit = NSMenuItem(title: "Quit ClipNoteX", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        menu.addItem(quit)

        // 一時的に menu を割り当ててから popUp、終わったら外す (button.action と両立)
        statusItem.menu = menu
        statusItem.button?.performClick(nil)
        statusItem.menu = nil
    }

    @objc private func openDoneLog() {
        DoneLogWindow.show()
    }

    @objc private func openPreferences() {
        PreferencesWindow.show()
    }

    @objc private func showAbout() {
        NSApp.activate(ignoringOtherApps: true)
        NSApp.orderFrontStandardAboutPanel(self)
    }

    // MARK: - DONE LOG capture

    fileprivate func captureCurrentClipboardAsDoneEntry() {
        let pb = NSPasteboard.general
        guard let text = pb.string(forType: .string) else { return }
        let r = text.withCString { cstr in cnx_capture_done(cstr) }
        if r != 0, let msg = cnx_last_error() {
            NSLog("capture_done failed: \(String(cString: msg))")
            cnx_free_string(msg)
        } else {
            // 小さなトーストの代わりにアイコンを一時的に強調
            flashIcon()
        }
    }

    private func flashIcon() {
        guard let btn = statusItem.button else { return }
        let original = btn.image
        btn.image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Captured")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) {
            btn.image = original
        }
    }
}

// MARK: - HistoryItem model (used by SearchPanel & misc)

struct HistoryItem: Decodable {
    let id: String
    let created_at: Int64
    let kind: String
    let source_app: String
    let preview: String
    let pinned: Bool
}
