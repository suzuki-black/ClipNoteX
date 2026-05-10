# ClipNoteX — 設計ドキュメント (v0.2 draft)

> Clipy 上位互換の Win/Mac クロスプラットフォームクリップボードマネージャ。
> 「完全復元」「整形ペースト」「DONE LOG」を備えたプロ向け OSS。
>
> v0.1 → v0.2 改訂内容:
> - 冒頭に **プロダクトの価値 (§0)** を新設
> - 完全復元 Stage B に **3 段階フォールバック** を明記（§5.3）
> - UI の **IME 対応方針** を追記（§7.2）
> - blob pack の **compaction トリガー条件** を明確化（§3.5）
> - DONE LOG 画像の **将来の形式最適化方針** を追記（§3.2）
> - **実装フェーズ注意点** を §10 に新設（NSPasteboardItem 遅延読込 / CF_HDROP / redb トランザクション規律）
>
> v0.0 → v0.1 改訂内容:
> - OS 仕様の地雷回避策を §4 と新設の §5 にまとめ直し
> - 完全復元の "OSクリップボード非汚染" 戦略を再設計（§5.3）
> - 整形ペースト MVP を JSON/SQL/Markdown に縮小（§4.4, §6）
> - DONE LOG を **追記専用 (immutable + overlay)** に変更（§4.5）
> - blob ファイル数爆発対策として **monthly blob pack** を追加（§3.5）
> - 履歴 UI 仕様を具体化（§7）
> - 除外アプリを **3 段階マッチ** に変更（§4.2）
> - MVP スコープを Copilot レビュー指摘に従って再整理（§6）
> - README / Issue テンプレ案を §9 に追加

---

## 0. プロダクトの価値（Why ClipNoteX）

ClipNoteX が OSS として埋めるニッチは明確で、競合不在の領域に直接刺さる。

| 価値 | 既存の状況 | ClipNoteX のポジション |
|---|---|---|
| **Clipy の後継** | Clipy は mac 専用 / 開発が緩慢 / プロ向け機能なし | UI とショートカットを継承、Win 対応 + 暗号化 + 完全復元 |
| **完全復元のクロスプラットフォーム** | mac は Paste / Maccy 等が部分対応、Win 側は Ditto を含め "完全復元" の概念自体が薄い | **Win/Mac 両対応で「全 NSPasteboard type / 全 CF_*」を保持・復元する唯一の OSS** |
| **DONE LOG (作業ログ)** | TODO アプリは飽和 / 「やった事ログ」は手作業前提 | コピー操作にショートカットを 1 つ足すだけで日報・研究ノートが自然蓄積 |
| **暗号化ローカル完結** | クラウド同期型が増え、社内 PC で禁止になることが多い | ローカル AEAD + OS Keychain で企業利用にも耐える |
| **コントリビュートしやすさ** | 多くの常駐アプリは UI と Core が密結合 | crates 物理分割 + フォーマッタ追加用 Issue テンプレで OSS 寄稿の入口を広げる |

**ターゲットユーザ像**:
- mac/win を行き来するソフトウェアエンジニア・SRE・データエンジニア
- 日報・週報・進捗報告を書く必要がある全員
- パスワードマネージャ利用者（除外機能の安心感が刺さる）
- 旧 Clipy 利用者（明示的に "Clipy 互換" を打ち出す）

**スターを取りに行く戦略上の根拠**:
1. Clipy の置き換え需要が顕在（GitHub Issue / Reddit に "Clipy alternative" の声が継続）
2. Windows 側に "完全復元" 概念のある OSS がほぼ無い（Ditto は強力だが UI 文化が異なる）
3. DONE LOG は他に類例が少なく、README の GIF 1 枚で価値が伝わりやすい
4. 暗号化 + パスワードマネージャ除外は、企業ユーザの導入ハードルを下げる重要な差別化要素

---

## 1. 全体アーキテクチャ

### 1.1 プロセス構成

シングルプロセス + 複数スレッドの常駐アプリ。Tauri 2 を採用。

- **Core (Rust)** : OS イベント・クリップボード監視・ストレージ・暗号化・グローバルショートカット・ペースト注入・トレイ常駐。
- **UI (WebView, TS+React)** : 履歴一覧 / スニペット / DONE LOG / 設定。Tauri IPC (invoke / event) で Core と通信。WebView は遅延生成（headless 常駐）。

### 1.2 モジュール分割（crates）

```
crates/
├─ clipnotex-core/        # 型・設定・エラー・イベントバス
├─ clipnotex-clipboard/   # OS抽象 (mac: NSPasteboard, win: OLE clipboard)
│   ├─ src/macos.rs       # NSPasteboard / NSWorkspace
│   ├─ src/windows.rs     # OLE Clipboard / GetClipboardOwner
│   └─ src/safelist.rs    # 安全に取得可能なフォーマット定義
├─ clipnotex-hotkey/      # global-hotkey ラッパ
├─ clipnotex-store/       # redb + AEAD + blob-store + monthly pack
│   ├─ src/items.rs
│   ├─ src/blobs.rs
│   └─ src/pack.rs        # 月次パック化（§3.5）
├─ clipnotex-format/      # 言語検出・フォーマッタ・将来のプラグイン I/F
├─ clipnotex-paste/       # ペースト注入 (キー合成 / AX / UIA)
├─ clipnotex-donelog/     # DONE LOG (immutable + edit overlay)
├─ clipnotex-app/         # オーケストレータ
└─ clipnotex-tauri/       # Tauri commands / events 薄ラッパ
```

責務を crate で物理分割しておくと、ライセンス境界（GPL なライブラリは特定 crate に隔離）、プラットフォーム差分、テスト性が保てる。

### 1.3 データフロー（更新版）

```
[OS Clipboard Change]
  │  mac: NSPasteboard.changeCount poll (50–100ms)
  │  win: AddClipboardFormatListener (WM_CLIPBOARDUPDATE)
  ▼
[ClipboardWatcher]
  │  ├─ self_write_guard:  直近 N 秒以内に自分が書いた digest なら無視
  │  ├─ source_detect:     mac=NSWorkspace.frontmost + org.nspasteboard.source
  │  │                     win=GetClipboardOwner → ProcessId → image name
  │  └─ format_safelist:   §5.2 のホワイトリストのみ取得試行
  ▼
[CaptureService]
  │  ├─ digest = blake3(primary payload)
  │  ├─ dedupe: by_digest インデックスで既存ヒットなら touch のみ
  │  ▼
[ExclusionFilter]
  │   3 段階マッチ (bundle_id / exe_basename / window_title)
  │   + 機微シグナル (org.nspasteboard.ConcealedType, ExcludeClipboardContentFromMonitorProcessing)
  │   pass ▼               block → 破棄
[StoreService] → 値単位 AEAD → redb (items) + blobs/ or monthly pack
  │
  ├─► [HistoryUI]
  ├─► [QuotaManager] (件数 / 容量ポリシー)
  └─► [DoneLogService] (DONEショートカット時のみ別経路)

[GlobalHotkey] ──► [PasteController]
                       ├─ normal:    最後にコピーされた item を再貼付
                       ├─ plain:     text/plain のみ抽出
                       ├─ format:    formatter → text → paste
                       └─ full:      退避→全フォーマット書戻→注入→書戻し
```

