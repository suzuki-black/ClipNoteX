<div align="center">

# 📋 ClipNoteX

**The clipboard manager that doesn't phone home — and can't be made to.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.82+-orange?logo=rust)](https://www.rust-lang.org)
[![Swift](https://img.shields.io/badge/Swift-5.9+-FA7343?logo=swift)](https://swift.org)
[![macOS](https://img.shields.io/badge/macOS-13+-lightgrey?logo=apple)](#)
[![Version](https://img.shields.io/badge/version-0.1.0--dev-brightgreen)](#)

<br/>

*Copy anything. Find it instantly. Paste it perfectly. Log what you did.*

</div>

---

## Why ClipNoteX?

Most clipboard managers either **sync to the cloud** (privacy risk), **skip encryption** (security risk), or **don't let you reformat** what you paste (productivity loss).

ClipNoteX does all three right — entirely offline, with military-grade encryption baked in from day one, in a tiny **menubar-only native app** (no Electron, no WebView, no Dock icon).

| | ClipNoteX | Clipy / Pasta | Cloud-based managers |
|---|---|---|---|
| **Encrypted at rest** | ✅ XChaCha20-Poly1305 | ❌ Plaintext | ⚠️ Server-side |
| **100% local / offline** | ✅ | ✅ | ❌ |
| **Format on paste** | ✅ JSON · SQL · MD | ❌ | ❌ |
| **Password manager aware** | ✅ Auto-exclude | ❌ | ⚠️ |
| **Work log (DONE LOG)** | ✅ Built-in | ❌ | ❌ |
| **Native macOS (no WebView)** | ✅ AppKit | ✅ | ❌ Electron |
| **Open source** | ✅ MIT | ✅ | ❌ |

---

## Features

- **🔒 Encrypted history** — Every entry is encrypted with XChaCha20-Poly1305. Keys are derived via Argon2id and stored in your OS keychain.
- **⚡ Instant native UI** — NSStatusItem + NSMenu, popup in milliseconds, zero WebView overhead.
- **✨ Format Paste** — Copy messy JSON, paste perfect JSON. Supports JSON / SQL / Markdown / plain text.
- **🔑 Password manager safe** — 1Password / Bitwarden / KeePassXC windows are automatically excluded.
- **📓 DONE LOG** — Turn any clipboard item into a work log entry. Edit, tag, export as Markdown.
- **📌 Pin & protect** — Pin important clips, survive quota eviction.
- **⌨️ Keyboard-first** — Global hotkeys, number-key quick-paste from the menu.

---

## Architecture

```
ClipNoteX/
├── crates/                       ← Pure Rust core (shared across OS)
│   ├── clipnotex-core/           Shared types, event bus, settings
│   ├── clipnotex-clipboard/      OS clipboard backend (NSPasteboard / Win32)
│   ├── clipnotex-store/          Encrypted redb storage
│   ├── clipnotex-donelog/        DONE LOG store + Markdown export
│   ├── clipnotex-paste/          Paste controller + format application
│   ├── clipnotex-format/         Text formatters (JSON / SQL / Markdown)
│   ├── clipnotex-hotkey/         Global hotkey registration
│   ├── clipnotex-app/            Capture loop, quota, filter
│   └── clipnotex-ffi/            ★ C ABI bridge (cbindgen → ClipNoteX.h)
└── apps/
    └── macos/                    ← Swift + AppKit shell (SPM)
        ├── Package.swift
        ├── Info.plist
        ├── build-app.sh           # → builds ClipNoteX.app
        └── Sources/
            ├── ClipNoteXCore/     # systemLibrary wrapping ClipNoteX.h
            └── ClipNoteX/
                ├── main.swift
                ├── AppDelegate.swift
                ├── StatusBarController.swift     (menubar + popup menu)
                └── DoneLogWindow.swift            (DONE LOG window)
```

**Tech stack:**

| Layer | Technology |
|-------|-----------|
| UI (macOS) | Swift 5.9 + AppKit (NSStatusItem · NSMenu · NSTableView) |
| Core | Rust + Tokio |
| FFI | cbindgen-generated C header + static library |
| Storage | [redb](https://github.com/cberner/redb) (embedded KV) |
| Encryption | XChaCha20-Poly1305 · Argon2id · BLAKE3 |
| Clipboard | NSPasteboard via [objc2](https://github.com/madsmtm/objc2) |

The same Rust crates will power a future **Windows** frontend (C# WinUI 3 against the same `libclipnotex_ffi.a`).

> **Note**: an earlier prototype used Tauri 2 for the UI shell but was retired
> at tag [`v0.1-tauri-legacy`](../../releases) — the WebView model could not
> deliver the non-activating menubar UX that clipboard managers need on macOS.

---

## Quick Start

### Requirements

- **macOS 13+**
- [Rust](https://rustup.rs/) 1.82+
- Xcode Command Line Tools (Swift 5.9+)

### Build & run

```bash
# 1) Build everything (Rust staticlib → Swift → .app bundle)
cd apps/macos
./build-app.sh

# 2) Launch the app
open build/ClipNoteX.app

# Or run from terminal (logs to stderr — useful while developing)
./build/ClipNoteX.app/Contents/MacOS/ClipNoteX
```

A 📋 icon appears in the menu bar. The first time you press `Cmd+Shift+V`, macOS will ask for **Accessibility permission** (required to simulate the paste keystroke).

### Development workflow

```bash
# Rust only (fast iteration on the core)
cargo build -p clipnotex-ffi
cargo test --workspace

# Swift only (assumes Rust .a is up-to-date)
cd apps/macos
swift build

# Full debug build of the .app
cd apps/macos
./build-app.sh --debug
```

The cbindgen build script regenerates `crates/clipnotex-ffi/include/ClipNoteX.h` on every cargo build; `build-app.sh` copies it into the Swift module.

---

## Keyboard Shortcuts

### Global (macOS)

| Key | Action |
|-----|--------|
| `⌘⇧V` | Open clipboard history menu at the menu bar icon |
| `⌘⇧D` | Capture current clipboard into DONE LOG |

### In the popup menu

| Key | Action |
|-----|--------|
| `1`–`9` | Paste the n-th history item |
| `↑` `↓` | Navigate items (NSMenu native) |
| `⏎` | Paste selected item |
| `⎋` | Close menu |

---

## Security

ClipNoteX was designed with security as a constraint, not an afterthought.

- **XChaCha20-Poly1305 AEAD** — authenticated encryption for every stored clip
- **Argon2id KDF** — key derivation resistant to GPU/ASIC attacks
- **BLAKE3** — fast, collision-resistant content hashing
- **macOS Keychain integration** — encryption keys live in the Keychain
- **Concealed Pasteboard** — `org.nspasteboard.ConcealedType` entries are discarded
- **Self-write guard** — prevents the app from re-capturing its own paste output
- **Zero network I/O** — the binary makes no outbound connections
- **No Dock icon** (`LSUIElement = true`) — runs unobtrusively in the menu bar

**Default exclusion list** (never captured, ever):

| App | Match type |
|-----|-----------|
| 1Password | Bundle ID + exe name |
| Bitwarden | Bundle ID + exe name |
| KeePassXC | Bundle ID + exe name |

---

## Data flow

```
Clipboard change
  └─ MacWatcher (100ms poll)
       └─ ExclusionFilter        ← blocks password managers
            └─ StoreService      ← encrypts + persists
                 └─ EventBus
                      ├─ QuotaManager   ← evicts old clips
                      └─ FFI callback   ← notifies Swift to refresh UI
```

---

## Roadmap

- [ ] Settings UI (shortcuts, exclusion rules, quota)
- [ ] Search field in history menu (Maccy-style)
- [ ] Format Paste live preview UI
- [ ] Image thumbnail support
- [ ] Code signing + Notarization
- [ ] **Windows port** (same Rust core + C# WinUI 3 frontend)
- [ ] iCloud / local sync between Macs (opt-in, encrypted)
- [ ] Plugin API for custom formatters

---

## Contributing

PRs and issues welcome!

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

---

## License

MIT © 2026 suzuki-black — see [LICENSE](LICENSE).

Third-party crates / packages are used under their respective licenses (MIT / Apache-2.0).

---

<div align="center">

# 📋 ClipNoteX（日本語）

**クラウドに送らない。暗号化しないなんてあり得ない。Electron 不要のネイティブ実装。**

</div>

---

## 主な機能

- **🔒 暗号化履歴** — XChaCha20-Poly1305 で全エントリを暗号化、鍵は macOS Keychain
- **⚡ ネイティブ UI** — `NSStatusItem` + `NSMenu` ベース、WebView なしで瞬時表示
- **✨ フォーマットペースト** — JSON / SQL / Markdown 整形してペースト
- **🔑 パスワードマネージャー除外** — 1Password / Bitwarden / KeePassXC は自動的にキャプチャしない
- **📓 DONE LOG** — クリップボードを作業ログに。タグ・編集・Markdown エクスポート
- **📌 ピン留め** — 重要アイテムをクォータ削除から保護

---

## アーキテクチャ

- **Rust コア** (`crates/`) — 暗号化・ストア・キャプチャ・ペースト・フォーマット等のロジック全部
- **C ABI 層** (`crates/clipnotex-ffi`) — cbindgen が `ClipNoteX.h` を自動生成
- **macOS フロント** (`apps/macos`) — Swift + AppKit。`NSStatusItem` + `NSMenu` で Clipy / Maccy と同じパターン
- 将来の **Windows フロント** は同じ staticlib を C# WinUI 3 から叩く想定

---

## クイックスタート

```bash
git clone https://github.com/suzuki-black/ClipNoteX.git
cd ClipNoteX/apps/macos
./build-app.sh
open build/ClipNoteX.app
```

初回 `⌘⇧V` 押下時に macOS が**アクセシビリティ権限**を要求します。

---

## キーボードショートカット

### グローバル

| キー | 動作 |
|-----|------|
| `⌘⇧V` | 履歴メニューを開く |
| `⌘⇧D` | 現在のクリップボードを DONE LOG にキャプチャ |

### ポップアップメニュー内

| キー | 動作 |
|-----|------|
| `1`〜`9` | n 番目のアイテムをペースト |
| `↑` `↓` | アイテム選択（NSMenu 標準） |
| `⏎` | ペースト |
| `⎋` | メニューを閉じる |

---

## セキュリティ設計

- **XChaCha20-Poly1305 AEAD** — 全データを認証付き暗号化
- **Argon2id KDF** — GPU/ASIC 耐性のある鍵導出
- **macOS Keychain** に暗号鍵を保管
- **ネットワーク通信なし** — バイナリは一切の外部通信をしない
- **デフォルト除外リスト** — 1Password / Bitwarden / KeePassXC は常に除外
- **Dock 非表示** (`LSUIElement = true`) — メニューバー常駐型

---

## ライセンス

MIT © 2026 suzuki-black
