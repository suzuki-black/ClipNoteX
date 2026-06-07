# ClipNoteX — アーキテクチャ設計書（実装版 / as-built）

> 本書は **実装済みコード（v0.4 時点）をリバースエンジニアリングして起こした設計書**です。
> 計画段階の [DESIGN.md](DESIGN.md)（v0.2 draft）/ [IMPLEMENTATION.md](IMPLEMENTATION.md) が
> 「これから何を作るか」を述べるのに対し、本書は「実際に何が動いているか」を、現行ソースの
> 型・関数・定数に基づいて記述します。計画と実装が食い違う箇所は **実装側を正**とします。
>
> 対象コミット基準: `crates/` 配下の Rust 実装と `apps/macos/` の Swift フロントエンド。
> Windows フロントエンド（`apps/windows/`）は未実装のため対象外。

---

## 0. 全体像

ClipNoteX は **Clipy 上位互換のクリップボード履歴マネージャ**です。設計の中核は
「**全ビジネスロジックを OS 非依存の Rust ワークスペースに集約し、UI は薄いネイティブ
シェルに留める**」という分離にあります。

```
┌─────────────────────────────────────────────┐
│  ネイティブ UI (apps/)                         │
│  ├ macOS: Swift + AppKit        ← 実装済み      │
│  └ Windows: C# WinUI 3          ← 未実装(v0.5)  │
└───────────────┬─────────────────────────────┘
                │  C ABI (clipnotex-ffi が公開)
┌───────────────▼─────────────────────────────┐
│  Rust 共通コア (crates/)                       │
│  9 クレート。全業務ロジックがここで完結          │
└─────────────────────────────────────────────┘
```

UI とコアの境界は **`clipnotex-ffi` が公開する 26 個の C 関数**（`cnx_*`）だけです。
複雑な値は JSON 文字列で受け渡し、単純な値は C のスカラ型で渡します。

---

## 1. クレート構成と責務

| クレート | 行数規模 | 責務 | OS 依存 |
|----------|---------|------|---------|
| `clipnotex-core` | 〜320 | 共有型・エラー・イベントバス・設定。**OS 依存ゼロの語彙層** | なし |
| `clipnotex-clipboard` | 〜950 | OS クリップボードの監視 / 書き込み抽象。`macos.rs` / `windows.rs` | あり |
| `clipnotex-store` | 〜1170 | 暗号化 redb KV + BlobStore。鍵管理・AEAD・マイグレーション・退避 | keyring のみ |
| `clipnotex-donelog` | 〜1010 | DONE LOG（作業日誌）ストア + Markdown エクスポート | なし |
| `clipnotex-paste` | 〜460 | ペースト注入コントローラ + IME 状態確認 | あり |
| `clipnotex-format` | 〜390 | JSON/SQL/Markdown/プレーンの整形と言語判定 | なし |
| `clipnotex-hotkey` | 〜160 | グローバルホットキー登録（`global-hotkey` ラッパ） | あり |
| `clipnotex-app` | 〜580 | 上位オーケストレーション。キャプチャループ・除外・クォータ・サムネイル | なし |
| `clipnotex-ffi` | 〜1260 | C ABI 公開層（cbindgen で `ClipNoteX.h` 生成） | libc |

依存方向は一方向です：`core` ← 各機能クレート ← `app` ← `ffi`。`core` は他のどのクレートにも
依存しません（語彙の安定性を保証）。

---

## 2. ドメインモデル（`clipnotex-core::model`）

### 2.1 `ClipItem` — 履歴の 1 エントリ

```rust
pub struct ClipItem {
    pub id: ClipId,              // ULID（時刻順ソート可能な一意 ID）
    pub created_at: i64,         // 生成時刻（ms）
    pub updated_at: i64,         // 最終 touch 時刻（再コピーで bump）
    pub source_app: SourceApp,   // コピー元アプリ情報
    pub primary_kind: ClipKind,  // 主要種別
    pub payloads: Vec<PayloadRef>,
    pub digest: [u8; 32],        // blake3。重複排除キー
    pub text_preview: Option<String>,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub total_bytes: u64,
}
```

- **ID は ULID**（`ClipId(Ulid)`）。生成時刻が ID に埋め込まれ、辞書順 = 時刻順になる。
- **重複排除は blake3 ダイジェスト**で行う。同一内容を再コピーすると新規挿入せず
  既存アイテムを `bump_to_top`（`updated_at` 更新で先頭へ）。これが Clipy 互換の挙動。

### 2.2 `ClipKind`