---

## 2. 技術スタック

| 候補 | 強み | 弱み | 評価 |
|---|---|---|---|
| **Tauri 2 + Rust** | 軽量・ネイティブ寄り・Rust の安全性・OS FFI 直叩きが楽 | WebView 差異 (WebKit/WebView2) | **採用** |
| Electron | UI 速い・エコシステム大 | 重い・ネイティブ拡張は別途 | 不採用 |
| Rust + egui | 完全ネイティブ・最軽量 | UI 表現力 / IME / アクセシビリティが弱い | 不採用 |
| .NET MAUI / Avalonia | C# 資産 | mac の細かいクリップボード制御が薄い | 不採用 |

採用 crate（initial pick）:

| 用途 | crate |
|---|---|
| 基本クリップボード | `arboard` |
| mac 完全制御 | `objc2`, `objc2-app-kit` |
| win 完全制御 | `windows` (windows-rs), `clipboard-win` |
| グローバル HK | `global-hotkey` |
| トレイ | `tray-icon` |
| KV ストア | **`redb`** (ピュア Rust, ACID, 単一ファイル) |
| 暗号化 | `chacha20poly1305` + `argon2` + `zeroize` |
| OS 鍵管理 | `keyring` |
| 圧縮 | `zstd` |
| ハッシュ | `blake3` |
| 言語検出 | `hyperpolyglot` + 自前ヒューリスティクス |
| JSON 整形 | `serde_json` |
| SQL 整形 | `sqlformat` |
| Markdown 整形 | `pulldown-cmark` round-trip |
| 入力合成 | `enigo` |
| ULID | `ulid` |

---

## 3. データモデル

### 3.1 履歴 (`history.redb`)

```rust
struct ClipItem {
    id: Ulid,
    created_at: i64,
    updated_at: i64,                // dedupe touch 時更新
    source_app: SourceApp,
    primary_kind: ClipKind,
    payloads: Vec<PayloadRef>,
    digest: [u8; 32],               // blake3, 重複検出
    text_preview: Option<String>,   // 先頭 256 文字 (UI 用, 暗号化対象)
    pinned: bool,
    tags: Vec<String>,
    total_bytes: u64,
}

enum ClipKind { Text, Image, Rtf, Html, Pdf, Files, Mixed, Custom }

struct PayloadRef {
    format_id: String,              // "public.utf8-plain-text" / "CF_UNICODETEXT" 等
    compression: Compression,       // None | Zstd
    storage: PayloadStorage,        // Inline(Vec<u8>) | Blob(BlobId) | Pack(PackId, Offset, Len)
    raw_size: u64,
}
```

### 3.2 DONE LOG (`donelog.redb`) — immutable + overlay

レビュー指摘 §2.5 に対応。**元のキャプチャは不変**、編集は overlay として別エントリ。

```rust
struct DoneEntry {
    id: Ulid,
    date: NaiveDate,
    time: NaiveTime,
    source_app: SourceApp,
    kind: ContentKind,
    body: String,                   // 元データ (不変)
    attachment: Option<Attachment>, // 画像時
    captured_at: i64,
}

struct Attachment {
    path: PathBuf,                  // images/<date>/<id>.<ext>
    format: ImageFormat,            // Png | Jpeg | WebP
    width: u32,
    height: u32,
    bytes: u64,
}

enum ImageFormat { Png, Jpeg, WebP }

struct DoneOverlay {
    entry_id: Ulid,                 // DoneEntry を参照
    user_note: Option<String>,      // 追記
    user_body: Option<String>,      // 完全置換 (上書き表示)
    tags: Vec<String>,
    edited_at: i64,
    history: Vec<EditOp>,           // 編集履歴
}

enum EditOp { SetNote(String), SetBody(String), AddTag(String), RemoveTag(String) }
```

UI 取得時は `DoneEntry` に `DoneOverlay` をマージして表示。元データは常に取り出せる（"原本表示" トグル）。

**画像形式の方針**:
- **MVP (v0.3)**: PNG 固定で実装。実装が単純で品質劣化なし。
- **v0.5+ の最適化**: 入力ヒューリスティクスで形式を自動選択。
  - スクリーンショットや UI 要素 (透過 / 鋭いエッジ / 256 色未満の色数) → **PNG**
  - 写真・グラデーション (色数多 / 高エントロピー / アルファなし) → **JPEG** (quality=85)
  - 上記以外でブラウザ系から来た PNG/JPEG → **WebP** (quality=85, lossless 切替) で 20–40% 削減
- 判定は `image` crate でデコード後にヒストグラム解析。判定コストが書込パスを重くするので、**バックグラウンド再エンコード** (取り込み時は PNG → 後でジョブが最適形式に置換) も選択肢。
- どの場合も DB には `Attachment.format` を保持し、互換性を確保。

### 3.3 設定

```jsonc
{
  "version": 1,
  "history": {
    "max_items": 1000,
    "max_bytes": 524288000,
    "eviction_policy": "size_priority",
    "keep_pinned": true
  },
  "shortcuts": { /* §設定参照 */ },
  "exclusions": [
    { "match": "bundle_id",     "value": "com.1password.1password" },
    { "match": "bundle_id",     "value": "com.bitwarden.desktop" },
    { "match": "exe_basename",  "value": "1Password.exe", "fuzzy": true },
    { "match": "exe_basename",  "value": "Bitwarden.exe", "fuzzy": true },
    { "match": "window_title",  "value": "*KeePassXC*" }
  ],
  "respect_concealed_pasteboard": true,
  "self_write_ignore_ms": 800,
  "formatters": {
    "json": { "indent": 2 },
    "sql":  { "dialect": "postgres", "uppercase": true },
    "markdown": { "wrap": 80 }
  }
}
```

### 3.4 redb テーブル

