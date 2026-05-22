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
- **⚡ Instant native UI** — NSStatusItem + non-activating NSPanel popup. Pops in milliseconds, never steals focus from the app you're pasting into.
- **✨ Format Paste** — Copy messy JSON, paste perfect JSON. Live preview for JSON / SQL / Markdown / plain text / HTML / CSS / JS / TS.
- **🔑 Password manager safe** — 1Password / Bitwarden / KeePassXC windows are automatically excluded.
- **📓 DONE LOG** — Turn any clipboard item into a work log entry. Edit, tag, delete, export as Markdown.
- **📌 Pin & protect** — Pin important clips, survive quota eviction.
- **🗄 BlobStore for large payloads** — Images / PDFs over 256 KiB live in a content-addressed encrypted blob store; small text stays inline.
- **⌨️ Keyboard-first** — Global hotkeys, number-key quick-paste, ⇧⏎ for plain, ⌥⏎ for format preview.

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
| UI (macOS) | Swift 5.9 + AppKit (NSStatusItem · non-activating NSPanel · NSTableView) |
| Core | Rust + Tokio |
| FFI | cbindgen-generated C header + static library |
| Storage | [redb](https://github.com/cberner/redb) (embedded KV) + content-addressed BlobStore |
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

### In the popup panel

| Key | Action |
|-----|--------|
| `1`–`9` | Quick-paste the n-th history item |
| `↑` `↓` | Navigate items |
| `⏎` | Paste selected item |
| `⇧⏎` | Paste as plain text |
| `⌥⏎` | Open Format Paste preview |
| Type | Live-filter (search) |
| `⎋` | Close panel |

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

- [x] Searchable history popup (NSPanel)
- [x] Preferences (history quota, launch-at-login)
- [x] BlobStore for large images
- [x] Format Paste live preview
- [ ] Custom hotkey editor (currently fixed at build time)
- [ ] Image thumbnail in popup
- [ ] DONE LOG search field
- [ ] Code signing + Notarization
- [ ] **Windows port** (same Rust core + C# WinUI 3 frontend)
- [ ] Local sync between Macs (opt-in, encrypted)
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

*何でもコピー。瞬時に検索。完璧にペースト。やった作業を記録。*

</div>

---

## なぜ ClipNoteX？

多くのクリップボードマネージャーは **クラウド同期 (プライバシーリスク)** か **暗号化なし (セキュリティリスク)** か **ペースト時の整形不可 (生産性損失)** のいずれかを抱えています。

ClipNoteX はその 3 つを全て解決します — 完全オフライン、起動時から軍用級暗号化、しかも **メニューバー常駐の小さなネイティブアプリ** (Electron なし / WebView なし / Dock アイコンなし)。

| | ClipNoteX | Clipy / Pasta | クラウド型 |
|---|---|---|---|
| **保存時暗号化** | ✅ XChaCha20-Poly1305 | ❌ 平文 | ⚠️ サーバ側 |
| **100% ローカル / オフライン** | ✅ | ✅ | ❌ |
| **ペースト時整形** | ✅ JSON · SQL · MD | ❌ | ❌ |
| **パスワードマネージャー対応** | ✅ 自動除外 | ❌ | ⚠️ |
| **作業ログ (DONE LOG)** | ✅ 内蔵 | ❌ | ❌ |
| **macOS ネイティブ (WebView なし)** | ✅ AppKit | ✅ | ❌ Electron |
| **オープンソース** | ✅ MIT | ✅ | ❌ |

---

## 主な機能

- **🔒 暗号化履歴** — 全エントリを XChaCha20-Poly1305 で暗号化、鍵は Argon2id で導出し macOS Keychain に保存
- **⚡ ネイティブ UI** — `NSStatusItem` + 非アクティブ化 `NSPanel`。WebView なしでミリ秒で出現、フォーカスを奪わない
- **✨ フォーマットペースト** — JSON / SQL / Markdown / Plain / HTML / CSS / JS / TS のライブプレビュー
- **🔑 パスワードマネージャー除外** — 1Password / Bitwarden / KeePassXC は自動的に除外
- **📓 DONE LOG** — クリップボード内容を作業ログに。編集・タグ・削除・Markdown エクスポート対応
- **📌 ピン留め** — 重要アイテムをクォータ削除から保護
- **🗄 BlobStore** — 256 KiB 超の画像 / PDF は コンテンツアドレス型暗号化ブロブとして別保管
- **⌨️ キーボード中心** — グローバルホットキー・番号即ペースト・⇧⏎ プレーン・⌥⏎ 整形プレビュー

---

## アーキテクチャ

```
ClipNoteX/
├── crates/                       ← Rust コア (OS 横断で共通)
│   ├── clipnotex-core/           共通型・イベントバス・設定
│   ├── clipnotex-clipboard/      OS クリップボードバックエンド (NSPasteboard / Win32)
│   ├── clipnotex-store/          暗号化 redb ストレージ + BlobStore
│   ├── clipnotex-donelog/        DONE LOG ストア + Markdown エクスポート
│   ├── clipnotex-paste/          ペースト制御 + 整形適用
│   ├── clipnotex-format/         テキストフォーマッタ (JSON / SQL / Markdown 等)
│   ├── clipnotex-hotkey/         グローバルホットキー登録
│   ├── clipnotex-app/            キャプチャループ・クォータ・フィルタ
│   └── clipnotex-ffi/            ★ C ABI ブリッジ (cbindgen → ClipNoteX.h)
└── apps/
    └── macos/                    ← Swift + AppKit シェル (SPM)
        ├── Package.swift
        ├── Info.plist
        ├── build-app.sh           # → ClipNoteX.app をビルド
        └── Sources/
            ├── ClipNoteXCore/     # ClipNoteX.h ラップ用 systemLibrary
            └── ClipNoteX/
                ├── main.swift
                ├── AppDelegate.swift
                ├── StatusBarController.swift     (メニューバー + 右クリックメニュー)
                ├── SearchPanel.swift              (検索付きポップアップ)
                ├── DoneLogWindow.swift            (DONE LOG ウィンドウ)
                ├── FormatPasteWindow.swift        (整形プレビュー)
                ├── PreferencesWindow.swift        (設定)
                └── Settings.swift                 (UserDefaults 永続化)
```

**技術スタック:**

| 層 | 技術 |
|---|---|
| UI (macOS) | Swift 5.9 + AppKit (NSStatusItem · 非アクティブ化 NSPanel · NSTableView) |
| コア | Rust + Tokio |
| FFI | cbindgen で C ヘッダ + staticlib 生成 |
| ストレージ | [redb](https://github.com/cberner/redb) (組込 KV) + コンテンツアドレス型 BlobStore |
| 暗号化 | XChaCha20-Poly1305 · Argon2id · BLAKE3 |
| クリップボード | NSPasteboard via [objc2](https://github.com/madsmtm/objc2) |

将来の **Windows 版** も同じ Rust crates を流用します (C# WinUI 3 から `libclipnotex_ffi.a` を呼ぶ構成を予定)。

> **メモ**: 初期試作は Tauri 2 を採用していましたが、WebView ベースでは
> macOS のクリップボードマネージャー UX (アプリ非アクティブのままポップアップ表示)
> が実現できなかったため、タグ [`v0.1-tauri-legacy`](../../releases) で退避済みです。

---

## クイックスタート

### 必要環境

- **macOS 13+**
- [Rust](https://rustup.rs/) 1.82+
- Xcode Command Line Tools (Swift 5.9+)

### ビルドと実行

```bash
# 1) 一括ビルド (Rust staticlib → Swift → .app バンドル)
cd apps/macos
./build-app.sh

# 2) アプリを起動
open build/ClipNoteX.app

# またはターミナルで実行 (ログが stderr に出るので開発時に便利)
./build/ClipNoteX.app/Contents/MacOS/ClipNoteX
```

メニューバーに 📋 アイコンが現れます。初回 `⌘⇧V` 押下時に macOS が **アクセシビリティ権限** を要求します (ペースト合成のため必須)。

### 開発ワークフロー

```bash
# Rust だけ (コアの高速反復)
cargo build -p clipnotex-ffi
cargo test --workspace

# Swift だけ (Rust .a が最新前提)
cd apps/macos
swift build

# debug ビルドの .app
cd apps/macos
./build-app.sh --debug
```

cbindgen のビルドスクリプトは cargo build のたび `crates/clipnotex-ffi/include/ClipNoteX.h` を再生成し、`build-app.sh` が Swift モジュールへコピーします。

---

## キーボードショートカット

### グローバル (macOS)

| キー | 動作 |
|---|---|
| `⌘⇧V` | メニューバー直下にクリップボード履歴ポップアップを開く |
| `⌘⇧D` | 現在のクリップボードを DONE LOG にキャプチャ |

### ポップアップパネル内

| キー | 動作 |
|---|---|
| `1`〜`9` | n 番目を即ペースト |
| `↑` `↓` | アイテム選択 |
| `⏎` | ペースト |
| `⇧⏎` | プレーンテキストでペースト |
| `⌥⏎` | フォーマットプレビューを開く |
| 文字入力 | ライブフィルタ (検索) |
| `⎋` | パネルを閉じる |

---

## セキュリティ

ClipNoteX はセキュリティを後付けではなく設計上の制約として作られています。

- **XChaCha20-Poly1305 AEAD** — 保存される全クリップに認証付き暗号化
- **Argon2id KDF** — GPU/ASIC 攻撃に耐性のある鍵導出
- **BLAKE3** — 高速で衝突耐性のあるコンテンツハッシュ
- **macOS Keychain 連携** — 暗号鍵を Keychain に保管
- **Concealed Pasteboard** — `org.nspasteboard.ConcealedType` エントリは破棄
- **自己書込ガード** — 自分が書いたペーストは再キャプチャしない
- **ネットワーク I/O ゼロ** — バイナリは一切の外部通信をしない
- **Dock 非表示** (`LSUIElement = true`) — メニューバーで邪魔せず常駐

**デフォルト除外リスト** (絶対にキャプチャしない):

| アプリ | マッチ方式 |
|---|---|
| 1Password | Bundle ID + exe 名 |
| Bitwarden | Bundle ID + exe 名 |
| KeePassXC | Bundle ID + exe 名 |

---

## データフロー

```
クリップボード変更
  └─ MacWatcher (100ms ポーリング)
       └─ ExclusionFilter        ← パスワードマネージャーをブロック
            └─ StoreService      ← 暗号化＋永続化 (大きな payload は BlobStore へ)
                 └─ EventBus
                      ├─ QuotaManager   ← 古いクリップを退避
                      └─ FFI コールバック ← Swift に UI 更新通知
```

---

## ロードマップ

- [x] 検索フィールド付き履歴ポップアップ
- [x] 設定画面 (履歴上限 / Launch at login)
- [x] BlobStore (大きな画像対応)
- [x] フォーマットペースト ライブプレビュー
- [ ] ショートカットの編集 UI (現在はビルド時固定)
- [ ] 画像サムネイル表示
- [ ] DONE LOG 検索
- [ ] Code signing + Notarization
- [ ] **Windows 版** (同じ Rust コア + C# WinUI 3 フロント)
- [ ] Mac 間ローカル同期 (オプトイン・暗号化)
- [ ] カスタムフォーマッタの Plugin API

---

## コントリビュート

Issue / PR 歓迎です！

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

---

## ライセンス

MIT © 2026 suzuki-black — 詳細は [LICENSE](LICENSE)。

サードパーティの crate / パッケージは各々のライセンス (MIT / Apache-2.0) に従います。
