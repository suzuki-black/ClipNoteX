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
            // Clipy/Maccy 並みのコンパクトサイズ。元は 480×520 だったが、画面占有が大きく
             // 視界を遮るとの指摘あり。1 行レイアウト + 小さめフォントで切り詰めた。
            contentRect: NSRect(x: 0, y: 0, width: 360, height: 400),
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

        // 検索欄右端のアクションボタン群 (アイコン右クリックを使わずに開けるように):
        //   - 📓 DONE LOG (⌘L)
        //   - ⚙  Preferences (⌘,)
        // ノッチ系常駐アプリで右クリックが奪われるユーザ向けの保険でもある。
        let doneButton = NSButton()
        doneButton.bezelStyle = .recessed
        doneButton.isBordered = false
        doneButton.imagePosition = .imageOnly
        if let img = NSImage(systemSymbolName: "book.closed", accessibilityDescription: "DONE LOG") {
            doneButton.image = img
        } else {
            doneButton.title = "📓"
        }
        doneButton.target = self
        doneButton.action = #selector(openDoneLog)
        doneButton.toolTip = "Open DONE LOG (⌘L)"
        doneButton.translatesAutoresizingMaskIntoConstraints = false

        let prefsButton = NSButton()
        prefsButton.bezelStyle = .recessed
        prefsButton.isBordered = false
        prefsButton.imagePosition = .imageOnly
        if let img = NSImage(systemSymbolName: "gearshape", accessibilityDescription: "Preferences") {
            prefsButton.image = img
        } else {
            prefsButton.title = "⚙"
        }
        prefsButton.target = self
        prefsButton.action = #selector(openPreferences)
        prefsButton.toolTip = "Preferences (⌘,)"
        prefsButton.translatesAutoresizingMaskIntoConstraints = false

        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.documentView = table

        table.dataSource = self
        table.delegate = self
        table.headerView = nil
        table.allowsMultipleSelection = false
        table.usesAlternatingRowBackgroundColors = true
        table.rowHeight = 22
        table.target = self
        table.doubleAction = #selector(rowDoubleClicked)

        let col = NSTableColumn(identifier: .init("c"))
        col.width = 340
        table.addTableColumn(col)

        // Clipy 風コンパクト: ヒント行は短く、フォントも 9pt に。
        let hint = NSTextField(labelWithString: "↑↓ · ⏎ paste · ⇧⏎ plain · ⌥⏎ fmt · ⌘P pin · ⌘⌫ del · 1–9 · ⌘L log · ⌘, prefs · ⎋")
        hint.font = .systemFont(ofSize: 9)
        hint.textColor = .tertiaryLabelColor
        hint.lineBreakMode = .byTruncatingTail
        hint.translatesAutoresizingMaskIntoConstraints = false

        content.addSubview(searchField)
        content.addSubview(doneButton)
        content.addSubview(prefsButton)
        content.addSubview(scroll)
        content.addSubview(hint)

        NSLayoutConstraint.activate([
            // タイトルバー裏に来ない最小限の天井 (信号機分: 22pt) + 6pt
            searchField.topAnchor.constraint(equalTo: content.topAnchor, constant: 28),
            searchField.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 8),
            // 検索欄の右端はボタン群の左に揃える
            searchField.trailingAnchor.constraint(equalTo: doneButton.leadingAnchor, constant: -4),

            // 📓 DONE LOG ボタン
            doneButton.centerYAnchor.constraint(equalTo: searchField.centerYAnchor),
            doneButton.trailingAnchor.constraint(equalTo: prefsButton.leadingAnchor, constant: -2),
            doneButton.widthAnchor.constraint(equalToConstant: 20),
            doneButton.heightAnchor.constraint(equalToConstant: 20),

            // ⚙ Preferences ボタン
            prefsButton.centerYAnchor.constraint(equalTo: searchField.centerYAnchor),
            prefsButton.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -8),
            prefsButton.widthAnchor.constraint(equalToConstant: 20),
            prefsButton.heightAnchor.constraint(equalToConstant: 20),

            scroll.topAnchor.constraint(equalTo: searchField.bottomAnchor, constant: 4),
            scroll.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: hint.topAnchor, constant: -2),

            hint.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 8),
            hint.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -8),
            hint.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -4),
        ])
    }

    // MARK: - Show / hide

    /// 既に開いていたら閉じて nil を返し、閉じていたら開いて true を返す。
    /// グローバルホットキーの「もう一度押したら消える」挙動を実現する。
    @discardableResult
    func toggle(near statusItem: NSStatusItem?) -> Bool {
        if let w = window, w.isVisible {
            w.orderOut(nil)
            return false
        }
        show(near: statusItem)
        return true
    }

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
        // 空クエリ時はリスト表示なので 50 件 (パフォーマンス重視)。
        // 検索中はバックエンドの上限 (200) まで掘る。
        if query.isEmpty {
            json = cnx_list_history_json(nil, 50)
        } else {
            json = query.withCString { cstr in cnx_list_history_json(cstr, 200) }
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
    /// Preferences ウィンドウを開く。検索パネル右上の歯車ボタン / ⌘, から呼ばれる。
    /// パネル自体は閉じる (Preferences が前面 / activate するので)。
    @objc fileprivate func openPreferences() {
        hide()
        PreferencesWindow.show()
    }

    /// DONE LOG ウィンドウを開く。検索パネル右上の 📓 ボタン / ⌘L から呼ばれる。
    @objc fileprivate func openDoneLog() {
        hide()
        DoneLogWindow.show()
    }

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

        // Clipy 風コンパクト 1 行レイアウト:
        //   [N] [kind] preview text ……          source · 5s
        // 1 行 22pt 高に収め、ソース/経過時刻は右寄せの小さな副情報として一列化。

        // 番号 (1〜9)。10 以上は空白扱いなので 1 桁固定幅 + 左寄せでよい
        // (旧コードは右揃え + 14pt 幅で左に余白ができていた)
        let idx = NSTextField(labelWithString: row < 9 ? "\(row + 1)" : " ")
        idx.font = .monospacedDigitSystemFont(ofSize: 10, weight: .medium)
        idx.textColor = .secondaryLabelColor
        idx.alignment = .left
        idx.widthAnchor.constraint(equalToConstant: 10).isActive = true

        // 種別アイコン
        let kindIcon = NSTextField(labelWithString: Self.kindEmoji(item.kind))
        kindIcon.font = .systemFont(ofSize: 11)
        kindIcon.widthAnchor.constraint(equalToConstant: 14).isActive = true
        kindIcon.toolTip = item.kind

        // プレビュー本文 (左、可伸長)
        let displayText: String = item.preview.isEmpty
            ? "(\(item.kind))"
            : item.preview.replacingOccurrences(of: "\n", with: " ")
        let baseText = item.pinned ? "📌 " + displayText : displayText
        let preview = NSTextField(labelWithString: baseText)
        preview.lineBreakMode = .byTruncatingTail
        preview.font = .systemFont(ofSize: 11)
        preview.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        preview.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let q = searchField.stringValue
        preview.attributedStringValue = Self.highlightMatches(in: baseText, query: q, base: preview.font)

        // メタ情報 (右、固定幅): ソースアプリ + 経過時間。長すぎる app 名は短縮。
        let appShort = item.source_app.count > 12
            ? String(item.source_app.prefix(11)) + "…"
            : item.source_app
        let metaText = "\(appShort) · \(Self.relativeTime(from: item.created_at))"
        let meta = NSTextField(labelWithString: metaText)
        meta.font = .systemFont(ofSize: 9)
        meta.textColor = .tertiaryLabelColor
        meta.lineBreakMode = .byTruncatingTail
        meta.alignment = .right
        meta.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        meta.setContentCompressionResistancePriority(.defaultHigh, for: .horizontal)
        meta.toolTip = "\(item.source_app) · \(Self.relativeTime(from: item.created_at))"

        let h = NSStackView(views: [idx, kindIcon, preview, meta])
        h.orientation = .horizontal
        h.spacing = 6
        h.alignment = .centerY
        h.translatesAutoresizingMaskIntoConstraints = false

        cell.addSubview(h)
        NSLayoutConstraint.activate([
            h.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 8),
            h.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -8),
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

    /// Highlight occurrences of `query` in `text` with a yellow background.
    /// Identical to the DONE LOG helper; kept duplicated to avoid cross-file refs.
    fileprivate static func highlightMatches(in text: String,
                                              query: String,
                                              base baseFont: NSFont?) -> NSAttributedString {
        let result = NSMutableAttributedString(string: text, attributes: [
            .font: baseFont ?? NSFont.systemFont(ofSize: 12),
            .foregroundColor: NSColor.labelColor,
        ])
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { return result }
        let nsText = text as NSString
        var searchRange = NSRange(location: 0, length: nsText.length)
        while searchRange.location < nsText.length {
            let found = nsText.range(of: q, options: .caseInsensitive, range: searchRange)
            if found.location == NSNotFound { break }
            result.addAttributes([
                .backgroundColor: NSColor.systemYellow.withAlphaComponent(0.55),
                .foregroundColor: NSColor.black,
            ], range: found)
            let next = found.upperBound
            searchRange = NSRange(location: next, length: max(0, nsText.length - next))
        }
        return result
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
            case 43: // kVK_ANSI_Comma  →  ⌘, で Preferences
                SearchPanel.shared.openPreferences()
                return true
            case 37: // kVK_ANSI_L  →  ⌘L で DONE LOG
                SearchPanel.shared.openDoneLog()
                return true
            default: break
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}
