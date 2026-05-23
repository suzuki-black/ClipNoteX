// DoneLogWindow — DONE LOG のフル UI。
//
// 構成:
//   [Toolbar: 今日へ / 日付ピッカー / 件数 / MD エクスポート ]
//   [TableView: time / source / body / tags ]
//   [Detail pane: 選択中エントリの note / tags 編集 + 削除]
//
// データは Rust FFI から JSON で取得して `[DoneItem]` にデコード。
// 全 CRUD は Rust 側に流す (delete / update_overlay)。

import AppKit
import Foundation
import ClipNoteXCore

// MARK: - Model

struct DoneItem: Decodable {
    let id: String
    let date: String   // YYYY-MM-DD
    let time: String   // HH:MM
    let source_app: String
    let kind: String
    let body: String
    let note: String?
    let tags: [String]
}

// MARK: - Window controller

final class DoneLogWindow: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate {
    static private(set) var instance: DoneLogWindow?

    static func show() {
        if instance == nil { instance = DoneLogWindow() }
        guard let i = instance else { return }
        NSApp.activate(ignoringOtherApps: true)
        i.showWindow(nil)
        i.window?.makeKeyAndOrderFront(nil)
        i.reload()
    }

    private var items: [DoneItem] = []         // フィルタ後 (表示用)
    private var allItems: [DoneItem] = []      // ロード結果 (検索ソース)
    private var selectedDate: String = "" // 初期値は init() で today を入れる
    private var filterText: String = ""

    private let datePicker = NSDatePicker()
    private let countLabel = NSTextField(labelWithString: "0 件")
    private let searchBar = NSSearchField()
    private let table = NSTableView()
    private let noteField = NSTextField()
    private let tagField = NSTextField()
    private let tagListLabel = NSTextField(labelWithString: "")
    private let bodyView = NSTextView()

