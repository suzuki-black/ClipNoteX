// SearchPanel — 検索可能なクリップボード履歴ポップアップ。
//
// 設計:
//   - `NSPanel` + `.nonactivatingPanel` style mask
//     → アプリをアクティブ化せずキー入力を受け取れる
//     → 直前のフロントアプリは frontmost のままなので、Cmd+V を合成すれば
//       元のアプリにペーストされる (= Maccy / Alfred と同型)
//   - 上部にサーチフィールド、下に NSTableView (フィルタ済みリスト)
//   - ↑↓ で選択、⏎ でペースト、1〜9 で即ペースト、Esc で閉じる
//   - クリック外し / アプリ切替で自動的に隠れる
//
// FFI: 文字入力毎に `cnx_list_history_json(query, limit)` を呼んで再描画。

import AppKit
import Foundation
import ClipNoteXCore

final class SearchPanel: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {

    static let shared = SearchPanel()

    private let searchField = NSSearchField()
    private let table = NSTableView()
    private var items: [HistoryItem] = []

    private init() {
        let panel = NonActivatingPanel(
            contentRect: NSRect(x: 0, y: 0, width: 480, height: 520),
            styleMask: [.titled, .closable, .nonactivatingPanel, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        panel.title = "ClipNoteX"
        panel.titleVisibility = .hidden
        panel.titlebarAppearsTransparent = true
        panel.isMovableByWindowBackground = true
        panel.level = .floating
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = false
        panel.isFloatingPanel = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        super.init(window: panel)
        panel.delegate = self
        setupUI()
    }
    required init?(coder: NSCoder) { fatalError() }

    // MARK: - UI

    private func setupUI() {
        guard let content = window?.contentView else { return }

        searchField.placeholderString = "Search clipboard…"
        searchField.delegate = self
        searchField.translatesAutoresizingMaskIntoConstraints = false

        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.documentView = table

        table.dataSource = self
        table.delegate = self
        table.headerView = nil
        table.allowsMultipleSelection = false
        table.usesAlternatingRowBackgroundColors = true
        table.rowHeight = 38
        table.target = self
        table.doubleAction = #selector(rowDoubleClicked)

        let col = NSTableColumn(identifier: .init("c"))
        col.width = 460
        table.addTableColumn(col)

        let hint = NSTextField(labelWithString: "↑↓ nav · ⏎ paste · ⇧⏎ plain · ⌥⏎ format · ⌘P pin · ⌘⌫ delete · 1–9 quick · ⎋ close")
        hint.font = .systemFont(ofSize: 10)
        hint.textColor = .secondaryLabelColor
        hint.translatesAutoresizingMaskIntoConstraints = false

        content.addSubview(searchField)
        content.addSubview(scroll)
        content.addSubview(hint)

        NSLayoutConstraint.activate([
            searchField.topAnchor.constraint(equalTo: content.topAnchor, constant: 32),
            searchField.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 12),
            searchField.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -12),

            scroll.topAnchor.constraint(equalTo: searchField.bottomAnchor, constant: 8),
            scroll.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: hint.topAnchor, constant: -4),

            hint.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 12),
            hint.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -6),
        ])
    }

    // MARK: - Show / hide

    func show(near statusItem: NSStatusItem?) {
        reload(query: "")
        searchField.stringValue = ""
        // Pre-select first row
        if !items.isEmpty {
            table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        }

        // Position the panel just below the status item's icon.
        if let button = statusItem?.button,
           let buttonWindow = button.window,
           let panel = window {
            let buttonFrameInWindow = button.convert(button.bounds, to: nil)
            let screenRect = buttonWindow.convertToScreen(buttonFrameInWindow)
            let panelOrigin = NSPoint(
                x: screenRect.midX - panel.frame.width / 2,
                y: screenRect.minY - panel.frame.height - 6
            )
            panel.setFrameOrigin(panelOrigin)
        } else {
            window?.center()
        }
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        // フォーカスをサーチフィールドに
        window?.makeFirstResponder(searchField)
    }

    func hide() {
        window?.orderOut(nil)
    }

    // MARK: - Data

    private func reload(query: String) {
        let json: UnsafeMutablePointer<CChar>?
        if query.isEmpty {
            json = cnx_list_history_json(nil, 50)
        } else {
            json = query.withCString { cstr in cnx_list_history_json(cstr, 50) }
        }
        defer { if let j = json { cnx_free_string(j) } }
        guard let json,
              let str = String(validatingUTF8: json),
              let data = str.data(using: .utf8) else {
            items = []
            table.reloadData()
            return
        }
        items = (try? JSONDecoder().decode([HistoryItem].self, from: data)) ?? []
        table.reloadData()
        if !items.isEmpty {
            table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
            table.scrollRowToVisible(0)
        }
    }

    // MARK: - Actions

    @objc private func rowDoubleClicked() {
        pasteSelected()
    }

    private func pasteSelected(mode: Int32 = 0) {
        guard let item = currentItem() else { return }
        hide()
        // 60ms 待って前面アプリにフォーカスが戻ってからペースト
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.06) {
            let r = item.id.withCString { cstr in cnx_paste_item(cstr, mode) }
            if r != 0, let msg = cnx_last_error() {
                NSLog("paste_item failed: \(String(cString: msg))")
                cnx_free_string(msg)
            }
        }
    }

    /// 選択アイテムのピン留めをトグルし、リスト再読込。
    fileprivate func togglePinSelected() {
        guard let item = currentItem() else { return }
        _ = item.id.withCString { cstr in cnx_pin_toggle(cstr) }
        // 同じ id を選択し続けるためにスクロール位置を保つ
        let prevId = item.id
        reload(query: searchField.stringValue)
        if let row = items.firstIndex(where: { $0.id == prevId }) {
            table.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            table.scrollRowToVisible(row)
        }
    }

    /// 選択アイテムを削除し、リスト再読込。
    fileprivate func deleteSelected() {
        guard let item = currentItem() else { return }
        let r = item.id.withCString { cstr in cnx_delete_item(cstr) }
        if r != 0, let msg = cnx_last_error() {
            NSLog("delete_item failed: \(String(cString: msg))")
            cnx_free_string(msg)
        }
        let prevRow = table.selectedRow
        reload(query: searchField.stringValue)
        // 削除後は同じ位置 (もしくは末尾) を選択
        if !items.isEmpty {
            let next = max(0, min(prevRow, items.count - 1))
            table.selectRowIndexes(IndexSet(integer: next), byExtendingSelection: false)
        }
    }

    /// Alt+Enter で Format Paste モーダルを開く。
    fileprivate func openFormatModalForSelected() {
        guard let item = currentItem() else { return }
        hide()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            NSApp.activate(ignoringOtherApps: true)
            let win = FormatPasteWindow(itemId: item.id, sourceText: item.preview)
            win.showWindow(nil)
            win.window?.makeKeyAndOrderFront(nil)
        }
    }

    private func currentItem() -> HistoryItem? {
        let row = table.selectedRow
        return (0..<items.count).contains(row) ? items[row] : nil
    }

    // MARK: - TextField delegate (search field input)

    func controlTextDidChange(_ obj: Notification) {
        reload(query: searchField.stringValue)
    }

    // Search field interprets some key commands, but we forward arrows/enter to the table.
    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        switch commandSelector {
        case #selector(NSResponder.moveDown(_:)):
            moveSelection(by: +1)
            return true
        case #selector(NSResponder.moveUp(_:)):
            moveSelection(by: -1)
            return true
        case #selector(NSResponder.insertNewline(_:)):
            // Shift+Enter = plain (mode 1) / Alt+Enter = format modal / default = normal
            let mod = NSApp.currentEvent?.modifierFlags ?? []
            if mod.contains(.shift) {
                pasteSelected(mode: 1)
            } else if mod.contains(.option) {
                openFormatModalForSelected()
            } else {
                pasteSelected()
            }
            return true
        case #selector(NSResponder.cancelOperation(_:)):
            hide()
            return true
        default:
            return false
        }
    }

    private func moveSelection(by delta: Int) {
        if items.isEmpty { return }
        let cur = table.selectedRow
        let next = max(0, min(items.count - 1, cur + delta))
        table.selectRowIndexes(IndexSet(integer: next), byExtendingSelection: false)
        table.scrollRowToVisible(next)
    }

    // MARK: - Key events from the panel itself (number quick-paste)

    func handleQuickPasteKey(_ event: NSEvent) -> Bool {
        // 1〜9 で n 番目をペースト (修飾キーなし)
        guard let chars = event.charactersIgnoringModifiers, !chars.isEmpty,
              event.modifierFlags.intersection(.deviceIndependentFlagsMask).isEmpty else {
            return false
        }
        if let n = Int(chars), (1...9).contains(n), n - 1 < items.count {
            table.selectRowIndexes(IndexSet(integer: n - 1), byExtendingSelection: false)
            pasteSelected()
            return true
        }
        return false
    }

    // MARK: - TableView data source

    func numberOfRows(in tableView: NSTableView) -> Int { items.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let item = items[row]
        let cell = NSTableCellView()
        cell.translatesAutoresizingMaskIntoConstraints = false

        // 番号 (1〜9)
        let idx = NSTextField(labelWithString: row < 9 ? "\(row + 1)" : " ")
        idx.font = .monospacedDigitSystemFont(ofSize: 11, weight: .medium)
        idx.textColor = .secondaryLabelColor
        idx.alignment = .right
        idx.widthAnchor.constraint(equalToConstant: 16).isActive = true

        // 種別アイコン (絵文字でシンプルに)
        let kindIcon = NSTextField(labelWithString: Self.kindEmoji(item.kind))
        kindIcon.font = .systemFont(ofSize: 13)
        kindIcon.widthAnchor.constraint(equalToConstant: 18).isActive = true
        kindIcon.toolTip = item.kind

        // プレビュー (本文) — 上段
        let displayText: String = {
            if item.preview.isEmpty {
                // バイナリ系 (Image/PDF/Files) は preview なしのことがある
                return "(\(item.kind))"
            }
            return item.preview.replacingOccurrences(of: "\n", with: " ")
        }()
        let preview = NSTextField(labelWithString: displayText)
        preview.lineBreakMode = .byTruncatingTail
        preview.font = .systemFont(ofSize: 12)
        preview.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        if item.pinned {
            preview.stringValue = "📌 " + preview.stringValue
        }

        // メタ情報 (右下): ソースアプリ · 経過時間
        let metaText = "\(item.source_app) · \(Self.relativeTime(from: item.created_at))"
        let meta = NSTextField(labelWithString: metaText)
        meta.font = .systemFont(ofSize: 10)
        meta.textColor = .secondaryLabelColor
        meta.lineBreakMode = .byTruncatingTail

        // 縦に preview / meta を積む
        let textStack = NSStackView(views: [preview, meta])
        textStack.orientation = .vertical
        textStack.alignment = .leading
        textStack.spacing = 1
        textStack.distribution = .fill

        let h = NSStackView(views: [idx, kindIcon, textStack])
        h.orientation = .horizontal
        h.spacing = 8
        h.alignment = .centerY
        h.translatesAutoresizingMaskIntoConstraints = false

        cell.addSubview(h)
        NSLayoutConstraint.activate([
            h.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 12),
            h.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -12),
            h.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
        ])
        cell.textField = preview
        return cell
    }

    /// Rust 側 `ClipKind` の Debug 表記を絵文字に。
    private static func kindEmoji(_ kind: String) -> String {
        switch kind {
        case "Text":   return "📝"
        case "Image":  return "🖼"
        case "Rtf":    return "🅡"
        case "Html":   return "🌐"
        case "Pdf":    return "📄"
        case "Files":  return "📁"
        default:        return "•"
        }
    }

    /// "5s" / "12m" / "3h" / "2d" 等の短い相対時刻表現。
    private static func relativeTime(from ms: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        let diff = max(0, now - ms) / 1000 // sec
        switch diff {
        case 0..<60:      return "\(diff)s"
        case 60..<3600:   return "\(diff / 60)m"
        case 3600..<86400: return "\(diff / 3600)h"
        default:           return "\(diff / 86400)d"
        }
    }

    // MARK: - Window delegate (auto-hide)

    func windowDidResignKey(_ notification: Notification) {
        // クリック外し / 他アプリ切替で隠す
        hide()
    }
}

// MARK: - NSPanel subclass that can become key (needed for nonactivatingPanel)

final class NonActivatingPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { // Esc
            SearchPanel.shared.hide()
            return
        }
        // 数字キー即ペースト (検索欄非フォーカス時)
        if SearchPanel.shared.handleQuickPasteKey(event) { return }
        super.keyDown(with: event)
    }

    /// ⌘ 系ショートカットは検索欄にフォーカスがあっても拾う必要があるため
    /// performKeyEquivalent をオーバーライド。
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if mods == .command {
            switch event.keyCode {
            case 51: // kVK_Delete (Backspace)
                SearchPanel.shared.deleteSelected()
                return true
            case 35: // kVK_ANSI_P
                SearchPanel.shared.togglePinSelected()
                return true
            default: break
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}
