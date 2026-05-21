// StatusBarController — メニューバーアイコン + クリップボード履歴ポップアップ。
//
// Clipy / Maccy 同様、`NSStatusItem` の menu に `NSMenu` を割り当てる。
// メニューはアプリをアクティブ化せず表示され、選択項目が決まったら閉じる。
// ペーストは Rust 側で実行 (元のアプリにフォーカスが残っているので Cmd+V が正しく飛ぶ)。

import AppKit
import ClipNoteXCore
import Foundation

final class StatusBarController {
    private let statusItem: NSStatusItem
    private let menu = NSMenu()
    private var historyItems: [HistoryItem] = []

    init() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let btn = statusItem.button {
            btn.image = NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: "ClipNoteX")
            btn.image?.isTemplate = true
        }
        statusItem.menu = menu
        menu.delegate = MenuDelegate.shared
        MenuDelegate.shared.controller = self

        NotificationCenter.default.addObserver(
            forName: .cnxHotkeyPressed,
            object: nil,
            queue: .main
        ) { [weak self] note in
            guard let self else { return }
            let id = note.userInfo?["id"] as? Int32 ?? 0
            switch id {
            case 1: self.openHistoryMenu() // ShowHistory
            case 6: self.captureCurrentClipboardAsDoneEntry() // DoneCapture
            default: break
            }
        }
    }

    /// メニューバーアイコンの位置でメニューを開く (ホットキー時)
    func openHistoryMenu() {
        // statusItem.menu を popUp で開くと、現在のフォーカスを変えない。
        if let button = statusItem.button {
            statusItem.menu = menu // ensure attached
            button.performClick(nil)
        }
    }

    /// メニュー開く直前に呼ばれる: 最新の履歴で再構築
    func rebuildMenu() {
        menu.removeAllItems()

        let json = cnx_list_history_json(nil, 30)
        defer { if json != nil { cnx_free_string(json) } }
        guard let json,
              let str = String(validatingUTF8: json),
              let data = str.data(using: .utf8) else {
            menu.addItem(NSMenuItem(title: "(no clipboard history)", action: nil, keyEquivalent: ""))
            return
        }

        do {
            historyItems = try JSONDecoder().decode([HistoryItem].self, from: data)
        } catch {
            menu.addItem(NSMenuItem(title: "decode error: \(error)", action: nil, keyEquivalent: ""))
            return
        }

        if historyItems.isEmpty {
            menu.addItem(NSMenuItem(title: "(history is empty)", action: nil, keyEquivalent: ""))
        } else {
            for (idx, item) in historyItems.enumerated() {
                let title = item.preview
                    .replacingOccurrences(of: "\n", with: " ")
                    .prefix(80)
                let mi = NSMenuItem(
                    title: "\(idx + 1). \(title)",
                    action: #selector(MenuDelegate.didSelectHistory(_:)),
                    keyEquivalent: idx < 9 ? "\(idx + 1)" : ""
                )
                mi.target = MenuDelegate.shared
                mi.representedObject = item.id
                if item.pinned {
                    mi.attributedTitle = NSAttributedString(
                        string: "📌 " + mi.title,
                        attributes: [.font: NSFont.menuBarFont(ofSize: 0)]
                    )
                }
                menu.addItem(mi)
            }
        }

        menu.addItem(.separator())
        let donelogItem = NSMenuItem(
            title: "DONE LOG…",
            action: #selector(MenuDelegate.openDoneLog),
            keyEquivalent: "d"
        )
        donelogItem.keyEquivalentModifierMask = [.command, .shift]
        donelogItem.target = MenuDelegate.shared
        menu.addItem(donelogItem)

        menu.addItem(.separator())
        let quit = NSMenuItem(title: "Quit ClipNoteX", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        menu.addItem(quit)
    }

    fileprivate func pasteItem(id: String) {
        let r = id.withCString { cstr in
            cnx_paste_item(cstr, /* mode: normal */ 0)
        }
        if r != 0, let msg = cnx_last_error() {
            NSLog("paste_item failed: \(String(cString: msg))")
            cnx_free_string(msg)
        }
    }

    fileprivate func captureCurrentClipboardAsDoneEntry() {
        let pb = NSPasteboard.general
        guard let text = pb.string(forType: .string) else { return }
        let r = text.withCString { cstr in cnx_capture_done(cstr) }
        if r != 0, let msg = cnx_last_error() {
            NSLog("capture_done failed: \(String(cString: msg))")
            cnx_free_string(msg)
        }
    }
}

/// メニューの delegate / action target は単一インスタンスで集約。
final class MenuDelegate: NSObject, NSMenuDelegate {
    static let shared = MenuDelegate()
    weak var controller: StatusBarController?

    func menuWillOpen(_ menu: NSMenu) {
        controller?.rebuildMenu()
    }

    @objc func didSelectHistory(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        controller?.pasteItem(id: id)
    }

    @objc func openDoneLog() {
        DoneLogWindow.show()
    }
}

struct HistoryItem: Decodable {
    let id: String
    let created_at: Int64
    let kind: String
    let source_app: String
    let preview: String
    let pinned: Bool
}