```
items.value      = AEAD(bincode(ClipItem))
by_time          = (i64 created_at, Ulid) -> ()      # 時系列
by_digest        = [u8;32]                -> Ulid    # 重複
blobs_meta       = BlobId                 -> (refcount, size, location)
packs_meta       = PackId                 -> PackHeader
done.entries     = (date_u32, time_u32, Ulid) -> AEAD(bincode(DoneEntry))
done.overlays    = Ulid                   -> AEAD(bincode(DoneOverlay))
```

### 3.5 Blob Store と Monthly Pack（新規）

レビュー §3.1 対応。

**問題**: 1 ファイル = 1 ペイロード方式だと年間数十万ファイルになり、FS のメタ操作・バックアップ時間・iCloud 同期で破綻する。

**戦略**: 2 段階。

1. **Hot blob** (`blobs/aa/bbcc...sha.enc`)
   - 直近 N 日（既定 30 日）または refcount > 1 のもの。
   - 個別ファイル。差分書込・即時 GC が効く。

2. **Cold pack** (`packs/YYYY-MM.cnxpack`)
   - 月次でバックグラウンドジョブが hot blob を 1 ファイルに連結し、`packs_meta` にオフセット表を持たせる。
   - 各 blob は AEAD 単位のまま（鍵不要で配置のみまとめる）。
   - 削除は tombstone を `packs_meta` に書き、月末に compaction (新パックを書いて差し替え)。

```
packs/2026-04.cnxpack   ── header(version, count) | [enc_blob_1][enc_blob_2]...
packs_meta              ── PackId -> { entries: [(BlobId, offset, len)], live_bytes, total_bytes }
```

UI からは透過的: `Storage::Pack(pack, offset, len)` を読むときに mmap / pread でランダムアクセス。

**追加効果**: バックアップ時に変更があった当月パックだけ転送される（旧月パックは不変 → 差分転送 OK）。

#### Compaction トリガー条件

「月末のみ」だと長期間使うと旧月パックの dead 比率が上がり肥大化するので、**3 つの自動トリガー** + **手動 CLI** の併用。

```rust
struct CompactionPolicy {
    /// dead 領域がパック全体の何 % を超えたら compaction するか (既定: 30%)
    dead_ratio_threshold: f32,
    /// パック単体のサイズがこれを超えたら無条件で再構築 (既定: 256 MiB)
    pack_size_hard_cap: u64,
    /// 当月パックは hot 扱いでスキップ、当月以外は対象
    skip_current_month: bool,
}

enum CompactionTrigger {
    /// 月初 (新しい月のパックに切り替わった瞬間)
    MonthRollover,
    /// バックグラウンドジョブ (週 1 アイドル時)
    Periodic,
    /// blob 削除直後に dead_ratio が閾値超え → 即時キュー
    OnDelete,
    /// ユーザ手動: clipnotex-cli pack compact [--all | --month YYYY-MM]
    Manual,
}
```

判定ロジック: `live_bytes / total_bytes < (1 - dead_ratio_threshold)` または `total_bytes > pack_size_hard_cap` のとき compaction 候補にキュー。実行は **idle 検知** (5 分間ユーザ操作なし) に限定し、I/O で UI を阻害しない。

**手動 CLI** (`tools/devcli`):
```bash
clipnotex-cli pack list                 # パック一覧と dead 比率を表示
clipnotex-cli pack compact --all        # 全パック再構築
clipnotex-cli pack compact --month 2026-04
clipnotex-cli pack verify               # 整合性チェック (entries の offset/len が破綻していないか)
clipnotex-cli pack export --month 2026-04 --out ./archive/    # 別媒体退避
```

ユーザは設定 UI から `dead_ratio_threshold` と `pack_size_hard_cap` を調整可能。デフォルト値で 99% のユーザは触らずに済むことを目指す。

---

## 4. 実装戦略と懸念点（個別）

### 4.1 暗号化

- **方式**: 値単位 AEAD (XChaCha20-Poly1305, 24B nonce 乱数)。
- **鍵管理**: `keyring` から `data_key` (32B) を取り出す/生成。履歴と DONE LOG で別鍵。任意のパスフレーズモード (`argon2id` で KEK 生成、`data_key` をラップ)。
- **平文ライフ**: `zeroize::Zeroizing` で消去。redb には常に暗号文を渡す。
- **AAD**: `format_id || created_at` を AAD に入れて改ざん検出を強化。
- **対象外**: 時系列インデックスのキー（`(created_at, ulid)`）は範囲検索のため平文。

### 4.2 除外アプリ — 3 段階マッチ（更新）

レビュー §3.3 対応。Slack / Electron 系は Helper プロセス名がブレるため bundle_id だけでは不十分。

```rust
enum ExclusionRule {
    BundleId(String),                       // mac 第一優先
    ExeBasename { name: String, fuzzy: bool }, // win + mac fallback。fuzzy は前方一致 / glob
    WindowTitle(GlobPattern),               // 最後の砦 (例 "*1Password*")
}

fn is_excluded(src: &SourceApp, rules: &[ExclusionRule]) -> bool {
    rules.iter().any(|r| match r {
        BundleId(id)    => src.bundle_id.as_deref() == Some(id),
        ExeBasename{name, fuzzy}
                        => src.exe_basename.as_deref().map(|b|
                             if *fuzzy { b.starts_with(name) } else { b == name }
                           ).unwrap_or(false),
        WindowTitle(g)  => src.window_title.as_deref().map(|t| g.matches(t)).unwrap_or(false),
    })
}
```

**取得元の優先順位**:
- mac: `NSWorkspace.shared.frontmostApplication.{bundleIdentifier, localizedName, executableURL}`、加えて `org.nspasteboard.source` UTI が pasteboard に乗っていればそれを最優先。
- win: `GetClipboardOwner()` から `GetWindowThreadProcessId` → `OpenProcess(QUERY_LIMITED)` → `QueryFullProcessImageName`。フォアグラウンド (`GetForegroundWindow`) は **fallback** に格下げ（ユーザがコピー直後にウィンドウを切替えると誤判定するため）。
- ウィンドウタイトル: mac は `CGWindowListCopyWindowInfo` (Accessibility 不要)、win は `GetWindowTextW`。

**機微シグナル**は無条件破棄:
- mac: `org.nspasteboard.ConcealedType`, `org.nspasteboard.TransientType`, `org.nspasteboard.AutoGeneratedType`
- win: クリップボード形式 `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory` (= "0")

### 4.3 整形ペースト

レビュー §2.4 を受け、**MVP は JSON / SQL / Markdown のみ**。