    init() {
        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 820, height: 540),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        win.title = "ClipNoteX — DONE LOG"
        win.center()
        win.minSize = NSSize(width: 600, height: 360)
        super.init(window: win)
        win.delegate = self
        selectedDate = Self.todayString()
        setupUI()
    }
    required init?(coder: NSCoder) { fatalError() }

    // MARK: UI

    private func setupUI() {
        guard let content = window?.contentView else { return }

        // -- Toolbar row --
        let toolbar = NSStackView()
        toolbar.orientation = .horizontal
        toolbar.spacing = 8
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.edgeInsets = NSEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)

        datePicker.datePickerStyle = .textField
        datePicker.datePickerElements = .yearMonthDay
        datePicker.dateValue = Self.parseDate(selectedDate) ?? Date()
        datePicker.target = self
        datePicker.action = #selector(dateChanged)

        let todayBtn = NSButton(title: "今日", target: self, action: #selector(goToday))
        todayBtn.bezelStyle = .rounded

        let exportBtn = NSButton(title: "MD エクスポート", target: self, action: #selector(exportMarkdown))
        exportBtn.bezelStyle = .rounded

        searchBar.placeholderString = "Search body / note / tag…"
        searchBar.target = self
        searchBar.action = #selector(searchChanged)
        searchBar.sendsSearchStringImmediately = true
        searchBar.sendsWholeSearchString = false
        searchBar.widthAnchor.constraint(equalToConstant: 220).isActive = true

        toolbar.addArrangedSubview(datePicker)
        toolbar.addArrangedSubview(todayBtn)
        toolbar.addArrangedSubview(countLabel)
        toolbar.addArrangedSubview(searchBar)
        toolbar.addArrangedSubview(NSView()) // spacer
        toolbar.addArrangedSubview(exportBtn)
        content.addSubview(toolbar)

        // -- Split: table on top, detail pane on bottom --
        let split = NSSplitView()
        split.dividerStyle = .thin
        split.isVertical = false
        split.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(split)

        // Table
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.translatesAutoresizingMaskIntoConstraints = false

        table.dataSource = self
        table.delegate = self
        table.usesAlternatingRowBackgroundColors = true
        table.allowsMultipleSelection = false
        table.rowHeight = 20

        let cTime = NSTableColumn(identifier: .init("time"))
        cTime.title = "Time"; cTime.width = 60
        let cSource = NSTableColumn(identifier: .init("source"))
        cSource.title = "Source"; cSource.width = 100
        let cBody = NSTableColumn(identifier: .init("body"))
        cBody.title = "Body"; cBody.width = 380
        let cTags = NSTableColumn(identifier: .init("tags"))
        cTags.title = "Tags"; cTags.width = 150
        for col in [cTime, cSource, cBody, cTags] { table.addTableColumn(col) }
        scroll.documentView = table
        split.addArrangedSubview(scroll)

        // Detail pane
        let detail = NSView()
        detail.translatesAutoresizingMaskIntoConstraints = false

        let bodyLabel = NSTextField(labelWithString: "Body:")
        let bodyScroll = NSScrollView()
        bodyScroll.hasVerticalScroller = true
        bodyView.isEditable = false
        bodyView.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        bodyScroll.documentView = bodyView

        let noteLabel = NSTextField(labelWithString: "Note:")
        noteField.placeholderString = "メモを入力 (⏎ で保存)"
        noteField.target = self
        noteField.action = #selector(saveNote)

        let tagLabel = NSTextField(labelWithString: "Tags:")
        tagField.placeholderString = "#タグ (⏎ で追加)"
        tagField.target = self
        tagField.action = #selector(addTag)

        tagListLabel.lineBreakMode = .byTruncatingTail
        tagListLabel.cell?.usesSingleLineMode = false
        tagListLabel.allowsEditingTextAttributes = false

        let deleteBtn = NSButton(title: "🗑 削除", target: self, action: #selector(deleteSelected))
        deleteBtn.bezelStyle = .rounded
        deleteBtn.contentTintColor = .systemRed

        let stack = NSStackView(views: [
            bodyLabel, bodyScroll,
            noteLabel, noteField,
            tagLabel, tagField, tagListLabel,
            deleteBtn,
        ])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 6
        stack.edgeInsets = NSEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        detail.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: detail.topAnchor),
            stack.leadingAnchor.constraint(equalTo: detail.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: detail.trailingAnchor),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: detail.bottomAnchor),
            bodyScroll.heightAnchor.constraint(equalToConstant: 80),
            bodyScroll.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -24),
        ])
        split.addArrangedSubview(detail)
        split.setHoldingPriority(.defaultLow, forSubviewAt: 0)

        // Layout: toolbar above split
        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: content.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            split.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            split.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            split.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            split.bottomAnchor.constraint(equalTo: content.bottomAnchor),
        ])
    }

    // MARK: Data load

    func reload() {
        let dateForFfi: String? = selectedDate.isEmpty ? nil : selectedDate
        let json: UnsafeMutablePointer<CChar>?
        if let d = dateForFfi {
            json = d.withCString { ds in cnx_list_done_json(ds, 200) }
        } else {
            json = cnx_list_done_json(nil, 200)
        }
        defer { if let j = json { cnx_free_string(j) } }

        guard let json,
              let str = String(validatingUTF8: json),
              let data = str.data(using: .utf8) else {
            allItems = []
            applyFilter()
            return
        }
        do {
            allItems = try JSONDecoder().decode([DoneItem].self, from: data)
        } catch {
            NSLog("DoneLog decode: \(error)")
            allItems = []
        }
        applyFilter()
    }

    private func applyFilter() {
        let q = filterText.lowercased()
        if q.isEmpty {
            items = allItems
        } else {
            items = allItems.filter { item in
                item.body.lowercased().contains(q)
                    || (item.note?.lowercased().contains(q) ?? false)
                    || item.tags.contains(where: { $0.lowercased().contains(q) })
                    || item.source_app.lowercased().contains(q)
            }
        }
        countLabel.stringValue = filterText.isEmpty
            ? "\(items.count) 件"
            : "\(items.count) / \(allItems.count) 件"
        table.reloadData()
        if !items.isEmpty {
            table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        } else {
            clearDetail()
        }
    }

    @objc private func searchChanged() {
        filterText = searchBar.stringValue
        applyFilter()
    }

    // MARK: Actions

    @objc private func dateChanged() {
        selectedDate = Self.dateString(from: datePicker.dateValue)
        reload()
    }

    @objc private func goToday() {
        selectedDate = Self.todayString()
        datePicker.dateValue = Date()
        reload()
    }

    @objc private func exportMarkdown() {
        let md: UnsafeMutablePointer<CChar>?
        if selectedDate.isEmpty {
            md = cnx_export_done_markdown(nil)
        } else {
            md = selectedDate.withCString { ds in cnx_export_done_markdown(ds) }
        }
        defer { if let m = md { cnx_free_string(m) } }
        guard let md, let text = String(validatingUTF8: md) else { return }

        let panel = NSSavePanel()
        panel.allowedContentTypes = [.init(filenameExtension: "md")!]
        panel.nameFieldStringValue = "donelog-\(selectedDate).md"
        panel.beginSheetModal(for: window!) { resp in
            if resp == .OK, let url = panel.url {
                try? text.write(to: url, atomically: true, encoding: .utf8)
            }
        }
    }

    @objc private func saveNote() {
        guard let item = currentItem() else { return }
        let payload: [String: Any] = [
            "id": item.id,
            "note": noteField.stringValue,
            "add_tags": [],
            "remove_tags": [],
        ]
        callUpdateOverlay(payload: payload)
        reload()
    }

    @objc private func addTag() {
        guard let item = currentItem() else { return }
        let raw = tagField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let tag = raw.hasPrefix("#") ? String(raw.dropFirst()) : raw
        if tag.isEmpty || item.tags.contains(tag) { return }
        let payload: [String: Any] = [
            "id": item.id,
            "add_tags": [tag],
            "remove_tags": [],
        ]
        callUpdateOverlay(payload: payload)
        tagField.stringValue = ""
        reload()
    }

    @objc private func deleteSelected() {
        guard let item = currentItem() else { return }
        let alert = NSAlert()
        alert.messageText = "この記録を削除しますか？"
        alert.informativeText = item.body.prefix(120) + (item.body.count > 120 ? "…" : "")
        alert.addButton(withTitle: "削除")
        alert.addButton(withTitle: "キャンセル")
        guard alert.runModal() == .alertFirstButtonReturn else { return }

        let r = item.id.withCString { cstr in cnx_delete_done(cstr) }
        if r != 0, let msg = cnx_last_error() {
            NSLog("delete_done: \(String(cString: msg))")
            cnx_free_string(msg)
        }
        reload()
    }

    private func callUpdateOverlay(payload: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let str = String(data: data, encoding: .utf8) else { return }
        let r = str.withCString { cstr in cnx_update_done_overlay_json(cstr) }
        if r != 0, let msg = cnx_last_error() {
            NSLog("update_overlay: \(String(cString: msg))")
            cnx_free_string(msg)
        }
    }

    private func currentItem() -> DoneItem? {
        let row = table.selectedRow
        return (0..<items.count).contains(row) ? items[row] : nil
    }

    private func clearDetail() {
        bodyView.string = ""
        noteField.stringValue = ""
        tagField.stringValue = ""
        tagListLabel.stringValue = ""
    }

    // MARK: TableView data source / delegate

    func numberOfRows(in tableView: NSTableView) -> Int { items.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard let col = tableColumn else { return nil }
        let id = col.identifier.rawValue
        let item = items[row]
        let cell = NSTableCellView()
        let tf = NSTextField(labelWithString: "")
        tf.lineBreakMode = .byTruncatingTail
        tf.font = .systemFont(ofSize: 11)
        switch id {
        case "time":   tf.stringValue = item.time
        case "source": tf.stringValue = item.source_app
        case "body":
            // 検索クエリと一致する部分を黄色マーカーで強調
            let body = item.body.replacingOccurrences(of: "\n", with: " ")
            tf.attributedStringValue = Self.highlightMatches(in: body,
                                                              query: filterText,
                                                              base: tf.font)
        case "tags":
            let joined = item.tags.map { "#\($0)" }.joined(separator: " ")
            tf.attributedStringValue = Self.highlightMatches(in: joined,
                                                              query: filterText,
                                                              base: tf.font)
        default: break
        }
        cell.addSubview(tf)
        tf.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            tf.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 2),
            tf.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -2),
            tf.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
        ])
        cell.textField = tf
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        if let item = currentItem() {
            bodyView.string = item.body
            noteField.stringValue = item.note ?? ""
            tagListLabel.stringValue = item.tags.isEmpty
                ? "(no tags)"
                : item.tags.map { "#\($0)" }.joined(separator: "  ")
        } else {
            clearDetail()
        }
    }

    // MARK: Window delegate

    func windowWillClose(_ notification: Notification) {
        Self.instance = nil
    }

    // MARK: Helpers

    private static func todayString() -> String {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withFullDate]
        return f.string(from: Date())
    }
    private static func dateString(from date: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return f.string(from: date)
    }
    private static func parseDate(_ s: String) -> Date? {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return f.date(from: s)
    }

    /// Wrap each case-insensitive occurrence of `query` in `text` with a
    /// yellow background highlight. Returns an NSAttributedString that can
    /// be assigned to NSTextField.attributedStringValue.
    private static func highlightMatches(in text: String,
                                         query: String,
                                         base baseFont: NSFont?) -> NSAttributedString {
        let attrs: [NSAttributedString.Key: Any] = [
            .font: baseFont ?? NSFont.systemFont(ofSize: 11),
            .foregroundColor: NSColor.labelColor,
        ]
        let result = NSMutableAttributedString(string: text, attributes: attrs)
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
}
