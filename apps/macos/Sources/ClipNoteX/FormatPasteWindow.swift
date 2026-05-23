// FormatPasteWindow — Format Paste のライブプレビュー。
//
//   - 上半分: 元のテキスト (read-only)
//   - 下半分: 整形後 (auto detect / json / sql / markdown 等を選択)
//   - "Paste Formatted" ボタンで Rust 側に `paste_item` mode=2 を投げる
//
// 設計: NSPanel ではなく通常 NSWindow (フォアグラウンドで使う想定。
//       ペースト時は閉じてから cnx_paste_item を呼ぶ)

import AppKit
import Foundation
import ClipNoteXCore

final class FormatPasteWindow: NSWindowController, NSWindowDelegate {
    private let sourceView = NSTextView()
    private let outputView = NSTextView()
    private let langPicker = NSPopUpButton()
    private let detectedLabel = NSTextField(labelWithString: "")
    private let pasteBtn = NSButton(title: "Paste Formatted", target: nil, action: nil)

    private let itemId: String
    private let sourceText: String

    init(itemId: String, sourceText: String) {
        self.itemId = itemId
        self.sourceText = sourceText
        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 500),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        win.title = "Format Paste"
        win.center()
        super.init(window: win)
        win.delegate = self
        setupUI()
        refreshPreview()
    }
    required init?(coder: NSCoder) { fatalError() }

    private func setupUI() {
        guard let content = window?.contentView else { return }

        let toolbar = NSStackView()
        toolbar.orientation = .horizontal
        toolbar.spacing = 8
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        toolbar.edgeInsets = NSEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)

        let langLbl = NSTextField(labelWithString: "Language:")
        langPicker.target = self
        langPicker.action = #selector(langChanged)
        for lang in ["auto", "json", "sql", "markdown", "plain", "html", "css", "javascript", "typescript"] {
            langPicker.addItem(withTitle: lang)
        }
        langPicker.selectItem(withTitle: "auto")

        detectedLabel.textColor = .secondaryLabelColor
        detectedLabel.font = .systemFont(ofSize: 11)

        pasteBtn.bezelStyle = .rounded
        pasteBtn.target = self
        pasteBtn.action = #selector(pasteFormatted)
        pasteBtn.keyEquivalent = "\r" // ⏎

        toolbar.addArrangedSubview(langLbl)
        toolbar.addArrangedSubview(langPicker)
        toolbar.addArrangedSubview(detectedLabel)
        toolbar.addArrangedSubview(NSView())
        toolbar.addArrangedSubview(pasteBtn)
        content.addSubview(toolbar)

        let split = NSSplitView()
        split.dividerStyle = .thin
        split.isVertical = false
        split.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(split)

        for tv in [sourceView, outputView] {
            tv.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
            tv.isAutomaticQuoteSubstitutionEnabled = false
            tv.isAutomaticDashSubstitutionEnabled = false
            tv.isAutomaticTextReplacementEnabled = false
            tv.minSize = .zero
            tv.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
            tv.isVerticallyResizable = true
            tv.autoresizingMask = [.width]
            tv.textContainer?.widthTracksTextView = true
        }
        sourceView.isEditable = false
        sourceView.string = sourceText
        outputView.isEditable = false

        let topScroll = scrollView(with: sourceView, title: "Source")
        let bottomScroll = scrollView(with: outputView, title: "Formatted")
        split.addArrangedSubview(topScroll)
        split.addArrangedSubview(bottomScroll)

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

    private func scrollView(with tv: NSTextView, title: String) -> NSView {
        let v = NSView()
        let lbl = NSTextField(labelWithString: title)
        lbl.font = .systemFont(ofSize: 10, weight: .semibold)
        lbl.textColor = .secondaryLabelColor
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.documentView = tv
        scroll.translatesAutoresizingMaskIntoConstraints = false
        lbl.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(lbl)
        v.addSubview(scroll)
        NSLayoutConstraint.activate([
            lbl.topAnchor.constraint(equalTo: v.topAnchor, constant: 4),
            lbl.leadingAnchor.constraint(equalTo: v.leadingAnchor, constant: 12),
            scroll.topAnchor.constraint(equalTo: lbl.bottomAnchor, constant: 4),
            scroll.leadingAnchor.constraint(equalTo: v.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: v.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: v.bottomAnchor),
        ])
        return v
    }

    @objc private func langChanged() { refreshPreview() }

    private func refreshPreview() {
        let lang = langPicker.titleOfSelectedItem ?? "auto"
        let json = sourceText.withCString { tc in
            lang.withCString { lc in
                cnx_format_preview_json(tc, lc, /* indent: */ 2)
            }
        }
        defer { if let j = json { cnx_free_string(j) } }
        guard let json,
              let str = String(validatingUTF8: json),
              let data = str.data(using: String.Encoding.utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            outputView.string = "(format error — see log)"
            return
        }
        let formatted = (obj["formatted"] as? String) ?? ""
        let detected = (obj["detected_lang"] as? String) ?? "?"
        // 検出言語 (auto-detect 結果) でハイライト
        let highlightLang = (lang == "auto") ? detected : lang
        let attr = SyntaxHighlight.highlight(formatted, language: highlightLang)
        outputView.textStorage?.setAttributedString(attr)
        detectedLabel.stringValue = "detected: \(detected)"
    }

    @objc private func pasteFormatted() {
        // 閉じてから前面アプリにフォーカスが戻るのを待つ
        let id = itemId
        close()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
            let r = id.withCString { cstr in cnx_paste_item(cstr, /* format */ 2) }
            if r != 0, let msg = cnx_last_error() {
                NSLog("paste(format) failed: \(String(cString: msg))")
                cnx_free_string(msg)
            }
        }
    }
}