| 言語 | フェーズ | 実装 |
|---|---|---|
| JSON | v0.3 | `serde_json::to_string_pretty` |
| SQL | v0.3 | `sqlformat` (Postgres dialect default) |
| Markdown | v0.3 | `pulldown-cmark` round-trip + 行折り返し |
| HTML / CSS | v0.4+ | `dprint-plugin-markup` (wasm) |
| TS / JS | v0.4+ | `dprint-plugin-typescript` (wasm) |
| PHP | v0.5+ | 外部 `php-cs-fixer` (検出時に同梱しない) |

**言語判定** (MVP):
1. ユーザ明示指定 (UI で言語選択メニュー、最後の選択を覚える)
2. 強いシグナル: 先頭 4KB に `{`/`[` で始まり JSON.parse 成功 → JSON。`SELECT|INSERT|UPDATE|DELETE|WITH` の冒頭 → SQL。`# `/`- `/`* ` の Markdown 構造 → Markdown。
3. それ以外は **判定不能 = フォーマットせずペースト** (誤判定よりマシ)

`hyperpolyglot` は v0.4 から導入し、HTML/JS/CSS/TS が増えたタイミングで投入。

### 4.4 DONE LOG ビュー — 不変原本 + overlay

- 右ペイン (スプレッドシート風) は **overlay 編集 UI**。元データは別カラム / トグルで参照可能。
- エクスポート時は overlay 適用後の view を出力するが、`--with-original` フラグで原本も出せる。
- **検索**: MVP は redb 全件スキャン + プレーン部分一致。v0.5 で `tantivy` を別ファイル (`donelog-fts/`) に。

### 4.5 履歴 UI 仕様（具体化、レビュー §3.2）

§7 に独立章として規定。

---

## 5. OS 仕様の地雷とその回避（新規章）

### 5.1 macOS NSPasteboard 監視

**地雷**:
- `changeCount` だけだと、自アプリ書戻し / 同 ts 内の連続コピー / promised file の遅延でカウントが飛ぶ。
- `pasteboardItems` 取得タイミングで他アプリが上書きしてレースする。

**回避策**:
1. **Self-write guard**: 自分が書いた直近 digest を `LRU<Sha256, Instant>` に保持し、`self_write_ignore_ms` (既定 800ms) 以内なら無視。
2. **changeCount + digest 二重判定**: `(changeCount, blake3(primary))` をキーに重複検出。changeCount は単調増加だが、digest が同じなら "同じコピー" と扱う。
3. **読取は単一トランザクション**: `NSPasteboardItem` を最初に `pasteboard.pasteboardItems.first` でスナップショット取得し、以降はそのオブジェクトに対する `data(forType:)` だけにする。再取得しない。
4. **Promised types**: `com.apple.NSFilePromise*` は遅延データ。コピー時点では空 → **タイプは記録するが payload は空** とし、UI で "deferred" マーク表示。MVP では復元対象外。
5. **org.nspasteboard.source** がある場合、それを `SourceApp.bundle_id` の最優先ソースに。

### 5.2 Windows EnumClipboardFormats — 安全フォーマット ホワイトリスト

**地雷**: `CF_DIB` / `CF_DIBV5` / `CF_ENHMETAFILE` / `CF_HDROP` / `CF_BITMAP` は HGLOBAL の構造が複雑で、誤った GlobalSize / GlobalLock 解釈で UB / クラッシュ。

**回避策**: クリップボード形式を **3 グループ**に分類し、それぞれ専用ハンドラ。

```rust
enum FormatHandler {
    Safe,          // CF_UNICODETEXT, CF_TEXT, registered "HTML Format", "Rich Text Format"
    StructuredBin, // CF_DIB(V5), CF_BITMAP, CF_ENHMETAFILE, CF_HDROP, CF_TIFF
    Unknown,       // 未登録の registered format → 取得スキップ (将来 opt-in)
}

const SAFE_TEXT_FORMATS: &[&str] = &[
    "CF_UNICODETEXT", "CF_TEXT", "CF_OEMTEXT",
    "HTML Format", "Rich Text Format", "FileGroupDescriptorW", "FileNameW",
];
```

各構造化バイナリには専用パーサ:

| Format | 取り扱い |
|---|---|
| `CF_DIB`, `CF_DIBV5` | `BITMAPINFO` ヘッダを解釈し PNG にエンコードして保存 (`image` crate) |
| `CF_BITMAP` | `GetDIBits` 経由で DIB に変換 → 上記と同じ |
| `CF_HDROP` | `DragQueryFileW` で UTF-16 ファイルパス列を取得 → `Vec<PathBuf>` 化、ファイル本体は保存しない |
| `CF_ENHMETAFILE` | MVP では取得スキップ。v0.4 で `GetEnhMetaFileBits` |
| `CF_TIFF` | `image` crate でデコード → PNG 化 |

**未登録 registered format** は取得を試みず、format name のみ記録（"このアイテムには XYZ アプリ独自データあり" と UI 表示）。

**HGLOBAL 取り扱い**: 必ず `GlobalLock` → `GlobalSize` → `slice::from_raw_parts` → `Vec` コピー → `GlobalUnlock`。`unsafe` ブロックは `clipnotex-clipboard::windows::hglobal` モジュールに集約し、`// SAFETY:` コメント必須。

### 5.3 完全復元ペーストの安全性 — 段階的戦略

レビュー §2.3 対応。MVP の "退避→復元方式" は他アプリが監視中に流出するリスクがあるため、**3 段階**に分けて段階導入。

**Stage A (MVP, v0.2)**: 退避→書戻しモード
1. 現在のクリップボード全フォーマットを `RestorePoint` に退避
2. 履歴アイテムの全フォーマットを書込
3. `enigo` で Cmd+V / Ctrl+V 注入
4. 100–200ms 待ってから `RestorePoint` を書戻し

⚠️ この間、他アプリ（クリップボードマネージャ等）が走査すると履歴データが見える。**UI で警告表示**「他のクリップボードマネージャが動作中の場合、内容が記録される可能性があります」。

**Stage B (v0.4)**: クリップボード非経由モード（テキストのみ）
- 対象: プレーンテキスト・コードのみ。
- リッチテキスト・画像は対象外（ターゲットが受け付けない）。
- **3 段階フォールバック**を必ず実装する。`SetValue` だけでは Chrome / Electron / Java Swing / 一部の Windows ストアアプリで沈黙失敗するため、必ず段階的に降りる。

