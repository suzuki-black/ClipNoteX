// DoneLogWindow — DONE LOG 一覧 (v0.1 最小版: テキストビュー表示のみ)。
//
// 本格 UI は v0.2 で NSTableView ベースに拡張する。
// 今は「JSON を取って表示」レベル + Markdown エクスポートボタン。

import AppKit
import ClipNoteXCore

final class DoneLogWindow: NSWindowController, NSWindowDelegate {
    static private var instance: DoneLogWindow?

    static func show() {
        if instance == nil {
            instance = DoneLogWindow()
        }
        instance?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
        instance?.window?.makeKeyAndOrderFront(nil)
        instance?.reload()
    }

    private let textView = NSTextView()

    init() {
        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 600, height: 480),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        win.title = "ClipNoteX — DONE LOG"
        win.center()

        super.init(window: win)
        win.delegate = self

        let scroll = NSScrollView(frame: win.contentView!.bounds)
        scroll.autoresizingMask = [.width, .height]
        scroll.hasVerticalScroller = true
        scroll.documentView = textView
        textView.isEditable = false
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.autoresizingMask = [.width]
        textView.textContainer?.widthTracksTextView = true
        textView.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        win.contentView?.addSubview(scroll)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reload() {
        guard let json = cnx_list_done_json(nil, 100) else {
            textView.string = "(failed to load DONE LOG)"
            return
        }
        defer { cnx_free_string(json) }
        textView.string = String(cString: json)
    }

    func windowWillClose(_ notification: Notification) {
        Self.instance = nil
    }
}
