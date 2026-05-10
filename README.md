<div align="center">

# 📋 ClipNoteX

**The clipboard manager that doesn't phone home — and can't be made to.**

[![CI](https://github.com/suzuki-black/ClipNoteX/actions/workflows/ci.yml/badge.svg)](https://github.com/suzuki-black/ClipNoteX/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-blue?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-1.82+-orange?logo=rust)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Windows-lightgrey)](#)
[![Version](https://img.shields.io/badge/version-0.1.0--dev-brightgreen)](#)

<br/>

*Copy anything. Find it instantly. Paste it perfectly. Log what you did.*

</div>

---

## Why ClipNoteX?

Most clipboard managers either **sync to the cloud** (privacy risk), **skip encryption** (security risk), or **don't let you reformat** what you paste (productivity loss).

ClipNoteX does all three right — entirely offline, with military-grade encryption baked in from day one.

| | ClipNoteX | Clipy / Pasta | Cloud-based managers |
|---|---|---|---|
| **Encrypted at rest** | ✅ XChaCha20-Poly1305 | ❌ Plaintext | ⚠️ Server-side |
| **100% local / offline** | ✅ | ✅ | ❌ |
| **Format on paste** | ✅ JSON · SQL · MD | ❌ | ❌ |
| **Password manager aware** | ✅ Auto-exclude | ❌ | ⚠️ |
| **Work log (DONE LOG)** | ✅ Built-in | ❌ | ❌ |
| **Open source** | ✅ MIT | ✅ | ❌ |

---

## Features

- **🔒 Encrypted history** — Every entry is encrypted with XChaCha20-Poly1305. Keys are derived via Argon2id and stored in your OS keychain. Your clips stay yours.
- **⚡ Instant search** — Full-text search across up to 1,000 entries in real time.
- **✨ Format Paste** — Copy messy JSON, paste perfect JSON. Supports JSON, SQL, Markdown, and plain text — with live preview before you paste.
- **🔑 Password manager safe** — 1Password, Bitwarden, and KeePassXC windows are automatically excluded. Always.
- **📓 DONE LOG** — Turn any clipboard item into a work log entry. Edit, tag, and export your daily digest as Markdown. Built for developers who want a `git log` for their brain.
- **📌 Pin & protect** — Pin important clips. They survive quota eviction.
- **⌨️ Keyboard-first** — Navigate, paste, delete, and format without touching the mouse.

---

## Demo

```
┌────────────────────────────────────────────┐
│  📋 Clipboard History  │  📓 DONE LOG      │
├────────────────────────────────────────────┤
│  🔍  Search...                             │
├────────────────────────────────────────────┤
│ ▶  {"name":"test","value":42,"list":[...   │  ← JSON, auto-detected
│    VS Code  ·  just now                    │
├────────────────────────────────────────────┤
│    SELECT * FROM users WHERE id = 1        │  ← SQL
│    Terminal  ·  2 min ago                  │
├────────────────────────────────────────────┤
│    Fixed the race condition in capture ... │
│    Slack  ·  12:30                         │
└────────────────────────────────────────────┘
  42 items   ↑↓ navigate · ↵ paste · ⇧↵ plain · ⌥↵ format
```

**Format Paste in action:**

```
Before:  {"name":"test","value":42,"list":[1,2,3]}

After (⌥↵ → ⌘↵):
{
  "name": "test",
  "value": 42,
  "list": [
    1,
    2,
    3
  ]
}
```

---

## Quick Start

### Requirements

- **macOS** 13+ or **Windows** 10+
- [Rust](https://rustup.rs/) 1.82+
- Node.js 20+ and npm

### Run in development

```bash
git clone https://github.com/suzuki-black/ClipNoteX.git
cd ClipNoteX/apps/desktop
npm install
npm run tauri dev
```

> On first launch macOS will ask for **Accessibility permission** — this is required to monitor clipboard changes and simulate paste keystrokes.

### Build for release

```bash
npm run tauri build
# Output: apps/desktop/src-tauri/target/release/bundle/
```

---

## Keyboard Shortcuts

### In-window

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate items |
| `Enter` | Paste (with formatting) |
| `Shift+Enter` | Paste as plain text |
| `Alt+Enter` | Open Format Paste modal |
| `Backspace` | Delete selected item |
| `Escape` | Clear search |

### Global (macOS)

| Key | Action |
|-----|--------|
| `⌘⇧V` | Show history window |
| `⌘⌃V` | Paste plain text |
| `⌘⌥V` | Format paste |
| `⌘⇧D` | Capture to DONE LOG |

---

## Security

ClipNoteX was designed with security as a constraint, not an afterthought.

- **XChaCha20-Poly1305 AEAD** — authenticated encryption for every stored clip
- **Argon2id KDF** — key derivation resistant to GPU/ASIC attacks
- **BLAKE3** — fast, collision-resistant content hashing
- **OS Keychain integration** — encryption keys live in macOS Keychain / Windows Credential Store
- **Concealed Pasteboard** — macOS `ConcealedType`/`TransientType` entries are automatically discarded
- **Self-write guard** — prevents the app from re-capturing its own paste operations
- **Zero network I/O** — the binary makes no outbound connections

**Default exclusion list** (never captured, ever):

| App | Match type |
|-----|-----------|
| 1Password | Bundle ID + exe name |
| Bitwarden | Bundle ID + exe name |
| KeePassXC | Bundle ID + exe name |

---

## Architecture

```
ClipNoteX/
├── apps/desktop/           # Tauri app shell
│   └── src/                # React 18 + TypeScript UI
└── crates/
    ├── clipnotex-core/     # Shared types, event bus, settings
    ├── clipnotex-clipboard/# OS clipboard backend (NSPasteboard / Win32)
    ├── clipnotex-store/    # Encrypted redb storage
    ├── clipnotex-donelog/  # DONE LOG store + Markdown export
    ├── clipnotex-paste/    # Paste controller + format application
    ├── clipnotex-format/   # Text formatters (JSON/SQL/Markdown)
    ├── clipnotex-hotkey/   # Global hotkey registration
    ├── clipnotex-app/      # Capture loop, quota management
    └── clipnotex-tauri/    # Tauri commands, composition root
```

**Data flow:**

```
Clipboard change
  └─ MacWatcher (100ms poll)
       └─ ExclusionFilter        ← blocks password managers
            └─ StoreService      ← encrypts + persists
                 └─ EventBus
                      ├─ QuotaManager   ← evicts old clips
                      └─ Frontend       ← updates list
```

**Tech stack:**

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri 2](https://tauri.app) |
| Backend | Rust + Tokio |
| Frontend | React 18 + TypeScript + Vite |
| Storage | [redb](https://github.com/cberner/redb) (embedded KV) |
| Encryption | XChaCha20-Poly1305 · Argon2id · BLAKE3 |
| macOS clipboard | NSPasteboard via [objc2](https://github.com/madsmtm/objc2) |

---

## Roadmap

- [ ] Settings UI (shortcuts, exclusion rules, quota)
- [ ] System tray integration
- [ ] Image thumbnail support
- [ ] iCloud / local sync between Macs (opt-in, encrypted)
- [ ] Plugin API for custom formatters
- [ ] iOS Shortcut integration (DONE LOG)

---

## Contributing

PRs and issues are welcome! Please read the design notes in [`DESIGN.md`](DESIGN.md) before proposing major changes.

```bash
# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

---

## License

MIT © 2026 suzuki-black — see [LICENSE](LICENSE) for details.

Third-party crates and packages are used under their respective licenses (MIT / Apache-2.0).

---

---

<div align="center">

# 📋 ClipNoteX（日本語）

**クラウドに送らない。暗号化しないなんてあり得ない。そんなクリップボードマネージャー。**

</div>

---

## なぜ ClipNoteX？

クリップボードマネージャーのほとんどは **クラウド同期でプライバシーが危ない**か、**暗号化なしでセキュリティが危ない**か、**ペースト時に整形できない**かのどれかです。

ClipNoteX は3つすべてを解決します。完全オフライン・暗号化必須・整形ペースト標準搭載。

---

## 主な機能

- **🔒 暗号化履歴** — XChaCha20-Poly1305 で全エントリを暗号化。鍵は Argon2id で導出し OS キーチェーンに保存。
- **⚡ 即時検索** — 最大 1,000 件をリアルタイム全文検索。
- **✨ フォーマットペースト** — 崩れた JSON をコピーして、整形済み JSON をペースト。JSON・SQL・Markdown に対応。ペースト前のライブプレビュー付き。
- **🔑 パスワードマネージャー除外** — 1Password・Bitwarden・KeePassXC のウィンドウからは自動的にキャプチャしない。
- **📓 DONE LOG** — クリップボードの内容をそのまま作業ログに。タグ付け・編集・Markdown エクスポートに対応。
- **📌 ピン留め** — 重要アイテムをピン留めしてクォータ削除から保護。
- **⌨️ キーボードファースト** — マウスなしで全操作が完結。

---

## クイックスタート

```bash
git clone https://github.com/suzuki-black/ClipNoteX.git
cd ClipNoteX/apps/desktop
npm install
npm run tauri dev
```

> macOS では初回起動時に**アクセシビリティ権限**の許可が必要です。

---

## キーボードショートカット

### ウィンドウ内

| キー | 動作 |
|-----|------|
| `↑` / `↓` | アイテム選択 |
| `Enter` | ペースト（書式保持） |
| `Shift+Enter` | プレーンテキストでペースト |
| `Alt+Enter` | フォーマットペーストモーダルを開く |
| `Backspace` | 選択アイテムを削除 |
| `Escape` | 検索クリア |

### グローバル（macOS）

| キー | 動作 |
|-----|------|
| `⌘⇧V` | 履歴ウィンドウを表示 |
| `⌘⌃V` | プレーンテキストペースト |
| `⌘⌥V` | フォーマットペースト |
| `⌘⇧D` | DONE LOG にキャプチャ |

---

## セキュリティ設計

- **XChaCha20-Poly1305 AEAD** — 全データを認証付き暗号化
- **Argon2id KDF** — GPU/ASIC 耐性のある鍵導出
- **OS キーチェーン連携** — macOS Keychain / Windows Credential Store に暗号鍵を保管
- **ネットワーク通信なし** — バイナリは一切の外部通信をしない
- **デフォルト除外リスト** — 1Password・Bitwarden・KeePassXC は常に除外

---

## ライセンス

MIT © 2026 suzuki-black