**macOS フォールバック**:
1. `AXUIElementCopyAttributeValue(focused, kAXFocusedUIElementAttribute)` → `AXUIElementSetAttributeValue(elem, kAXValueAttribute, str)`
2. ↑が `kAXErrorNoValue` / `kAXErrorAttributeUnsupported` を返す → `CGEventCreateKeyboardEvent` で **Unicode keystroke** を文字列として注入 (`CGEventKeyboardSetUnicodeString`)
3. ↑も IME 競合等で失敗 → **Stage A** に降格（クリップボード経由 + 警告）

**Windows フォールバック**:
1. `IUIAutomationValuePattern::SetValue(focused_element, str)` を試行。`HRESULT` 成功 + 値が反映されたかを `GetValue` で検証
2. 失敗 / 反映されない (Chrome, Electron, Java Swing 等で頻発) → `SendInput` で **Unicode keystroke** (`KEYEVENTF_UNICODE`, `wScan = char`) を 1 文字ずつ送出
3. それでも失敗 (パスワードフィールド等で `KEYEVENTF_UNICODE` を弾くケース) → **Stage A** に降格

```rust
enum PasteAttempt { DirectInject, UnicodeKeystroke, ClipboardFallback }

fn paste_text_safely(text: &str, target: FocusedTarget) -> Result<PasteAttempt> {
    if try_direct_inject(text, &target).is_ok()      { return Ok(DirectInject); }
    if try_unicode_keystroke(text).is_ok()           { return Ok(UnicodeKeystroke); }
    stage_a_clipboard_paste(text)?;
    Ok(ClipboardFallback)
}
```

各試行のテレメトリ（成功率 by アプリ bundle_id / exe）はローカルにのみ保持し、次回同じターゲットには成功した手段を最優先で試す。「どの段階で成功したか」を UI のステータスバーに小さく表示（デバッグ目的、デフォルト OFF）。

**Stage C (v0.5+)**: 透明モード
- mac: `NSPasteboard.with(name: .init("ClipNoteXPrivate"))` のような **named pasteboard** に書き、ターゲットアプリにペーストイベントを向ける（要 AX 介入）。
- win: 当面研究フェーズ。

各モードはユーザが選択可能 (`paste_full.mode = "stageA" | "stageB" | "auto"`)。

### 5.4 監視ループ防止

自分の書込で `changeCount` が回って再キャプチャ → 無限ループの典型バグ。`self_write_guard` (LRU + 時間窓) と、書込時に **意図的に digest を pre-register** する両輪で対策。

### 5.5 macOS の権限ウィザード

- Accessibility (キー注入・AX paste) → 初回起動時に「機能制限モードで起動 → 権限取得後に有効化」ウィザード。
- ScreenRecording 不要（クリップボードは要らない）。
- Input Monitoring もキー注入 (`enigo`) なら原則不要だが、Catalina 以降の挙動を要確認。

---

## 6. MVP スコープ（再構築）

レビュー §5 をそのまま採用。

### v0.1 — Clipy 置き換え最小構成

| 機能 | 詳細 |
|---|---|
| 履歴: テキスト・画像 | mac は public.utf8-plain-text / public.png、win は CF_UNICODETEXT / CF_DIB → PNG 変換 |
| 件数 + 容量上限 | 両ポリシー (count_priority / size_priority) |
| 暗号化保存 | XChaCha20-Poly1305 + keyring |
| 履歴 UI | §7 のフルスペック (検索バー / サムネイル / フォーマットアイコン / 矢印選択 / Enter ペースト) |
| グローバルショートカット | Cmd+Shift+V / Ctrl+Shift+V |
| 除外アプリ | 3 段階マッチ + 機微シグナル尊重、デフォルトで 1Password / Bitwarden / KeePassXC |
| トレイ常駐 | mac メニューバー / win タスクトレイ |
| 自動アップデート | Tauri updater + signed |

### v0.2

- スニペット (Cmd/Ctrl+Shift+C, フォルダ階層)
- RTF / HTML / PDF / Files の保存（復元は同じフォーマットのみ）
- 完全復元ペースト Stage A (退避→書戻し方式、警告 UI つき)
- ピン留め

### v0.3

- 整形ペースト: **JSON / SQL / Markdown のみ**
- DONE LOG キャプチャ + **読み取り専用** 2 ペイン UI (左: カレンダー、右: スプレッドシート風表示)
- DONE LOG エクスポート (テンプレート文字列)

### v0.4

- DONE LOG 編集 (overlay 方式) + 横断検索 + 日報出力テンプレ強化
- 完全復元 Stage B (クリップボード非経由 / テキストのみ)
- 整形ペースト言語追加: HTML / CSS / TS / JS（dprint wasm）
- アーカイブ機能（古い DONE LOG を別ファイルに）

### v0.5+

- WASM フォーマッタプラグイン
- Tantivy 全文検索
- OCR (画像 → テキスト DONE)
- 完全復元 Stage C 研究

### 非ゴール

- クラウドサーバ・チーム共有
- モバイル
- AI 要約 (将来オプションのみ)

---

## 7. 履歴 UI 仕様（具体化）

### 7.1 ポップアップウィンドウ

```
┌─────────────────────────────────────────────────────────┐
│ 🔍 [_______________________________]   [⚙ 設定] [×]    │  <- 検索バー (常時フォーカス)
├─────────────────────────────────────────────────────────┤
│ [📌] 📝 "TODO: refactor auth..."        Slack    14:32 │  <- 1行 = 1アイテム
│ [  ] 🖼 [サムネイル 32x32]              Figma    14:30 │
│ [  ] 🌐 "<table><tr>..."                Notion   14:28 │
│ [  ] 📄 sample.pdf                      Finder   14:20 │
│ [  ] 💻 "function foo() {...}" json     VSCode   14:15 │
│ ...                                                     │
├─────────────────────────────────────────────────────────┤
│ プレビュー: ───────────────────────────────────────── │
│ {                                                       │
│   "name": "foo",                                        │
│   "value": 42                                           │
│ }                                                       │
│ ─────────────────────────────────────────────────────── │
│ Enter:ペースト  ⌘Enter:プレーン  ⌥Enter:整形  ⌘P:ピン │
└─────────────────────────────────────────────────────────┘
```

- **キーバインド**:
  - 任意キー入力 → 即座に検索フィルタ
  - ↑↓ / Ctrl+J/K / Vim キー: 選択移動
  - Enter: 通常ペースト
  - Cmd/Ctrl+Enter: プレーンテキストペースト
  - Opt/Alt+Enter: 整形ペースト (整形可能な場合のみ)
  - Shift+Enter: 完全復元ペースト
  - Cmd/Ctrl+P: ピン留めトグル
  - Cmd/Ctrl+Delete: 削除
  - 1–9: クイック選択（上から N 番目）
  - Esc: ウィンドウを閉じる（フォーカス元に戻す）

