// ShortcutRecorder.swift — minimal NSControl that captures a single
// global-hotkey accelerator string (e.g. "Cmd+Shift+V").
//
// Behaviour (mirrors MASShortcut / KeyboardShortcuts feel):
//   - Click ⇒ enter "recording" mode (text turns to "Press shortcut…")
//   - Press any non-modifier key while at least one modifier is held
//     ⇒ accept the shortcut, emit `onChange`, leave recording mode
//   - Escape ⇒ cancel, restore previous value
//   - Click again ⇒ clear (set to nil) and emit `onChange(nil)`
//
// Accelerator string format matches what `global-hotkey` expects:
//   "Cmd", "Ctrl", "Shift", "Alt" joined by "+" plus a non-modifier key.

import AppKit
import Carbon.HIToolbox

final class ShortcutRecorder: NSButton {
    var onChange: ((String?) -> Void)?
    private(set) var accelerator: String? {
        didSet { updateLabel() }
    }

    private var isRecording: Bool = false {
        didSet { updateLabel() }
    }
    private var monitor: Any?

    init(initial: String?) {
        self.accelerator = initial
        super.init(frame: .zero)
        self.bezelStyle = .rounded
        self.setButtonType(.momentaryPushIn)
        self.target = self
        self.action = #selector(buttonPressed)
        self.widthAnchor.constraint(greaterThanOrEqualToConstant: 180).isActive = true
        updateLabel()
    }
    required init?(coder: NSCoder) { fatalError() }

    deinit { stopRecording() }

    // MARK: - State

    @objc private func buttonPressed() {
        if isRecording {
            stopRecording(cancel: true)
        } else if accelerator != nil {
            // 2-click semantics: first click clears, then next click records.
            // We collapse it: a click on a populated recorder clears it.
            accelerator = nil
            onChange?(nil)
        } else {
            startRecording()
        }
    }

    private func startRecording() {
        guard !isRecording else { return }
        isRecording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { [weak self] ev in
            guard let self else { return ev }
            if ev.type == .keyDown {
                // Escape cancels
                if ev.keyCode == UInt16(kVK_Escape) {
                    self.stopRecording(cancel: true)
                    return nil
                }
                let mods = ev.modifierFlags.intersection(.deviceIndependentFlagsMask)
                let hasMod = mods.contains(.command) || mods.contains(.control) ||
                             mods.contains(.option) || mods.contains(.shift)
                guard hasMod else { return nil } // require at least one modifier
                guard let key = Self.keyName(for: ev) else { return nil }
                let parts = Self.modifierParts(mods) + [key]
                let acc = parts.joined(separator: "+")
                self.accelerator = acc
                self.onChange?(acc)
                self.stopRecording()
                return nil
            }
            return ev
        }
    }

    private func stopRecording(cancel: Bool = false) {
        if let m = monitor {
            NSEvent.removeMonitor(m)
            monitor = nil
        }
        isRecording = false
        _ = cancel // explicit
    }

    private func updateLabel() {
        if isRecording {
            self.title = "Press shortcut…  (esc to cancel)"
        } else if let a = accelerator {
            self.title = a + "   ⌫ to clear"
        } else {
            self.title = "Click to record"
        }
    }

    // MARK: - Helpers

    private static func modifierParts(_ mods: NSEvent.ModifierFlags) -> [String] {
        var out: [String] = []
        if mods.contains(.control) { out.append("Ctrl") }
        if mods.contains(.option)  { out.append("Alt") }
        if mods.contains(.shift)   { out.append("Shift") }
        if mods.contains(.command) { out.append("Cmd") }
        return out
    }

    private static func keyName(for ev: NSEvent) -> String? {
        // Map keycodes for letter / digit / function keys to global-hotkey names.
        if let s = letterKeyName(keyCode: Int(ev.keyCode)) { return s }
        if let s = functionKeyName(keyCode: Int(ev.keyCode)) { return s }
        if let s = specialKeyName(keyCode: Int(ev.keyCode)) { return s }
        // Fallback: take the printable character upper-cased
        if let chars = ev.charactersIgnoringModifiers?.uppercased(), !chars.isEmpty {
            return chars
        }
        return nil
    }

    private static func letterKeyName(keyCode: Int) -> String? {
        // ANSI virtual keycodes for A-Z, 0-9
        let letters: [Int: String] = [
            kVK_ANSI_A: "A", kVK_ANSI_B: "B", kVK_ANSI_C: "C", kVK_ANSI_D: "D",
            kVK_ANSI_E: "E", kVK_ANSI_F: "F", kVK_ANSI_G: "G", kVK_ANSI_H: "H",
            kVK_ANSI_I: "I", kVK_ANSI_J: "J", kVK_ANSI_K: "K", kVK_ANSI_L: "L",
            kVK_ANSI_M: "M", kVK_ANSI_N: "N", kVK_ANSI_O: "O", kVK_ANSI_P: "P",
            kVK_ANSI_Q: "Q", kVK_ANSI_R: "R", kVK_ANSI_S: "S", kVK_ANSI_T: "T",
            kVK_ANSI_U: "U", kVK_ANSI_V: "V", kVK_ANSI_W: "W", kVK_ANSI_X: "X",
            kVK_ANSI_Y: "Y", kVK_ANSI_Z: "Z",
            kVK_ANSI_0: "0", kVK_ANSI_1: "1", kVK_ANSI_2: "2", kVK_ANSI_3: "3",
            kVK_ANSI_4: "4", kVK_ANSI_5: "5", kVK_ANSI_6: "6", kVK_ANSI_7: "7",
            kVK_ANSI_8: "8", kVK_ANSI_9: "9",
        ]
        return letters[keyCode]
    }

    private static func functionKeyName(keyCode: Int) -> String? {
        let fns: [Int: String] = [
            kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4",
            kVK_F5: "F5", kVK_F6: "F6", kVK_F7: "F7", kVK_F8: "F8",
            kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
        ]
        return fns[keyCode]
    }

    private static func specialKeyName(keyCode: Int) -> String? {
        let specials: [Int: String] = [
            kVK_Space: "Space",
            kVK_Return: "Enter",
            kVK_Tab: "Tab",
            kVK_LeftArrow: "Left",
            kVK_RightArrow: "Right",
            kVK_UpArrow: "Up",
            kVK_DownArrow: "Down",
            kVK_Delete: "Backspace",
            kVK_ForwardDelete: "Delete",
        ]
        return specials[keyCode]
    }
}