`Text / Image / Rtf / Html / Pdf / Files / Mixed / Custom` の 8 種。複数フォーマットを
含むクリップボードは `Mixed` として保持し、ペースト時に全フォーマットを復元できる。

### 2.3 `PayloadRef` と `PayloadStorage` — 二段ストレージ

各ペイロードは `format_id`（例 `public.utf8-plain-text` / `CF_UNICODETEXT` / `public.png`）と
保存先を持つ。保存先は **サイズで自動振り分け**される：

```rust
pub enum PayloadStorage {
    Inline(Vec<u8>),                       // 小さい暗号文をレコードに同梱
    Blob(BlobId),                          // 大きいものはコンテンツアドレス blob ファイル
    Pack { pack_id, offset, len },         // v0.2+ 予約（現状の writer は生成しない）
}
```

- **閾値は 256 KiB**（`store::BLOB_OFFLOAD_THRESHOLD`）。これを超えるペイロードは
  自動的に `BlobStore` にオフロードされ、レコードには `BlobId`（blake3）だけが残る。
- **キャプチャ上限は 50 MiB/payload**（`capture::MAX_PAYLOAD_BYTES`）。これを超える巨大
  ペイロードはメモリ・ディスク負荷が大きすぎるため破棄する。
- 圧縮は `Compression::{None, Zstd}`。テキスト系は zstd 圧縮の対象。

---

## 3. ストレージ層（`clipnotex-store`）

### 3.1 redb テーブル定義（`tables.rs`）

履歴 DB は単一の `redb` ファイル。4 テーブルで構成：

| テーブル | キー → 値 | 用途 |
|----------|-----------|------|
| `ITEMS` | ULID bytes → AEAD 暗号文 | `ClipItem` 本体（暗号化済み） |
| `BY_TIME` | (created_at_ms, ulid) → () | 時刻順インデックス |
| `BY_DIGEST` | digest → ULID | 重複排除の逆引き |
| `META` | "version" → u64 | スキーマバージョン（マイグレーション用） |

DONE LOG は **別ファイル `donelog.redb`** に隔離し、履歴 DB と混在させない。

### 3.2 暗号化（`aead.rs`）

- **AEAD は XChaCha20-Poly1305**（24 byte nonce, 32 byte key）。`ITEMS` の値も DONE LOG も
  すべて暗号文として保存される。
- **鍵は履歴用・DONE LOG 用で分離**（`DataKeys { history, donelog }`）。`Zeroizing` で
  ドロップ時にメモリをゼロ消去。
- **鍵の保管先は `KeySource`** で切替：
  - `Keyring { service, account }` — OS キーストア（macOS Keychain / Windows Credential
    Manager）。初回は鍵を生成して保存、以降は読み出す。
  - `Ephemeral` — メモリ上のランダム鍵。**テスト専用**（`cnx_init(_, ephemeral_keys=1)`）。

### 3.3 退避ポリシー（`EvictionPolicy`）

クォータ超過時の削除戦略を 2 種で表現：

- `UntilCount(target)` — アイテム数が target 以下になるまで古い順に削除
- `UntilBytes(target)` — 合計バイト数が target 以下になるまで削除

ピン留めアイテムは退避対象外（履歴の上限に関係なく保持）。

---

## 4. キャプチャパイプライン（`clipnotex-app::capture`）

`run_capture_loop` が常駐し、`ClipboardWatcher::next()` でクリップボード変化を待つ：

```
watcher.next()                       ← OS クリップボード変化を検出
   ↓
preview 生成（テキストは先頭30字 / バイナリは "[N bytes binary]"）
   ↓
ExclusionFilter::should_block()      ← 除外ルール判定。該当なら SkipReason::Excluded
   ↓
build_clip_item()                    ← ClipItem 構築（digest 計算等）
   ↓
StoreService に挿入（重複なら bump_to_top）
   ↓
EventBus::emit(CoreEvent::...)       ← UI へ再ロード通知
```

- **自己書き込みの除外**：ペースト時に ClipNoteX 自身がクリップボードへ書く内容を
  再キャプチャしないよう、`SelfWriteGuard`（`clipnotex-clipboard::guard`）でフィルタする。
  FFI 層の `AppState` がこの guard を保持し続ける（ドロップ厳禁）。

### 4.1 除外ルール（`exclusion.rs`）

3 段階マッチで「特定アプリからのコピーを履歴に残さない」を実現：

- `bundle_id`（macOS のバンドル ID 完全一致）
- `exe_basename`（実行ファイル名、`fuzzy` 部分一致オプション付き）
- `window_title`（ワイルドカード `*1Password*` など）