- **フォーマットアイコン**: `📝`(text) `🖼`(image) `🌐`(html) `📄`(pdf) `📁`(files) `💻`(code) `🎨`(rtf)
- **言語バッジ**: code 検出時に `json` `sql` `js` `md` 等を小さく表示
- **サムネイル**: 画像は 32x32 でキャッシュ (`thumbnails/<sha>.png`、暗号化対象外: 既に小さくマスクされた表示用)
- **プレビューペイン**: 選択中アイテムの先頭 8KB / 画像なら原寸スケール表示。重い場合は遅延ロード。
- **仮想スクロール**: 1000 件以上を想定し `react-virtuoso`。

### 7.2 ウィンドウ挙動 と IME 対応

- 表示位置: マウスカーソル近傍 / 直近のフォーカスウィンドウ中央 / 画面中央 をユーザ設定で選択
- フォーカスを取らない: 表示時に元アプリのフォーカスを保持し、ペースト時に確実に元アプリに送る
  - mac: `NSPanel` + `nonactivatingPanel` スタイル
  - win: `WS_EX_NOACTIVATE` + `SetForegroundWindow` を使わない
- 閉じるトリガー: Esc / フォーカスアウト / ペースト実行後

#### IME 対応（macOS）

`nonactivatingPanel` のままだとアプリが key window を取れず、**日本語/中国語/韓国語の IME 変換が起動しない**（`NSTextInputContext` がアクティブにならないため）。これは検索バーが事実上使えなくなる致命的問題。

**採用方針**: **遅延アクティベーション**。

1. パネル表示直後はフォーカスを元アプリに残す（矢印キー / 数字キー / Vim キーで選択を完結できる前提）
2. 検索バーへの **最初の文字入力**（あるいは検索バーへの明示的なクリック）を検知したタイミングで、初めて `panel.makeKeyAndOrderFront(nil)` を呼んで key window 化 → IME が動作開始
3. ペースト確定 (`Enter`) 前に、ペースト先のフォーカスを `previousKeyWindow` に戻してから注入
4. Esc 時も同様にフォーカス復元

```swift
// 概念コード
class HistoryPanel: NSPanel {
    var previousFrontApp: NSRunningApplication?
    func showFloating() {
        previousFrontApp = NSWorkspace.shared.frontmostApplication
        self.styleMask.insert(.nonactivatingPanel)
        self.orderFrontRegardless()
    }
    func didStartTyping() {
        self.styleMask.remove(.nonactivatingPanel)  // 一時的に通常 NSWindow 化
        self.makeKeyAndOrderFront(nil)              // IME 起動
    }
    func performPaste(item: ClipItem) {
        previousFrontApp?.activate(options: [])
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(50)) {
            paster.paste(item)
        }
    }
}
```

代替案として「**検索バーだけ別の通常 NSWindow を子ウィンドウで重ねる**」も検討したが、レイアウト崩れと z-order 管理が複雑なので採用しない。遅延アクティベーション方式を MVP で実装する。

#### IME 対応（Windows）

Windows では `WS_EX_NOACTIVATE` でも TSF (Text Services Framework) は基本動作するが、**IME 候補ウィンドウが親ウィンドウの z-order に追従しない**ことがある。対策:

1. `ITfThreadMgr` を初期化し、検索バーフォーカス時に明示的に `ITfDocumentMgr` を push
2. 候補ウィンドウ位置は `ImmSetCandidateWindow` で IME に強制指定
3. それでも問題が出る環境向けに、設定で「常に通常ウィンドウとして表示」を選択可能にする（フォーカス奪取の代償あり）

#### 検索バー自体の挙動

- macOS: 表示と同時に `firstResponder` を search field に設定（ただし上記の通り key window 化はしない）
- Windows: `SetFocus` を search field 子コントロールに発行
- いずれも **方向キー / Enter / Esc は検索バー内で消費せず一覧側に流す** (search field の key handler で intercept)

---

## 8. リポジトリ構成と OSS 運用

```
ClipNoteX/
├─ apps/
│  └─ desktop/                  # Tauri アプリ
│     ├─ src/
│     │  ├─ features/{history,snippets,donelog,settings}/
│     │  ├─ components/
│     │  ├─ ipc/                # 型安全 invoke ラッパ (ts-rs で Rust から型生成)
│     │  └─ i18n/{ja,en}.json
│     └─ src-tauri/
├─ crates/                      # §1.2
├─ tools/
│  ├─ devcli/                   # DBダンプ・テストデータ・blob pack 操作
│  └─ packaging/                # codesign / notarize / msi 生成
├─ docs/
│  ├─ architecture.md           # §1 + 図
│  ├─ adr/                      # 決定記録 (redb, encryption, paste strategy …)
│  ├─ os-pitfalls.md            # §5 を独立化 (コントリビュータ必読)
│  └─ user-guide/
├─ .github/
│  ├─ workflows/{ci,release,lint}.yml
│  ├─ ISSUE_TEMPLATE/
│  │  ├─ bug_report.yml
│  │  ├─ feature_request.yml
│  │  └─ formatter_request.yml  # §9.2
│  └─ PULL_REQUEST_TEMPLATE.md
├─ Cargo.toml                   # workspace
├─ package.json                 # pnpm workspace
├─ README.md                    # §9.1
├─ CONTRIBUTING.md
├─ CODE_OF_CONDUCT.md
├─ SECURITY.md
├─ CHANGELOG.md
└─ LICENSE                      # Apache-2.0
```

**コーディング規約**: `rustfmt` + `clippy -D warnings`、`unsafe` は `clipnotex-clipboard` の OS FFI に限定し `// SAFETY:` 必須。TS は `biome` + `tsc --strict`。
**コミット**: Conventional Commits → `release-please`。
**CI**: `cargo test` / `clippy` / `pnpm test` / `pnpm typecheck` を mac/win/linux マトリクス。
**リリース**: tag push → mac universal2 (codesign+notarize) / win msi+exe (signed) を GitHub Releases。

---

## 9. README / Issue テンプレ提案（レビュー §4 対応）

### 9.1 README 構成案

