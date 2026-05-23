// PreferencesWindow — 設定画面 (v0.2 ベーシック版)。
//
// 構成:
//   タブ:
//     - General : 履歴上限、起動時の振る舞い
//     - Shortcuts : 現在のショートカット一覧 (read-only display in v0.2)
//     - Privacy : 除外アプリ一覧 (read-only display in v0.2)
//     - About   : バージョン / ライセンス
//
// 設定の永続化は v0.3 で UserDefaults + Rust 側 Settings 連携。
// 今は表示のみ + 一部即時反映 (履歴上限) で骨格を作る。

import AppKit
import Foundation
import ClipNoteXCore

final class PreferencesWindow: NSWindowController, NSWindowDelegate {
    static private(set) var instance: PreferencesWindow?

    static func show() {
        if instance == nil { instance = PreferencesWindow() }
        guard let i = instance else { return }
        NSApp.activate(ignoringOtherApps: true)
        i.showWindow(nil)
        i.window?.makeKeyAndOrderFront(nil)
    }

    init() {
        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 520, height: 380),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        win.title = "ClipNoteX Preferences"
        win.center()
        super.init(window: win)
        win.delegate = self
        setupTabs()
    }
    required init?(coder: NSCoder) { fatalError() }

    private func setupTabs() {
        guard let content = window?.contentView else { return }
        let tabView = NSTabView(frame: content.bounds)
        tabView.autoresizingMask = [.width, .height]

        tabView.addTabViewItem(makeTab(label: "General",     view: generalView()))
        tabView.addTabViewItem(makeTab(label: "Shortcuts",   view: shortcutsView()))
        tabView.addTabViewItem(makeTab(label: "Privacy",     view: privacyView()))
        tabView.addTabViewItem(makeTab(label: "Maintenance", view: maintenanceView()))
        tabView.addTabViewItem(makeTab(label: "About",       view: aboutView()))

        content.addSubview(tabView)
    }

    private func makeTab(label: String, view: NSView) -> NSTabViewItem {
        let item = NSTabViewItem()
        item.label = label
        item.view = view
        return item
    }

    // MARK: - General

    private var quotaField: NSTextField?
    private var quotaStepper: NSStepper?
    private var launchAtLoginCheck: NSButton?

    private func generalView() -> NSView {
        let v = NSView()
        let title = NSTextField(labelWithString: "Clipboard history")
        title.font = .systemFont(ofSize: 13, weight: .semibold)

        let quotaLbl = NSTextField(labelWithString: "Maximum items kept:")
        let field = NSTextField()
        field.alignment = .right
        field.formatter = NumberFormatter()
        field.intValue = Int32(Settings.historyQuota)
        field.target = self
        field.action = #selector(quotaChanged)
        field.widthAnchor.constraint(equalToConstant: 80).isActive = true
        quotaField = field

        let stepper = NSStepper()
        stepper.minValue = 50
        stepper.maxValue = 50_000
        stepper.increment = 50
        stepper.integerValue = Settings.historyQuota
        stepper.target = self
        stepper.action = #selector(stepperChanged)
        quotaStepper = stepper

        let info = NSTextField(wrappingLabelWithString:
            "Older items are evicted automatically when the cap is reached. Pinned items survive eviction. Range: 50–50,000.")
        info.textColor = .secondaryLabelColor
        info.font = .systemFont(ofSize: 11)

        let appTitle = NSTextField(labelWithString: "Startup")
        appTitle.font = .systemFont(ofSize: 13, weight: .semibold)

        let launchCheck = NSButton(checkboxWithTitle: "Launch at login",
                                   target: self,
                                   action: #selector(launchAtLoginToggled))
        launchCheck.state = Settings.launchAtLogin ? .on : .off
        launchAtLoginCheck = launchCheck

        let quotaRow = NSStackView(views: [quotaLbl, field, stepper])
        quotaRow.orientation = .horizontal
        quotaRow.spacing = 6

        let stack = NSStackView(views: [
            title, quotaRow, info,
            NSBox.separator(),
            appTitle, launchCheck,
        ])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 20, bottom: 16, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(stack)
        pin(stack, to: v)
        return v
    }

    @objc private func quotaChanged() {
        let v = Int(quotaField?.intValue ?? 1000)
        Settings.historyQuota = v
        quotaStepper?.integerValue = Settings.historyQuota
        quotaField?.intValue = Int32(Settings.historyQuota) // clamped
    }

    @objc private func stepperChanged() {
        let v = quotaStepper?.integerValue ?? 1000
        Settings.historyQuota = v
        quotaField?.intValue = Int32(Settings.historyQuota)
    }

    @objc private func launchAtLoginToggled() {
        Settings.launchAtLogin = (launchAtLoginCheck?.state == .on)
    }

    // MARK: - Shortcuts

    private func shortcutsView() -> NSView {
        let v = NSView()
        let entries: [(String, String)] = [
            ("Open clipboard popup",    "⌘⇧V"),
            ("Capture to DONE LOG",     "⌘⇧D"),
            ("Quick paste (in popup)",  "1 – 9"),
            ("Navigate (in popup)",     "↑ ↓"),
            ("Paste selected",          "⏎"),
            ("Close popup",             "⎋"),
        ]
        let rows: [NSView] = entries.map { (action, key) in
            let l = NSTextField(labelWithString: action)
            let r = NSTextField(labelWithString: key)
            r.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
            r.alignment = .right
            return hRow(l, r)
        }
        let note = NSTextField(wrappingLabelWithString:
            "Custom shortcut editing is planned for v0.3.")
        note.textColor = .secondaryLabelColor
        note.font = .systemFont(ofSize: 11)

        let stack = NSStackView(views: rows + [note])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 6
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 20, bottom: 16, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(stack)
        pin(stack, to: v)
        return v
    }

    // MARK: - Privacy

    private func privacyView() -> NSView {
        let v = NSView()
        let title = NSTextField(labelWithString: "Default exclusions")
        title.font = .systemFont(ofSize: 13, weight: .semibold)
        let body = NSTextField(wrappingLabelWithString:
            "ClipNoteX never captures clipboard content originating from these apps:")
        body.textColor = .secondaryLabelColor
        body.font = .systemFont(ofSize: 11)
        let list = NSTextField(wrappingLabelWithString:
            "• 1Password\n• Bitwarden\n• KeePassXC")
        list.font = .systemFont(ofSize: 12)

        let kind = NSTextField(labelWithString: "Also discarded automatically:")
        kind.font = .systemFont(ofSize: 13, weight: .semibold)
        let kindBody = NSTextField(wrappingLabelWithString:
            "• Pasteboard entries marked Concealed / Transient / AutoGenerated\n• Empty clipboards")
        kindBody.textColor = .secondaryLabelColor
        kindBody.font = .systemFont(ofSize: 11)

        let stack = NSStackView(views: [title, body, list, NSBox.separator(), kind, kindBody])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 20, bottom: 16, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(stack)
        pin(stack, to: v)
        return v
    }

    // MARK: - Maintenance (destructive: data reset)

    private func maintenanceView() -> NSView {
        let v = NSView()
        let title = NSTextField(labelWithString: "Reset all data")
        title.font = .systemFont(ofSize: 13, weight: .semibold)

        let desc = NSTextField(wrappingLabelWithString:
            "Erases every clipboard history entry, every DONE LOG entry, and all on-disk blob files. " +
            "Use this if the encryption state has gotten out of sync (for example, you see large numbers of " +
            "‘decrypt failed for item, skipping’ warnings in the log) and the history list shows nothing despite " +
            "new copies. The current encryption key is kept, so new captures keep working immediately.\n\n" +
            "⚠ This cannot be undone.")
        desc.textColor = .secondaryLabelColor
        desc.font = .systemFont(ofSize: 11)

        let resetBtn = NSButton(title: "Reset all data…", target: self, action: #selector(resetAllDataTapped))
        resetBtn.bezelStyle = .rounded
        if #available(macOS 11.0, *) {
            resetBtn.contentTintColor = .systemRed
        }

        let stack = NSStackView(views: [title, desc, resetBtn])
        stack.orientation = .vertical
        stack.alignment = NSLayoutConstraint.Attribute.left
        stack.spacing = 12
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 20, bottom: 16, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(stack)
        pin(stack, to: v)
        return v
    }

    @objc private func resetAllDataTapped() {
        let alert = NSAlert()
        alert.messageText = "Erase all ClipNoteX data?"
        alert.informativeText = "This will permanently delete all clipboard history, DONE LOG entries, " +
            "and saved blob files. The action cannot be undone."
        alert.alertStyle = .critical
        alert.addButton(withTitle: "Erase everything")
        alert.addButton(withTitle: "Cancel")
        // make Cancel the default to discourage accidents
        alert.buttons[0].keyEquivalent = ""
        alert.buttons[1].keyEquivalent = "\r"

        guard alert.runModal() == .alertFirstButtonReturn else { return }

        let rc = cnx_reset_data()
        if rc == 0 {
            let ok = NSAlert()
            ok.messageText = "Done."
            ok.informativeText = "All data has been erased."
            ok.runModal()
        } else {
            let err = NSAlert()
            err.messageText = "Reset failed (code \(rc))."
            if let msg = cnx_last_error() {
                err.informativeText = String(cString: msg)
                cnx_free_string(msg)
            }
            err.alertStyle = .warning
            err.runModal()
        }
    }

    // MARK: - About

    private func aboutView() -> NSView {
        let v = NSView()
        let appName = NSTextField(labelWithString: "ClipNoteX")
        appName.font = .systemFont(ofSize: 22, weight: .semibold)

        let version = NSTextField(labelWithString: "Version 0.1.0-dev")
        version.textColor = .secondaryLabelColor

        let blurb = NSTextField(wrappingLabelWithString:
            "Encrypted, offline-only clipboard manager.\nMIT © 2026 suzuki-black.")
        blurb.alignment = .center

        let repoBtn = NSButton(title: "GitHub", target: self, action: #selector(openRepo))
        repoBtn.bezelStyle = .rounded

        let stack = NSStackView(views: [appName, version, blurb, repoBtn])
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = 12
        stack.edgeInsets = NSEdgeInsets(top: 24, left: 20, bottom: 24, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(stack)
        pin(stack, to: v)
        return v
    }

    @objc private func openRepo() {
        if let url = URL(string: "https://github.com/suzuki-black/ClipNoteX") {
            NSWorkspace.shared.open(url)
        }
    }

    // MARK: - Layout helpers

    private func hRow(_ left: NSView, _ right: NSView) -> NSView {
        let h = NSStackView(views: [left, NSView(), right])
        h.orientation = .horizontal
        h.spacing = 12
        h.distribution = .fill
        return h
    }

    private func pin(_ inner: NSView, to outer: NSView) {
        NSLayoutConstraint.activate([
            inner.topAnchor.constraint(equalTo: outer.topAnchor),
            inner.leadingAnchor.constraint(equalTo: outer.leadingAnchor),
            inner.trailingAnchor.constraint(equalTo: outer.trailingAnchor),
            inner.bottomAnchor.constraint(lessThanOrEqualTo: outer.bottomAnchor),
        ])
    }

    // MARK: - NSWindowDelegate

    func windowWillClose(_ notification: Notification) {
        Self.instance = nil
    }
}

private extension NSBox {
    static func separator() -> NSBox {
        let b = NSBox()
        b.boxType = .separator
        return b
    }
}