パスワードマネージャ等を除外する用途。`cnx_get/set_exclusions_json` で UI から編集。

---

## 5. ペースト注入（`clipnotex-paste`）

現行実装は **Stage A のみ**（DESIGN §5.3 のうち最も堅牢な経路）：

```
1. OS クリップボードの現状をスナップショット
2. 選択アイテムを書き込む
3. Cmd/Ctrl+V を合成（enigo）
4. （v0.4 修正）元クリップボードは復元しない
```

> **v0.4 の重要修正**（コミット `c6f1bf3`）：以前はステップ 4 で元クリップボードを復元して
> いたが、画像ペーストが復元レースで壊れる不具合があったため**復元処理を撤廃**した。
> ペースト後はそのアイテムがクリップボードに残る（Clipy と同じ挙動）。

`PasteMode` は `Normal(0) / Plain(1) / Format(2) / Full(3)` の 4 種で、FFI の
`cnx_paste_item(id, mode)` の第2引数に対応。

- **Stage B**（AX/UIA 直接注入 + Unicode キーストロークフォールバック）は v0.4 でコードは
  存在するが未配線（`#[allow(dead_code)]`）。3 段フォールバックの設計のみ確定。
- **IME 考慮**（`ime.rs`）：クリップボード経由（Ctrl+V）のため理論上 IME 非干渉。Stage B の
  Unicode キーストローク経路のみ Windows IME ON 時はスキップ。

---

## 6. DONE LOG（`clipnotex-donelog`）

作業日誌機能。**追記専用（immutable + overlay）モデル**：

- **`DoneEntry`** — 不変・暗号化済み。`DoneLogStore::capture()` で生成。
- **`DoneOverlay`** — ユーザー注釈（note / tags / body 上書き）。別途暗号化保存。
  `EditOp` の履歴を保持する。
- **`DoneView`** — `list_done()` / `get_done()` が返す読み取り専用の合成ビュー。

本体を不変にし、編集はオーバーレイで重ねることで、監査可能性と編集自由度を両立する。
`cnx_export_done_markdown(date)` で日次の Markdown エクスポート。

---

## 7. 整形（`clipnotex-format`）

`Formatter` トレイトを実装した 4 種を `FormatService` に登録：

| フォーマッタ | 処理 |
|--------------|------|
| `JsonFormatter` | `serde_json` による pretty-print |
| `SqlFormatter` | `sqlformat` クレートによる整形 |
| `MarkdownFormatter` | 行末スペース正規化のみ |
| `PlainTextFormatter` | タブ展開 / 行末トリム |

UI は `detect()` で `Language` を推定 → `FormatService::format_as()` を呼ぶ。FFI 経由は
`cnx_format_preview_json(text, lang, indent)`（`lang` null で自動判定、結果に
`detected_lang` を含む）。Format Paste のライブプレビューに使う。

---

## 8. ホットキー（`clipnotex-hotkey`）

`global-hotkey` クレートのラッパ。`HotkeyId` 列挙（`ShowHistory / ShowSnippets /
PastePlain / PasteFormat / PasteFull / DoneCapture`）と OS ショートカットを対応付ける。

- 登録は `Result` ではなく **`RegistrationResult`** を返し、競合による失敗を UI に伝える
  （「このショートカットは使用できません」表示のため）。
- 全ショートカットはユーザーが Settings から変更可能。
- メインスレッドのタイマーから `cnx_hotkey_pump()` を定期呼び出しして入力を処理
  （macOS では 50ms タイマー）。

---

## 9. FFI 境界（`clipnotex-ffi`）

### 9.1 メモリ・スレッド規約

```text
Rust → C : Box::leak で確保し、呼び出し側が cnx_free_string で解放
C → Rust : 呼び出し側が所有権を保持。Rust は入口でコピーする
文字列    : 全て UTF-8。Swift 側は MarshalAs(LPUTF8Str) 相当
エラー    : 非ゼロのステータスコードで返し、詳細は cnx_last_error（スレッドローカル）
非同期    : Tokio ランタイムを cnx_init で一度だけ構築し全 async work で再利用
```

### 9.2 公開関数一覧（26 関数）