```markdown
<p align="center">
  <img src="docs/assets/logo.svg" width="120">
  <h1 align="center">ClipNoteX</h1>
  <p align="center">
    <strong>Clipy 上位互換のクロスプラットフォームクリップボードマネージャ</strong><br>
    完全復元 ・ 整形ペースト ・ DONE LOG を備えたプロ向け OSS
  </p>
  <p align="center">
    <a href="..."><img src="https://img.shields.io/github/v/release/.../ClipNoteX"></a>
    <a href="..."><img src="https://img.shields.io/github/stars/.../ClipNoteX"></a>
    <a href="..."><img src="https://img.shields.io/badge/macOS-13%2B-blue"></a>
    <a href="..."><img src="https://img.shields.io/badge/Windows-10%2B-blue"></a>
    <a href="..."><img src="https://img.shields.io/badge/license-Apache--2.0-green"></a>
  </p>
</p>

![hero demo](docs/assets/demo-history.gif)

## ✨ Features
- 📋 **Clipy 互換** の操作感 (`⌘⇧V` / `Ctrl+Shift+V`)
- 🧬 **完全復元ペースト** — RTF/HTML/画像など元のフォーマットを保持
- ✨ **整形ペースト** — JSON / SQL / Markdown を自動整形してペースト
- 📓 **DONE LOG** — "やったこと" を蓄積して日報を生成
- 🔒 **暗号化ローカル保存** — XChaCha20-Poly1305 + OS Keychain
- 🔇 **パスワードマネージャ自動除外** — 1Password / Bitwarden / KeePassXC
- 🖥 **Win/Mac 両対応** — 単一バイナリ、軽量 (Tauri 2)

## 🎬 Demos
| 履歴 | 整形ペースト | DONE LOG |
|---|---|---|
| ![](docs/assets/demo-history.gif) | ![](docs/assets/demo-format.gif) | ![](docs/assets/demo-donelog.gif) |

## 🚀 Quick Start
[Download latest release](...) → 起動 → メニューバー / トレイから利用

## ⌨ Shortcuts (default)
| Action | macOS | Windows |
|---|---|---|
| 履歴ポップアップ | `⌘⇧V` | `Ctrl+Shift+V` |
| スニペット      | `⌘⇧C` | `Ctrl+Shift+C` |
| プレーン貼付    | `⌘⌃V` | `Ctrl+Shift+Alt+V` |
| 整形貼付        | `⌘⌥V` | `Ctrl+Alt+V` |
| 完全復元貼付    | `⌘⇧⌥V`| `Ctrl+Shift+Alt+F` |
| DONE 記録       | `⌘⇧D` | `Ctrl+Shift+D` |

## 🏗 Architecture

```
┌──────────────┐  IPC  ┌──────────────────────────────┐
│  WebView UI  │◄─────►│  Rust Core                   │
│ (React + TS) │       │  ┌─────────────────────────┐ │
│  - 履歴      │       │  │ ClipboardWatcher        │ │
│  - スニペット│       │  │ ExclusionFilter         │ │
│  - DONE LOG  │       │  │ StoreService (redb+AEAD)│ │
│  - 設定      │       │  │ PasteController         │ │
└──────────────┘       │  │ HotkeyService           │ │
                       │  └─────────────────────────┘ │
                       └──────────────────────────────┘
                              ▲                ▲
                       OS Clipboard    Global Shortcuts
```

詳細は [docs/architecture.md](docs/architecture.md)

## 🛠 Development
... (cargo / pnpm / tauri dev コマンド)

## 🤝 Contributing
[CONTRIBUTING.md](CONTRIBUTING.md) を参照。
**Good first issues**: フォーマッタ追加 / 言語検出ヒューリスティクス / UI 翻訳 / テーマ。

## 🔐 Security
[SECURITY.md](SECURITY.md) — 脆弱性は GitHub Security Advisory へ。

## 📜 License
Apache-2.0
```

### 9.2 Issue テンプレ「Formatter 追加」案

`.github/ISSUE_TEMPLATE/formatter_request.yml`:

```yaml
name: 🎨 Formatter Request
description: 整形ペースト対応言語の追加リクエスト
labels: ["formatter", "good first issue"]
body:
  - type: input
    id: language
    attributes:
      label: 対象言語
      placeholder: e.g. YAML
    validations: { required: true }
  - type: dropdown
    id: detector
    attributes:
      label: 言語判定の手段
      options:
        - 拡張子ベース
        - 内容ヒューリスティクス
        - hyperpolyglot で十分
        - 不明
  - type: input
    id: library
    attributes:
      label: 利用候補ライブラリ (Rust crate / WASM プラグイン)
      placeholder: e.g. serde_yaml
  - type: textarea
    id: sample
    attributes:
      label: フォーマット前 / 後のサンプル
      render: yaml
  - type: checkboxes
    id: implementation
    attributes:
      label: 実装範囲
      options:
        - label: フォーマッタ実装 (`crates/clipnotex-format/src/lang/`)
        - label: 言語判定追加 (`crates/clipnotex-format/src/detect.rs`)
        - label: 設定スキーマ拡張
        - label: ドキュメント更新
```

### 9.3 docs/architecture.md に載せる図

§1.3 のデータフロー図 + §3.5 のストレージ階層図 + §5.3 の Stage A/B/C 図。すべて ASCII / mermaid で描き、SVG レンダリングは `mermaid-cli` で CI 生成。

---

## 10. 実装フェーズ注意点（新規）

実装に着手するコントリビュータが踏みやすい罠を、設計の段階で文書化しておく。`docs/os-pitfalls.md` に転記してコントリビュータ必読資料にする。

### 10.1 macOS NSPasteboardItem は遅延読み込み

`pasteboardItem.data(forType:)` は **呼び出した瞬間に**ソースアプリへ IPC が走り、データが生成・転送される（特に画像 / PDF / promised file）。

- **UI スレッドで呼ぶと数百ms ブロックする** ことがある。必ず `tokio::task::spawn_blocking` または専用ワーカスレッドで実行。
- promised types は data 取得前にソースアプリが終了すると失敗するので `Result` で受け、失敗時は payload を空のまま記録。
- 取得は **§5.1 で取った単一スナップショット**に対してまとめて行い、複数 type を **逐次** 取得する（並列にすると一部 OS バージョンで racey）。

```rust
async fn capture_pasteboard(snapshot: NSPasteboardItem) -> Result<Vec<Payload>> {
    tokio::task::spawn_blocking(move || {
        snapshot.types()
            .into_iter()
            .filter(|t| safelist::is_safe(t))
            .map(|t| read_one(&snapshot, &t))   // 逐次
            .collect()
    }).await?
}
```

### 10.2 Windows CF_HDROP はファイル本体を保存しない（理由を明記）

