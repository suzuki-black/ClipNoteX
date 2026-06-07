<div align="center">

# 📋 ClipNoteX

**The native clipboard manager that doesn't phone home — and can't be made to.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.82+-orange?logo=rust)](https://www.rust-lang.org)
[![Swift](https://img.shields.io/badge/Swift-5.9+-orange?logo=swift)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2013%2B-lightgrey?logo=apple)](#)
[![Version](https://img.shields.io/badge/version-0.4.0-brightgreen)](#)

<br/>

*Copy anything. Find it instantly. Paste it perfectly. Log what you did — all offline, all encrypted.*

</div>

---

## Why ClipNoteX?

Most clipboard managers either **sync to the cloud** (privacy risk), **skip encryption** (security risk), or are **Electron-based bloatware** (battery drain).

ClipNoteX takes the opposite approach:

|                              | ClipNoteX               | Clipy / Maccy | Cloud-based managers   |
|------------------------------|-------------------------|---------------|------------------------|
| **Native UI**                | ✅ AppKit / NSPanel      | ✅             | ❌ Electron / WebView   |
| **Encrypted at rest**        | ✅ XChaCha20-Poly1305    | ❌ Plaintext   | ⚠️ Server-side          |
| **100% local / offline**     | ✅                       | ✅             | ❌                       |
| **Image / RTF / HTML paste** | ✅                       | ✅             | ⚠️                       |
| **Format on paste**          | ✅ JSON · SQL · MD ...   | ❌             | ❌                       |
| **Password-manager safe**    | ✅ Auto-exclude          | ⚠️             | ⚠️                       |
| **Work log (DONE LOG)**      | ✅ Built-in              | ❌             | ❌                       |
| **Open source**              | ✅ MIT                   | ✅             | ❌                       |

---

## Features

- **🔒 Encrypted history** — Every entry is sealed with XChaCha20-Poly1305 AEAD. Keys live in the macOS Keychain (Keyring v1 service `ClipNoteX`).
- **⚡ Native menubar UI** — `NSStatusItem` + `NSPanel` (non-activating). Pops up at the cursor in milliseconds; never steals focus from your text editor. Compact 360 × 400 single-line rows (Clipy-grade density) — search is always faster than browsing.
- **🧠 Clipy-compatible reorder** — Pasted items move to the top of the history. Same content re-copied bumps the existing entry. No silent dedup losses.
- **🖼 Full payload support** — Text, RTF, HTML, images, PDFs and file lists are all captured and round-tripped. Large payloads (>256 KiB) are content-addressed and offloaded to encrypted `blobs/` to keep the DB compact.
- **✨ Format Paste** — Copy messy JSON, paste perfect JSON. Live preview before commit. Supports JSON · SQL · Markdown · HTML · CSS · JavaScript · TypeScript · plain text.
- **🔑 Password-manager safe** — 1Password, Bitwarden, KeePassXC windows are excluded by default and at the bundle-id level. Always.
- **📓 DONE LOG** — Promote any clipboard item (or quick-capture freeform text) into a daily work log. Tag, edit, export as Markdown.
- **📌 Pin & protect** — Pinned clips survive quota eviction.
- **🛠 Reset switch** — One-button data wipe for recovering from broken encryption state.
- **⌨️ Keyboard-first** — `⌘⇧V` toggles the popup (no mouse needed to close), paste with `⏎` or `1…9`, never touch the trackpad. All global shortcuts customizable from Preferences.
- **🎨 Syntax-highlighted Format Paste** — JSON / SQL / JS / TS / HTML / CSS / Markdown preview is colorized before you commit.
- **🟡 Search highlight** — Live filter matches are marked in both the clipboard popup and the DONE LOG list.

---

## Demo

```
┌──────────────────────────────────────────────────┐
│  🔍 Search clipboard…              📓  ⚙        │
├──────────────────────────────────────────────────┤
│ 1 📝 Hello from ClipNoteX           Safari · 12s │
│ 2 🖼 (Image)                       Preview · 1m │
│ 3 📌 SELECT * FROM users WHERE …  Terminal · 3h │
│ 4 📝 Fixed the race condition…       Slack · 12h │
└──────────────────────────────────────────────────┘
  ↑↓ · ⏎ paste · ⇧⏎ plain · ⌥⏎ fmt · ⌘P pin · ⌘⌫ del · 1–9 · ⌘L log · ⌘, prefs · ⎋
```

**Format Paste:**

```
Before:  {"name":"test","value":42,"list":[1,2,3]}

After (⌥⏎ → live preview → ⌘⏎):
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

## Architecture

```
ClipNoteX/
├── crates/                          ← shared Rust core
│   ├── clipnotex-core/              # types, event bus, settings
│   ├── clipnotex-clipboard/         # NSPasteboard / Win32 backends
│   ├── clipnotex-store/             # encrypted redb + blob store
│   ├── clipnotex-donelog/           # DONE LOG store + Markdown export
│   ├── clipnotex-paste/             # paste controller + format apply
│   ├── clipnotex-format/            # JSON / SQL / MD / HTML formatters
│   ├── clipnotex-hotkey/            # global-hotkey abstraction
│   ├── clipnotex-app/               # capture loop + quota + exclusion
│   └── clipnotex-ffi/               # C ABI surface (cbindgen → ClipNoteX.h)
├── apps/
│   ├── macos/                       # Swift + AppKit shell
│   │   ├── Sources/ClipNoteX/       # AppDelegate / StatusBar / SearchPanel ...
│   │   ├── Info.plist               # LSUIElement = true (no Dock icon)
│   │   ├── icon.icns                # app icon
│   │   ├── build-app.sh             # cargo build → SwiftPM → .app
│   │   └── sign-app.sh              # codesign + notarize helper
│   └── windows/                     # WinUI 3 scaffolding (v0.5)
└── tools/devcli                     # CLI for store/paste smoke tests
```

### Why this split?

The Rust crates are 100% platform-agnostic (or feature-flagged per OS). The Swift app is a **thin AppKit shell** that talks to the Rust core through a stable C ABI. The same staticlib will drive a Windows WinUI 3 frontend later — no Electron, no WebView, no JS toolchain.

### Tech stack

| Layer        | Technology                                       |
|--------------|--------------------------------------------------|
| Core         | Rust 1.82 · Tokio · redb                         |
| Crypto       | XChaCha20-Poly1305 · Argon2id · BLAKE3           |
| FFI          | `staticlib` + `cbindgen` → `ClipNoteX.h`         |
| macOS shell  | Swift 5.9 · AppKit · `NSStatusItem` · `NSPanel`  |
| Hotkey       | `global-hotkey` (Carbon under the hood)          |
| Keystroke    | `enigo` with raw kVK codes (no TSM lookups)      |

---

## Quick Start

### Requirements

- macOS 13 (Ventura) or later
- [Rust](https://rustup.rs/) 1.82+
- Xcode 15 (command-line tools only is enough)

### Build & run

```bash
git clone https://github.com/suzuki-black/ClipNoteX.git
cd ClipNoteX/apps/macos
./build-app.sh           # cargo → swift → assemble + ad-hoc sign + strip quarantine
open build/ClipNoteX.app
```

The very first `⌘⇧V` press triggers a macOS **Accessibility permission** prompt — required to synthesize the paste keystroke. Grant it once.

For logs while debugging:

```bash
./build/ClipNoteX.app/Contents/MacOS/ClipNoteX                       # foreground
RUST_LOG=clipnotex=debug ./build/ClipNoteX.app/Contents/MacOS/ClipNoteX
```

### Dev keys (no Keychain prompt)

```bash
CLIPNOTEX_EPHEMERAL=1 ./build/ClipNoteX.app/Contents/MacOS/ClipNoteX
```

Generates fresh in-memory keys each run — useful while iterating, but every restart loses old history. Use **Preferences → Maintenance → Reset all data** to wipe stale entries cleanly when switching modes.

---

## Keyboard Shortcuts

### Global

| Key       | Action                                                            |
|-----------|-------------------------------------------------------------------|
| `⌘⇧V`     | **Toggle** the clipboard history popup (press again to dismiss)   |
| `⌘⇧D`     | Capture current clipboard to DONE LOG                             |

Both shortcuts are user-customizable via **Preferences → Shortcuts**.

### Inside the popup

| Key           | Action                                                |
|---------------|-------------------------------------------------------|
| `↑` `↓`       | Navigate items                                        |
| `⏎`           | Paste with all original formats                       |
| `⇧⏎`         | Paste as plain text                                   |
| `⌥⏎`         | Open Format Paste live preview                        |
| `⌘P`          | Pin / unpin the selected item                         |
| `⌘⌫`          | Delete the selected item                              |
| `1` … `9`     | Quick-paste the Nth item                              |
| `⌘L`          | Open the DONE LOG window                              |
| `⌘,`          | Open Preferences                                      |
| `⎋`           | Close the popup                                       |
| any character | Live-filter the history                               |

The popup also exposes 📓 / ⚙ buttons next to the search field — useful for users whose menubar right-click is taken over by another app (notch utilities, etc.).

---

## Security

ClipNoteX was designed with security as a constraint, not an afterthought.

- **XChaCha20-Poly1305 AEAD** — authenticated encryption for every stored clip; `created_at` (big-endian) is used as AAD to prevent ciphertext replay between entries.
- **Argon2id KDF** — key derivation resistant to GPU/ASIC attacks.
- **BLAKE3** — fast, collision-resistant content hashing for dedup and blob addressing.
- **macOS Keychain integration** — data key lives in Keychain (service `ClipNoteX`, account `data_key`). Ephemeral in-memory keys available for development via `CLIPNOTEX_EPHEMERAL=1`.
- **Concealed Pasteboard handling** — macOS `org.nspasteboard.ConcealedType` / `TransientType` / `AutoGenerated` entries are discarded before they ever reach the store.
- **Self-write guard** — a digest LRU prevents the app from re-capturing its own paste operations.
- **Zero network I/O** — the binary makes no outbound connections. Verify with `lsof -i -p <pid>` or Little Snitch.

### Default exclusion list

These applications are **never** captured:

| App         | Match type           |
|-------------|----------------------|
| 1Password   | Bundle ID + exe name |
| Bitwarden   | Bundle ID + exe name |
| KeePassXC   | Bundle ID + exe name |

---

## Data flow

```
NSPasteboard change
  └─ MacWatcher (100 ms changeCount poll)
       └─ ConcealedType / SelfWriteGuard checks
            └─ ExclusionFilter  (blocks password managers)
                 └─ StoreService.add_item
                      ├─ small payload → Inline (encrypted in row)
                      ├─ large payload → BlobStore (content-addressed file)
                      └─ duplicate digest → touch → bump_to_top
                           └─ EventBus → QuotaManager evicts oldest non-pinned
```

---

## Roadmap

- [x] **v0.1** — initial Tauri prototype (now retired; tagged `v0.1-tauri-legacy`)
- [x] **v0.2** — Swift + AppKit native shell, image paste, Clipy-compatible reorder, Reset feature
- [x] **v0.3** — customizable hotkeys, exclusion-rule GUI, syntax-highlighted Format Paste, search highlight, third-party license dialog, hotkey toggle
- [x] **v0.4** — reverse-engineered Japanese architecture doc, expanded unit-test suite, source audit (dead-code/license/vuln/PII) & image-decoder hardening
- [ ] **v0.5** — Windows version (WinUI 3 frontend on the same Rust core)
- [ ] **v0.6** — opt-in iCloud-encrypted sync between Macs
- [ ] **v0.7** — plugin API for custom formatters
- [ ] **v1.0** — Developer ID signing, notarization, Sparkle auto-update

---

## Contributing

PRs and issues are welcome.

```bash
# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets

# Format
cargo fmt --all
```

Please open an issue before starting work on anything larger than a typo fix, so we can talk through scope.

---

## License

MIT © 2026 suzuki-black — see [LICENSE](LICENSE).

Third-party crates are used under their respective licenses (MIT / Apache-2.0 / BSD-2-Clause).

---

---

<div align="center">

# 📋 ClipNoteX（日本語）

**クラウドに送らない。暗号化しないなんてあり得ない。Electron 不要のネイティブ実装。**

</div>

---

## なぜ ClipNoteX？

世のクリップボードマネージャーは、だいたい以下のどれかに当てはまります：

- **クラウド同期** でプライバシーが危ない
- **暗号化なし** でセキュリティが危ない
- **Electron 製** で常駐バッテリーを食う

ClipNoteX はそのすべての逆を行きます — **完全オフライン・常時暗号化・AppKit ネイティブ**。

|                                | ClipNoteX               | Clipy / Maccy | クラウド系             |
|--------------------------------|-------------------------|---------------|------------------------|
| **ネイティブ UI**              | ✅ AppKit / NSPanel      | ✅             | ❌ Electron / WebView   |
| **暗号化保管**                 | ✅ XChaCha20-Poly1305    | ❌ 平文        | ⚠️ サーバ側             |
| **完全オフライン**             | ✅                       | ✅             | ❌                       |
| **画像 / RTF / HTML 対応**     | ✅                       | ✅             | ⚠️                       |
| **整形ペースト**               | ✅ JSON · SQL · MD ...   | ❌             | ❌                       |
| **パスワードマネージャー除外** | ✅                       | ⚠️             | ⚠️                       |
| **作業ログ (DONE LOG)**        | ✅ 標準搭載              | ❌             | ❌                       |
| **オープンソース**             | ✅ MIT                   | ✅             | ❌                       |

---

## 主な機能

- **🔒 暗号化履歴** — XChaCha20-Poly1305 AEAD で全エントリを暗号化。鍵は macOS Keychain (`ClipNoteX` サービス) に保管
- **⚡ ネイティブメニューバー UI** — `NSStatusItem` + 非アクティブ化 `NSPanel`。カーソル付近に即時ポップアップし、エディタのフォーカスを奪わない。1 行レイアウト 360×400 で Clipy 級の情報密度（検索が速いので、ブラウズより常に有利）
- **🧠 Clipy 互換挙動** — ペーストしたアイテムは履歴の先頭に移動。同じ内容を再コピーすると既存エントリを浮上（黙って消す dedup 損失なし）
- **🖼 全ペイロード対応** — テキスト・RTF・HTML・画像・PDF・ファイルリストすべてキャプチャ＆ペースト可能。256 KiB 超は content-addressed 暗号化 blob として `blobs/` に格納し DB をコンパクトに保つ
- **✨ フォーマットペースト** — 崩れた JSON をコピー → 整形プレビュー → そのままペースト。JSON · SQL · Markdown · HTML · CSS · JS · TS · プレーンに対応
- **🔑 パスワードマネージャー除外** — 1Password・Bitwarden・KeePassXC を bundle-id レベルで常時除外
- **📓 DONE LOG** — クリップボードや手書きメモを作業日誌に昇格。タグ付け・編集・Markdown エクスポート
- **📌 ピン留め** — ピン留めはクォータ削除から保護
- **🛠 リセット機能** — 「壊れた暗号化状態」(decrypt failed 大量警告) からの復帰用ワンボタン全削除
- **⌨️ キーボード完結** — `⌘⇧V` でトグル表示（再度押すと閉じる）→ `⏎` / `1〜9` でペースト。トラックパッド不要。全ホットキーは Preferences で変更可
- **🎨 シンタックスハイライト付きフォーマットペースト** — JSON / SQL / JS / TS / HTML / CSS / Markdown のプレビューに色付け
- **🟡 検索ハイライト** — クリップボードポップアップ・DONE LOG どちらも一致箇所を黄色マーカー

---

## クイックスタート

```bash
git clone https://github.com/suzuki-black/ClipNoteX.git
cd ClipNoteX/apps/macos
./build-app.sh
open build/ClipNoteX.app
```

初回 `⌘⇧V` 押下時に macOS が**アクセシビリティ権限**を要求します（ペーストキー合成のため必要）。許可してください。

開発時、Keychain プロンプトが煩わしい場合：

```bash
CLIPNOTEX_EPHEMERAL=1 ./build/ClipNoteX.app/Contents/MacOS/ClipNoteX
```

毎起動で鍵が変わるため過去データは読めなくなります。**Preferences → Maintenance → Reset all data…** で安全にクリーンアップ可能。

---

## キーボードショートカット

### グローバル

| キー  | 動作                                                                |
|-------|---------------------------------------------------------------------|
| `⌘⇧V` | 履歴ポップアップを**トグル**（もう一度押すと閉じる、Clipy 風）       |
| `⌘⇧D` | 現在のクリップボードを DONE LOG にキャプチャ                         |

両方とも **Preferences → Shortcuts** で自由に変更できます。

### ポップアップ内

| キー       | 動作                                         |
|------------|----------------------------------------------|
| `↑` `↓`    | アイテム選択                                  |
| `⏎`        | ペースト（元のフォーマット保持）              |
| `⇧⏎`      | プレーンテキストでペースト                    |
| `⌥⏎`      | フォーマットペースト（ライブプレビュー）       |
| `⌘P`       | 選択アイテムをピン留め / 解除                 |
| `⌘⌫`       | 選択アイテムを削除                            |
| `1`〜`9`   | n 番目を即ペースト                            |
| `⌘L`       | DONE LOG ウィンドウを開く                     |
| `⌘,`       | Preferences を開く                            |
| `⎋`        | 閉じる                                        |
| 文字       | 履歴をライブフィルタ                          |

検索欄の横に 📓 / ⚙ ボタンも配置しています（ノッチ系常駐アプリでアイコン右クリックが奪われているユーザ向け）。

---

## セキュリティ設計

- **XChaCha20-Poly1305 AEAD** — 全データを認証付き暗号化。AAD として `created_at` (big-endian) を使い、エントリ間の ciphertext リプレイを防止
- **Argon2id KDF** — GPU/ASIC 耐性の鍵導出
- **BLAKE3** — 重複検知 / blob アドレッシング用の高速ハッシュ
- **macOS Keychain** にデータ鍵を保管（service `ClipNoteX` / account `data_key`）
- **ネットワーク通信なし** — 外部通信は一切しない（Little Snitch 等で検証可）
- **デフォルト除外** — 1Password・Bitwarden・KeePassXC は常に除外
- **NSPasteboard ConcealedType / TransientType** はストアに渡る前に破棄

データが大量の `decrypt failed` 警告で読めなくなった場合は、**Preferences → Maintenance → Reset all data…** で復帰してください。

---

## アーキテクチャ

- **Rust コア** (`crates/`) — 暗号化・ストレージ・キャプチャ・ペースト・フォーマット
- **C ABI 層** (`crates/clipnotex-ffi`) — cbindgen が `ClipNoteX.h` を自動生成
- **macOS フロント** (`apps/macos`) — Swift + AppKit。Clipy / Maccy と同じ `NSStatusItem` パターン
- **Windows フロント** (`apps/windows`) — v0.5 で同じ Rust staticlib を WinUI 3 から利用予定

---

## ロードマップ

- [x] **v0.1** — Tauri 試作（廃止、タグ `v0.1-tauri-legacy` で退避済み）
- [x] **v0.2** — Swift + AppKit ネイティブ実装、画像ペースト、Clipy 互換並び替え、リセット機能
- [x] **v0.3** — ショートカットカスタマイズ、除外ルール GUI、フォーマットペーストのシンタックスハイライト、検索ハイライト、ライセンス情報ダイアログ、ホットキートグル
- [x] **v0.4** — リバースエンジニアリングによる日本語アーキテクチャ設計書、単体テスト拡充、ソース監査（不要コード／ライセンス／脆弱性／個人情報）と画像デコーダの堅牢化
- [ ] **v0.5** — Windows 版（同じ Rust コア＋ WinUI 3）
- [ ] **v0.6** — opt-in iCloud 暗号化同期
- [ ] **v0.7** — 独自フォーマッタ用プラグイン API
- [ ] **v1.0** — Developer ID 署名、Notarization、Sparkle 自動更新

---

## 開発

```bash
cargo test --workspace                # コア全テスト
cargo clippy --workspace --all-targets
cd apps/macos && ./build-app.sh       # .app 生成 (ad-hoc sign + quarantine 除去込み)
```

---

## ライセンス

MIT © 2026 suzuki-black

サードパーティクレートは、それぞれの MIT / Apache-2.0 / BSD-2-Clause ライセンスに従います。