| 分類 | 関数 |
|------|------|
| ライフサイクル | `cnx_init` / `cnx_shutdown` / `cnx_last_error` / `cnx_free_string` |
| 履歴 | `cnx_list_history_json` / `cnx_paste_item` / `cnx_pin_toggle` / `cnx_delete_item` |
| DONE LOG | `cnx_capture_done` / `cnx_list_done_json` / `cnx_delete_done` / `cnx_update_done_overlay_json` / `cnx_export_done_markdown` |
| 設定・除外 | `cnx_get/set_exclusions_json` / `cnx_set/get_history_quota` / `cnx_reset_data` |
| 整形 | `cnx_format_preview_json` |
| ホットキー | `cnx_register_hotkey` / `cnx_clear_hotkeys` / `cnx_hotkey_pump` / `cnx_set_hotkey_callback` |
| キャプチャ | `cnx_start_capture_loop` / `cnx_set_capture_callback` |

コールバックは 2 種：ホットキー押下通知（`CnxCnxHotkeyCallback`）と新規キャプチャ通知
（`CnxCnxCaptureCallback`）。Swift 側は受け取って UI を再ロードする。

---

## 10. macOS フロントエンド（`apps/macos`）

Swift + AppKit。FFI を `ClipNoteXCore` モジュール（`ClipNoteX.h` + modulemap）経由で呼ぶ。

| Swift ファイル | 役割 |
|----------------|------|
| `main.swift` / `AppDelegate.swift` | NSApp 起動、`cnx_init` + キャプチャループ開始 |
| `StatusBarController.swift` | メニューバーアイコン + 右クリックメニュー |
| `SearchPanel.swift` | 検索付き非アクティブ化ポップアップ（NSPanel） |
| `DoneLogWindow.swift` | DONE LOG フル UI |
| `PreferencesWindow.swift` | 設定（履歴クォータ / 除外 / ショートカット / About） |
| `FormatPasteWindow.swift` | Format Paste ライブプレビュー |
| `ShortcutRecorder.swift` | ホットキー record UI |
| `ExclusionTableController.swift` | 除外アプリ一覧編集 |
| `SyntaxHighlight.swift` | JSON/SQL/JS 等の簡易ハイライト |
| `ThirdPartyLicenses.swift` | サードパーティライセンス表示（手動メンテ） |
| `Settings.swift` | UserDefaults 永続化 |

UX 仕様（同一ホットキーでトグル表示、数字キー即ペースト、↑↓ で選択移動など）は
README §「使い方」と SearchPanel に確定実装あり。

---

## 11. セキュリティ・プライバシー設計

ClipNoteX の差別化は「**ローカル完結 + 暗号化**」にある：

1. **全データ暗号化** — 履歴も DONE LOG も XChaCha20-Poly1305 で保存。鍵は OS キーストア。
2. **ネットワーク非通信** — HTTP クライアント不使用。完全オフライン。
3. **除外ルール** — パスワードマネージャ等からのコピーを履歴に残さない。
4. **画像デコーダ堅牢化**（v0.4）— クリップボード画像は信頼できない入力。サムネイル生成
   （`thumbnail.rs`）はフォーマット自動判定をやめ **PNG 固定デコード**にし、TIFF/JPEG など
   意図しないデコーダ経路への露出を排除した。
5. **公開リポジトリ衛生** — 個人情報・絶対パス・実メールをソース/コミットに含めない。
   コミット著者は `ClipNoteX <noreply@example.com>` に統一。

---

## 12. テスト

ワークスペース全体で 46 件の単体テスト（v0.4 時点）。重点領域：

- `clipnotex-store` — AEAD ラウンドトリップ、退避ポリシー、重複排除
- `clipnotex-format` — 各フォーマッタの整形結果と言語判定
- `clipnotex-donelog` — capture / overlay / Markdown export
- `clipnotex-paste` — PasteMode ごとのペイロード抽出
- `clipnotex-app` — 除外フィルタのマッチ、サムネイル生成
- `clipnotex-clipboard` — 自己書き込みガード、HGLOBAL ヘルパー（Windows）

---

## 13. 未実装・将来作業（コード内マーカーより）

| 箇所 | 内容 | 予定 |
|------|------|------|
| `clipnotex-paste` Stage B | AX/UIA 直接注入 + 3 段フォールバック | v0.4+ |
| `clipnotex-app/thumbnail.rs` | `CoreEvent::ThumbnailReady` のバス通知 | M8 |
| `clipnotex-clipboard/windows.rs` | Win32 クリップボード実装の完成 | v0.5（Windows 版） |
| `PayloadStorage::Pack` | 月次 blob pack によるファイル数削減 | v0.2+ 予約 |
| `apps/windows/` | C# WinUI 3 フロントエンド | v0.5 |

詳細な Windows 移植手順は別途ハンドオフ文書（git 管理外）にまとめてある。
</content>
</invoke>