設計上「パスのみ保存」としているが、その**理由**を docs に明記:

1. **サイズ爆発**: ユーザは数 GB のファイル群をコピーすることがある。履歴に取り込めば DB が破綻。
2. **コピー意味の歪み**: ユーザは「ファイルへの参照を貼る」つもりであり、内容のスナップショットを残したい意図とは異なる。
3. **ペースト先の互換性**: Explorer / メールクライアント等は HDROP のパスを期待しており、内容を直接貼っても動作しない。
4. **権限問題**: ファイルが消えたり権限が変わると、後でアクセス不能になる。「**コピー時のパスを保持**」が一番予測可能。

UI には「ファイル参照（N 個）」と表示し、ペースト時に `CF_HDROP` を再構築する。元ファイルが既に存在しない場合は警告を出す。

### 10.3 redb トランザクションは短時間で閉じる

redb は `WriteTransaction` を 1 つしか持てず、長時間保持すると後続書込が全部ブロックされ、UI 操作のあらゆる待機につながる。

**規律**:
- **UI スレッドで DB を直接触らない**。Tauri command は必ず `tokio::spawn` 経由で Core のサービスに委譲。
- **1 トランザクション = 1 ユースケース**。「履歴 1 件追加 + by_time 更新 + by_digest 更新」のような関連書込は同一 tx だが、それを複数アイテムでまとめない。
- 大物 blob の暗号化・圧縮・ファイル書込は **tx の外で完了** させ、最後に redb には参照だけ書く。
- **読込はスナップショット**: `ReadTransaction` は cheap なので、UI からのクエリは毎回新規に開く。
- ロック競合検知用に、tx 開始時に `Instant::now()` を保持し、`Drop` 時に `> 50ms` 経過していたら warn ログを出す（開発時の早期発見用）。

```rust
async fn add_clip_item(svc: Arc<StoreService>, item: ClipItem) -> Result<()> {
    let blob_refs = svc.write_blobs_outside_tx(&item).await?;     // tx 外
    tokio::task::spawn_blocking(move || {
        let tx = svc.db.begin_write()?;                            // tx は短く
        {
            let mut items   = tx.open_table(ITEMS)?;
            let mut by_time = tx.open_table(BY_TIME)?;
            let mut by_dig  = tx.open_table(BY_DIGEST)?;
            items.insert(item.id.as_bytes(), encrypted(&item)?)?;
            by_time.insert(&(item.created_at, item.id), &())?;
            by_dig.insert(&item.digest, &item.id)?;
        }
        tx.commit()?;                                              // ここで閉じる
        Ok(())
    }).await?
}
```

### 10.4 自分が書いたクリップボードを再キャプチャしないループ防止（再掲・実装観点）

`§5.4` で言及済みだが、実装時の具体的注意:
- `enigo` でキー注入する **直前** に、これから貼るペイロードの digest を `self_write_guard` に登録（書込後ではなく前。書込から changeCount 反映までの遅延に勝たせる）。
- guard の TTL は `self_write_ignore_ms` (既定 800ms)。短すぎると見逃し、長すぎると正規のコピーを取り損ねる。
- guard ヒット時もログには残す（`debug!("clipboard change ignored: self-write")`）。デバッグで「コピーしたのに履歴に出ない」を切り分ける根拠になる。

### 10.5 グローバルショートカット衝突

OS / 他アプリと衝突したショートカットは無音で登録失敗することがある（特に Windows）。
- 登録結果 (`Result`) を必ず確認し、失敗時は設定 UI で警告表示 + 代替候補を提案。
- macOS は Mission Control / Spotlight 等のシステムショートカットと衝突しやすい。デフォルト値選定時に主要環境で衝突確認テストを CI に組み込む。

---

## 11. 主な懸念点まとめ（更新）

| 領域 | 懸念 | 対応 |
|---|---|---|
| mac changeCount 誤検知 | 自書戻し / promised file | self_write_guard + digest 二重判定 |
| win バイナリ format | DIB/HDROP 等の UB リスク | ホワイトリスト + 個別パーサ + unsafe 局所化 |
| 完全復元の流出 | 他クリップボード監視で可視化 | Stage A は警告 UI、Stage B でクリップボード非経由化 |
| 整形ペースト誤判定 | PHP/HTML/SQL 曖昧 | MVP は JSON/SQL/MD のみ、判定不能なら整形しない |
| DONE LOG 編集が破壊的 | 原本喪失 | overlay 方式で原本保持 |
| blob ファイル爆発 | FS / バックアップ破綻 | hot blob + monthly pack + 月末 compaction |
| ペースト権限 (mac AX) | 初回ハードル | 機能限定モード + ウィザード |
| 自分の書込で監視ループ | クラシックバグ | self_write_guard 必須 |
| アップデートでスキーマ変更 | DB マイグレ | redb の `version` キー + マイグレーション層 |
| Stage B が一部アプリで沈黙失敗 | Chrome/Electron/Java の SetValue 不発 | SetValue → Unicode keystroke → Stage A の 3 段階フォールバック (§5.3) |
| `nonactivatingPanel` で IME 不動 | 日本語検索バーが使えない | 遅延アクティベーション方式 (§7.2) |
| pack の dead 領域肥大 | 月末のみだと古い pack が膨張 | dead_ratio / size_cap / OnDelete の 3 トリガー + CLI (§3.5) |

---

## 12. 次の具体アクション

1. `pnpm create tauri-app apps/desktop` + `cargo new --lib crates/*` で workspace スケルトン作成
2. `clipnotex-core` に `ClipItem` / `Settings` / `Error` / `EventBus` を定義、`ts-rs` で TS 型生成
3. `clipnotex-store` の redb + AEAD + blob (hot のみ) 実装、ユニットテスト
4. `clipnotex-clipboard` で **mac テキスト + 画像 / win テキスト + DIB→PNG** を実装（§5.1, §5.2 のガード込み）
5. `clipnotex-hotkey` + `clipnotex-paste` で「履歴ポップアップ → 選択 → ペースト」をクリティカルパスとして貫通
6. UI 側で §7 の履歴ポップアップを実装、検索 / 矢印 / Enter まで
7. 除外アプリ 3 段階マッチ + 設定 UI を v0.1 リリース要件として完成
8. monthly pack は **v0.1 では実装しない** が、`Storage::Pack(...)` の enum バリアントだけ用意して将来追加に備える

---

*この文書は ADR の素になる暫定設計です。各 crate の API 確定と DB スキーマの v1 凍結は、v0.1 alpha 実装後にレビューしてください。*
