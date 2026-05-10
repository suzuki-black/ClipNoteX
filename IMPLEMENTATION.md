# ClipNoteX v0.1 — 実装計画 (Implementation Plan)

> 対応する設計書: [DESIGN.md](DESIGN.md) v0.2
>
> 本書は v0.1 (Clipy 置き換え最小構成) を「動くものに変える」ための実装ガイド。
> タスク間の依存関係、実装順序、技術的補足、コードスケルトンの所在を一元化する。
>
> **Rev 2**: 懸念点対応表（§6）と各マイルストーンへの安全策マッピングを追加。

---

## 0. 実装順序の最適化（依存関係グラフ）

優先順位は **「動かないと先に進めない縦軸」を先に通す** のが基本方針。横展開（プラットフォーム差分・UI 装飾）は後。

```
                        ┌─────────────────────┐
                        │ M0: workspace 初期化│  ← 全ての出発点
                        └────────┬────────────┘
                                 ▼
       ┌─────────────────────────────────────────────┐
       │ M1: clipnotex-core (型 + Settings + Bus)    │  ← 全 crate が依存
       └────────┬────────────────────────────────────┘
                ▼
   ┌────────────────────────┐    ┌──────────────────────────┐
   │ M2: clipnotex-store    │    │ M3: clipnotex-clipboard  │
   │  (redb + AEAD + blob)  │    │  (mac/win, テキストのみ)  │
   └─────────┬──────────────┘    └─────────┬────────────────┘
             │                             │
             └──────────────┬──────────────┘
                            ▼
                ┌────────────────────────┐
                │ M4: capture pipeline   │  ClipboardWatcher → Filter → Store
                │ (clipnotex-app)        │  ← ここで「履歴が DB に溜まる」が成立
                └─────────┬──────────────┘
                          ▼
   ┌─────────────────────────┐   ┌──────────────────────────┐
   │ M5: clipnotex-hotkey    │   │ M6: clipnotex-paste      │
   │ (global-hotkey 登録)    │   │ (キー注入 + 退避)         │
   └─────────┬───────────────┘   └─────────┬────────────────┘
             └──────────────┬──────────────┘
                            ▼
                ┌────────────────────────┐
                │ M7: paste round-trip   │  HK → 直近1件を再ペースト
                │  (CLI で疎通確認)       │  ← クリティカルパス貫通点
                └─────────┬──────────────┘
                          ▼
       ┌─────────────────────────────────────────────┐
       │ M8: clipnotex-tauri + UI 履歴ポップアップ    │
       │  (検索・矢印・Enter・プレビュー)             │
       └────────┬────────────────────────────────────┘
                ▼
    ┌────────────────────┐    ┌────────────────────────┐
    │ M9: 画像対応        │    │ M10: 除外アプリ         │
    │ (PNG / DIB→PNG)    │    │ (3 段階マッチ + 機微)    │
    └─────────┬──────────┘    └─────────┬──────────────┘
              └────────────┬─────────────┘
                           ▼
              ┌─────────────────────────┐
              │ M11: Quota Manager       │
              │ (件数・容量 + eviction)   │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │ M12: 設定 UI             │
              │ (上限・除外・ショートカット)│
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │ M13: トレイ + 起動制御   │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │ M14: パッケージング       │
              │ (codesign / notarize)    │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │ M15: テスト + リリース    │
              └─────────────────────────┘
```

**重要原則**:

1. **M7 まで一気に通す** ことが何よりも重要。「コピー → 履歴に入る → ショートカット → 再ペースト」が動かない限り、UI 装飾も設定 UI も意味がない。**最初の 2 週間で M7 を通す**ことを目標に置く。
2. **M3 (clipboard) と M2 (store) は並行**で進める。依存しない。
3. **M9 (画像) は M8 の後** にする。テキストだけでも UI は完成形に近づくので、画像対応で UI を遅延させない。
4. **除外アプリ (M10) は M9 と並行可** だが、リリース要件なので M11 (Quota) より前に必ず終わらせる。
5. **設定 UI (M12) は最後** で構わない。デフォルト値で開発者は困らない。

---

## 1. マイルストーン詳細

### M0: workspace 初期化（半日）

| タスク | 成果物 |
|---|---|
| Cargo workspace 作成 | `Cargo.toml` (workspace), `rust-toolchain.toml` |
| Tauri アプリ初期化 | `apps/desktop/` (`pnpm create tauri-app --template react-ts`) |
| pnpm workspace | `package.json`, `pnpm-workspace.yaml` |
| CI 雛形 | `.github/workflows/ci.yml` (lint + test on mac/win) |
| ライセンス・README 雛形 | `LICENSE` (Apache-2.0), `README.md`, `CONTRIBUTING.md`, `SECURITY.md` |
| Issue テンプレ | `.github/ISSUE_TEMPLATE/{bug_report,feature_request,formatter_request}.yml` |
| `.gitignore` | Rust + Node + Tauri |

**完了条件**: `cargo check --workspace` と `pnpm install && pnpm --filter desktop tauri dev` が両方成功する。

---

### M1: clipnotex-core（1〜2日）

最初に固める「全 crate が共有する語彙」。後から壊すと全 crate を直すハメになる。

**実装範囲**:

```rust
// crates/clipnotex-core/src/lib.rs
pub mod model;     // ClipItem, PayloadRef, SourceApp, ClipKind, Compression
pub mod settings;  // Settings, HistoryConfig, ShortcutConfig, ExclusionRule
pub mod error;     // CnxError, Result
pub mod bus;       // EventBus (tokio::broadcast ラッパ)
pub mod ids;       // type Ulid 等
```

**ポイント**:
- `serde::{Serialize, Deserialize}` + `bincode` で DB シリアライズ、`ts-rs` で TS 型生成。
- `EventBus` は `tokio::sync::broadcast::Sender<CoreEvent>` を `Arc` で持つ薄ラッパ。`CoreEvent` は `enum { ClipboardCaptured(Ulid), HotkeyPressed(HotkeyId), Quota(QuotaEvent), ... }`。
- `Settings` は不変 (`Arc<Settings>`) で配り、変更は `SettingsService` 経由で新しい `Arc` に差し替える設計（読み手はロック不要）。

**テスト**: serde round-trip、Settings の JSON スキーマ検証。

---

### M2: clipnotex-store（3〜4日）

**実装範囲**:

```rust
// crates/clipnotex-store/src/lib.rs
pub mod aead;       // XChaCha20-Poly1305 + keyring 連携
pub mod blobs;      // hot blob のみ (pack は v0.2 以降)
pub mod tables;     // redb テーブル定義 (TableDefinition の const)
pub mod store;      // StoreService (公開 API)
pub mod migrations; // schema_version: u32
```

**設計補足**:

```rust
pub struct StoreService {
    db: Arc<redb::Database>,
    aead: Arc<DataKeys>,       // history_key, donelog_key
    blob_root: PathBuf,
}

impl StoreService {
    pub async fn add_item(&self, item: ClipItem, payloads: Vec<PayloadData>) -> Result<()>;
    pub async fn get_item(&self, id: Ulid) -> Result<Option<ClipItem>>;
    pub async fn list_recent(&self, limit: usize, query: Option<&str>) -> Result<Vec<ClipItem>>;
    pub async fn delete(&self, id: Ulid) -> Result<()>;
    pub async fn touch(&self, id: Ulid, now: i64) -> Result<()>;  // dedupe ヒット用
    pub async fn count_and_bytes(&self) -> Result<(u64, u64)>;
    pub async fn evict(&self, policy: EvictionPolicy) -> Result<u64>;  // 返り値 = 削除件数
}
```

**実装注意 (DESIGN §10.3)**:
- 全公開 API は `tokio::task::spawn_blocking` 内で redb tx を完結。
- blob 書込は **tx の外** で完了させ、参照だけ tx 内に書く。
- `add_item` は冪等 (`by_digest` ヒット時は `touch` のみ)。

**鍵管理** (`aead.rs`):
```rust
pub fn load_or_create_keys() -> Result<DataKeys> {
    let entry = keyring::Entry::new("ClipNoteX", "data_key")?;
    match entry.get_password() {
        Ok(b64) => DataKeys::from_b64(&b64),
        Err(keyring::Error::NoEntry) => {
            let keys = DataKeys::generate();
            entry.set_password(&keys.to_b64())?;
            Ok(keys)
        }
        Err(e) => Err(e.into()),
    }
}
```

**テスト**:
- 暗号化 round-trip
- redb 書込 → 別プロセスで読込
- eviction が古い順に削除し、`pinned` を残すこと
- 1000 件 / 100 MB の負荷テストで p99 < 50ms を測る (Criterion)

---

### M3: clipnotex-clipboard（4〜5日 — 最大の難所）

**実装範囲**:

```rust
// crates/clipnotex-clipboard/src/lib.rs
pub mod platform;      // OS 抽象 trait
pub mod safelist;      // 安全フォーマット定義
pub mod source;        // SourceApp 取得
pub mod guard;         // self-write guard (LRU<Sha256, Instant>)

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub use platform::{ClipboardWatcher, ClipboardWriter, CapturedItem};
```

**抽象 trait**:
```rust
#[async_trait]
pub trait ClipboardWatcher: Send + Sync {
    async fn next(&mut self) -> Result<Option<CapturedItem>>;
    fn register_self_write(&self, digest: [u8; 32]);
}

pub trait ClipboardWriter: Send + Sync {
    fn write(&self, payloads: &[PayloadData]) -> Result<()>;
    fn snapshot_for_restore(&self) -> Result<Vec<PayloadData>>;  // 退避用
}

pub struct CapturedItem {
    pub source_app: SourceApp,
    pub payloads: Vec<PayloadData>,
    pub primary_kind: ClipKind,
    pub digest: [u8; 32],
    pub captured_at: i64,
}
```

#### M3-mac

```rust
// macos.rs (擬似コード)
pub struct MacWatcher {
    pasteboard: id,                             // NSPasteboard.general
    last_change_count: AtomicI64,
    guard: Arc<SelfWriteGuard>,
    poll_interval: Duration,                    // 100ms
}

async fn next(&mut self) -> Result<Option<CapturedItem>> {
    loop {
        tokio::time::sleep(self.poll_interval).await;
        let cc = unsafe { msg_send![self.pasteboard, changeCount] };
        if cc == self.last_change_count.load(Acquire) { continue; }
        self.last_change_count.store(cc, Release);

        // 単一スナップショット (DESIGN §5.1, §10.1)
        let item: id = unsafe { msg_send![self.pasteboard, pasteboardItems] };
        let first = nsarray_first(item)?;
        let captured = tokio::task::spawn_blocking(move || {
            read_all_safe_types(first)
        }).await??;

        if self.guard.contains(&captured.digest) { continue; }
        return Ok(Some(captured));
    }
}
```

**機微 type 検査** (DESIGN §4.2):
```rust
const CONCEALED_TYPES: &[&str] = &[
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
    "org.nspasteboard.AutoGeneratedType",
];
fn is_concealed(types: &[String]) -> bool {
    types.iter().any(|t| CONCEALED_TYPES.contains(&t.as_str()))
}
```

#### M3-win

```rust
// windows.rs
pub struct WinWatcher {
    hwnd: HWND,                          // メッセージ受信用の hidden window
    rx: tokio::sync::mpsc::Receiver<()>, // WM_CLIPBOARDUPDATE で push
    guard: Arc<SelfWriteGuard>,
}
```

- 起動時に hidden window を作成、`AddClipboardFormatListener(hwnd)` を呼ぶ。
- WindowProc で `WM_CLIPBOARDUPDATE` を捕捉して `mpsc` に push（GUI スレッドはブロックしない）。
- `next()` で `rx.recv().await` → `OpenClipboard` → `EnumClipboardFormats` → `safelist.rs` でフィルタ → 各フォーマットを HGLOBAL 経由で取得 → `CloseClipboard`。
- HGLOBAL 取り扱いは `mod hglobal` に閉じ込め、`unsafe` 局所化（DESIGN §5.2）。

**safelist** (DESIGN §5.2):
```rust
pub fn classify(format: u32, name: Option<&str>) -> FormatHandler {
    match format {
        CF_UNICODETEXT | CF_TEXT | CF_OEMTEXT => FormatHandler::SafeText,
        CF_DIB | CF_DIBV5 => FormatHandler::DibToPng,
        CF_BITMAP => FormatHandler::BitmapToPng,
        CF_HDROP => FormatHandler::FilePathsOnly,
        CF_TIFF => FormatHandler::TiffToPng,
        _ => match name {
            Some("HTML Format") | Some("Rich Text Format") => FormatHandler::SafeText,
            _ => FormatHandler::Unknown,  // v0.1 ではスキップ
        }
    }
}
```

**テスト**:
- mac: NSPasteboard モック (`MockPasteboard` trait) で changeCount 増加・concealed・self-write をシミュレート。
- win: HGLOBAL のサイズ 0 / null 等の異常系。
- 両 OS: テキスト→画像→テキストの連続コピーで取り損ねがないこと。

---

### M4: capture pipeline（2日）

`clipnotex-app` 内のオーケストレーション。

```rust
pub async fn run_capture_loop(
    mut watcher: Box<dyn ClipboardWatcher>,
    filter: Arc<ExclusionFilter>,
    store: Arc<StoreService>,
    bus: EventBus,
) -> Result<()> {
    while let Some(item) = watcher.next().await? {
        if filter.should_block(&item.source_app, &item.payloads) {
            continue;
        }
        let id = ClipItem::from(item).id;
        match store.add_item(/* ... */).await {
            Ok(_) => bus.emit(CoreEvent::ClipboardCaptured(id)),
            Err(e) => warn!(?e, "failed to store clip"),
        }
    }
    Ok(())
}
```

**M4 完了時に成立すること**: アプリを `cargo run` で起動 → コピー操作 → `tools/devcli history list` で履歴に入っていることを確認できる。

---

### M5: clipnotex-hotkey（1〜2日）

```rust
pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    bindings: HashMap<HotkeyId, HotKey>,
    bus: EventBus,
}
impl HotkeyService {
    pub fn register(&mut self, id: HotkeyId, accel: &str) -> Result<()> {
        let hk = HotKey::try_from(accel)?;
        self.manager.register(hk)?;          // ← 結果を必ずチェック (DESIGN §10.5)
        self.bindings.insert(id, hk);
        Ok(())
    }
    pub fn run(&self) {  // event-loop に hook
        // GlobalHotKeyEvent::receiver() を bus に転送
    }
}
```

**衝突検出**: `register` の `Result` を UI に返し、設定 UI で「このショートカットは使用できません」を表示。

---

### M6: clipnotex-paste（2日）

v0.1 では Stage A 退避方式の最小実装のみ（mac/win 両対応）。

```rust
pub async fn paste(item: &ClipItem, store: &StoreService, mode: PasteMode) -> Result<()> {
    let writer = clipboard_writer();
    let backup = writer.snapshot_for_restore()?;             // 1. 退避
    let payloads = match mode {
        PasteMode::Normal => store.load_payloads(item.id).await?,
        PasteMode::Plain  => vec![extract_text(item, store).await?],
    };
    writer.write(&payloads)?;                                // 2. 書込
    register_self_write(&item.digest);
    inject_paste_keystroke()?;                               // 3. Cmd+V / Ctrl+V
    tokio::time::sleep(Duration::from_millis(150)).await;
    writer.write(&backup)?;                                  // 4. 復元
    Ok(())
}

fn inject_paste_keystroke() -> Result<()> {
    let mut e = enigo::Enigo::new(&Settings::default())?;
    #[cfg(target_os = "macos")] e.key(Key::Meta,    Press)?;
    #[cfg(target_os = "windows")] e.key(Key::Control, Press)?;
    e.key(Key::Unicode('v'), Click)?;
    #[cfg(target_os = "macos")] e.key(Key::Meta,    Release)?;
    #[cfg(target_os = "windows")] e.key(Key::Control, Release)?;
    Ok(())
}
```

---

### M7: paste round-trip（1日）

**統合**: M5 で「履歴ポップアップ HK」を押したら、UI を出さず暫定的に `store.list_recent(1).first()` を取って M6 でペーストする CLI 動作確認モード (`--debug-paste-latest`) を作る。

**完了条件**: HK を押すと直近 1 件が貼られる。これでクリティカルパスが通る。

---

### M8: clipnotex-tauri + UI 履歴ポップアップ（4〜5日）

**Tauri commands**:

```rust
// crates/clipnotex-tauri/src/commands.rs
#[tauri::command]
pub async fn list_history(state: State<'_, AppState>, query: Option<String>, limit: usize)
    -> Result<Vec<ClipItemSummary>, String> { ... }

#[tauri::command]
pub async fn paste_item(state: State<'_, AppState>, id: String, mode: String)
    -> Result<(), String> { ... }

#[tauri::command]
pub async fn pin_toggle(...) -> ...;
#[tauri::command]
pub async fn delete_item(...) -> ...;
#[tauri::command]
pub async fn get_item_full(...) -> ...;   // プレビュー用
```

**UI コンポーネント** (`apps/desktop/src/features/history/`):

```
HistoryPopup.tsx          ルート (フォーカス管理)
  ├─ SearchBar.tsx         (key intercept)
  ├─ HistoryList.tsx       (react-virtuoso)
  │    └─ HistoryRow.tsx   (アイコン / プレビュー / アプリ名 / 時刻)
  └─ PreviewPane.tsx       (text or img)
hooks/
  ├─ useHistoryQuery.ts    (debounce 80ms + invoke)
  ├─ useKeyboardNav.ts     (↑↓, Enter, Cmd+Enter, Esc)
  └─ useFocusManagement.ts (IME 遅延アクティベーション §7.2)
```

**IME 対応 (DESIGN §7.2) の Tauri 実装**:

```rust
// src-tauri/src/main.rs
let win = tauri::WebviewWindowBuilder::new(app, "history", ...)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;

#[cfg(target_os = "macos")] {
    use cocoa::appkit::{NSWindow, NSWindowStyleMask, NSWindowCollectionBehavior};
    let ns_win = win.ns_window()? as id;
    unsafe {
        ns_win.setStyleMask_(NSWindowStyleMask::NSNonactivatingPanelMask
            | NSWindowStyleMask::NSResizableWindowMask);
        ns_win.setCollectionBehavior_(
            NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorTransient,
        );
    }
}
#[cfg(target_os = "windows")] {
    let hwnd = win.hwnd()?;
    unsafe {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE.0 as i32);
    }
}

// 検索バーで最初のキー入力時に呼ぶ Tauri command
#[tauri::command]
fn enable_input_focus(window: tauri::WebviewWindow) {
    #[cfg(target_os = "macos")] {
        let ns_win = window.ns_window().unwrap() as id;
        unsafe {
            let mut mask = ns_win.styleMask();
            mask.remove(NSWindowStyleMask::NSNonactivatingPanelMask);
            ns_win.setStyleMask_(mask);
            ns_win.makeKeyAndOrderFront_(nil);
        }
    }
}
```

**フォーカス保持の TS 側** (`useFocusManagement.ts`):
```ts
const onSearchKeydown = (e: KeyboardEvent) => {
  if (!hasActivated.current && e.key.length === 1) {
    invoke("enable_input_focus");      // 最初の文字入力で初めて key window 化
    hasActivated.current = true;
  }
  if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter") {
    e.preventDefault();
    bus.emit(e.key);                    // リスト側へ
  }
};
```

**完了条件**: HK でポップアップ → 検索 → 矢印で選択 → Enter で前面アプリにペースト → ポップアップが閉じてフォーカスが元アプリに戻る。

---

### M9: 画像対応（2日）

- mac: `NSPasteboardType.png` を取得、`primary_kind = Image`、サムネイル化。
- win: `CF_DIB`/`CF_DIBV5` を `BITMAPINFO` 解釈 → `image::ImageBuffer` 経由で PNG エンコード → 保存。
- サムネイル: `image::imageops::thumbnail(32, 32)` で生成、`thumbnails/<sha>.png` に未暗号化で保存（プレビュー画像のみで原本ではないため、機微度低と判断）。

---

### M10: 除外アプリ（2日）

```rust
// crates/clipnotex-app/src/exclusion.rs
pub struct ExclusionFilter {
    rules: ArcSwap<Vec<ExclusionRule>>,
    respect_concealed: bool,
}

impl ExclusionFilter {
    pub fn should_block(&self, src: &SourceApp, payloads: &[PayloadData]) -> bool {
        if self.respect_concealed && payloads.iter().any(|p| is_concealed_format(&p.format_id)) {
            return true;
        }
        let rules = self.rules.load();
        rules.iter().any(|r| r.matches(src))
    }
}
```

デフォルトルール:

```rust
fn default_rules() -> Vec<ExclusionRule> {
    vec![
        ExclusionRule::BundleId("com.1password.1password".into()),
        ExclusionRule::BundleId("com.1password.1password7".into()),
        ExclusionRule::BundleId("com.bitwarden.desktop".into()),
        ExclusionRule::BundleId("com.keepassxc.keepassxc".into()),
        ExclusionRule::ExeBasename { name: "1Password".into(), fuzzy: true },
        ExclusionRule::ExeBasename { name: "Bitwarden".into(), fuzzy: true },
        ExclusionRule::ExeBasename { name: "KeePassXC".into(), fuzzy: true },
    ]
}
```

---

### M11: Quota Manager（1〜2日）

```rust
pub struct QuotaManager {
    store: Arc<StoreService>,
    config: Arc<ArcSwap<HistoryConfig>>,
}

impl QuotaManager {
    pub async fn enforce(&self) -> Result<u64> {
        let cfg = self.config.load();
        let (count, bytes) = self.store.count_and_bytes().await?;
        let policy = match cfg.eviction_policy {
            Policy::CountPriority => evict_until(|c, _| c <= cfg.max_items),
            Policy::SizePriority  => evict_until(|_, b| b <= cfg.max_bytes),
        };
        self.store.evict(policy).await
    }
}
```

`StoreService::add_item` の最後に `quota.enforce()` を呼ぶ（同 tx ではなく別タスク発火）。

---

### M12〜M15: 設定 UI / トレイ / パッケージング / リリース

設計書 §6 v0.1 のチェックリストをそのまま実装すれば良い段階。詳細は実装着手時に別途。

---

## 2. OS 別の実装注意点（補足）

### 2.1 macOS

| 項目 | 注意 |
|---|---|
| Cocoa バインディング | **`objc2` + `objc2-app-kit`** を採用（古い `cocoa` クレートは非推奨方向）。`unsafe` は最小限。 |
| AppKit 初期化 | Tauri が `NSApplication` を起動するので、自前で `NSApplication.sharedApplication` を呼んではいけない。 |
| Pasteboard ポーリング | 100ms 固定で当面 OK。バッテリ気にする場合は `NSWorkspace` の `didActivateApplicationNotification` でポーリング再開・停止を制御。 |
| Notarize | `notarytool submit --wait`。Apple Developer ID 必要。OSS で誰でもビルドできるよう、署名は **リリースビルドのみ** で実施し、開発ビルドは ad-hoc 署名で OK。 |
| Accessibility 権限 | `enigo` でキー注入する時点で必要。初回は `AXIsProcessTrustedWithOptions(prompt: true)` で誘導 → 取得後に再起動を案内。 |
| Universal Binary | `cargo build --target aarch64-apple-darwin && --target x86_64-apple-darwin` → `lipo -create`。GitHub Actions の `macos-14` で両方ビルド可。 |

### 2.2 Windows

| 項目 | 注意 |
|---|---|
| `windows-rs` バージョン | 0.58 系を推奨。features は `Win32_System_DataExchange`, `Win32_UI_WindowsAndMessaging`, `Win32_System_Memory`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_System_Threading`. |
| Hidden message window | `RegisterClassExW` + `CreateWindowExW(HWND_MESSAGE)` で message-only window。WindowProc は thread_local 変数で受信器を持つ。 |
| HGLOBAL 取り扱い | 必ず `GlobalLock` → `GlobalSize` → `Vec` コピー → `GlobalUnlock`。`Drop` で確実に Unlock するため `struct GlobalLockGuard` を作り `unsafe` を局所化。 |
| 署名 | コード署名証明書が必要（個人 OSS は SignPath.io の OSS 無料枠を申請）。MSIX か MSI か → MSIX は配布が複雑なので **MSI + portable EXE** を v0.1 では選択。 |
| AutoStart | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` に登録するオプション。`auto_launch` crate が利用可。 |
| マニフェスト | `app.manifest` で `requestedExecutionLevel = asInvoker`、`dpiAware = PerMonitorV2`、`longPathAware = true`。 |

### 2.3 共通

- **シングルインスタンス**: `tauri-plugin-single-instance` を使う。2 度目の起動は履歴ポップアップを呼び出すだけにする。
- **ロギング**: `tracing` + `tracing-subscriber`。ログは `~/Library/Logs/ClipNoteX/` (mac) / `%LOCALAPPDATA%\ClipNoteX\logs\` (win) にローテート。
- **クラッシュレポート**: MVP は OFF。将来 opt-in で `sentry` 等を検討。

---

## 3. UI モック補足改善

### 3.1 履歴ポップアップ — 詳細レイアウト（実装向け）

```
┌──────────────────────────────────────────────────────────────────────┐
│ 🔍 ┃ Search clipboard...                                  ⚙  ─  ✕  │  ← 32px
├──────────────────────────────────────────────────────────────────────┤
│ ┌──────────────────────────────┐ ┌────────────────────────────────┐ │
│ │ List (40% width, virtual)    │ │ Preview (60%, lazy)            │ │
│ │ ┌──────────────────────────┐ │ │ ┌────────────────────────────┐ │ │
│ │ │📌 📝 TODO refactor...   │ │ │ │ {                          │ │ │
│ │ │   Slack  ·  14:32       │ │ │ │   "name": "foo",           │ │ │
│ │ ├──────────────────────────┤ │ │ │   "value": 42              │ │ │
│ │ │   🖼 [thumb 32x32]      │ │ │ │ }                          │ │ │
│ │ │   Figma  ·  14:30       │ │ │ │                            │ │ │
│ │ ├──────────────────────────┤ │ │ │ format: json (detected)    │ │ │
│ │ │   🌐 <table><tr>...    │ │ │ │ size: 42 B  · from VSCode  │ │ │
│ │ │   Notion · 14:28        │ │ │ └────────────────────────────┘ │ │
│ │ └──────────────────────────┘ │ │                                │ │
│ └──────────────────────────────┘ └────────────────────────────────┘ │
├──────────────────────────────────────────────────────────────────────┤
│ ↵ paste  ⌘↵ plain  ⌥↵ format  ⇧↵ full  ⌘P pin  ⌘⌫ del   3/142 │  ← 24px
└──────────────────────────────────────────────────────────────────────┘
```

- **デフォルトサイズ**: 720 × 480 px、リサイズ可・最小 480 × 320。
- **最終行ステータスバー**: 利用可能なキーバインドと選択位置を常時表示（学習コスト削減）。
- **ダーク/ライト**: OS テーマに追従（`prefers-color-scheme`）。
- **言語バッジ**: 行に小さく `json` 等を表示するのは v0.3 (整形ペースト導入時)。v0.1 はアイコンのみ。
- **空状態**: 履歴 0 件のとき「Cmd+C で何かをコピーすると、ここに表示されます」のヒント。
- **検索ヒット強調**: `<mark>` でマッチ部分にハイライト。
- **カラーパレット**: Tailwind の neutral + accent (blue-500)。アクセントカラーは設定で変更可（v0.4+）。

### 3.2 トレイメニュー（v0.1）

```
ClipNoteX
─────────────────────
Show History    ⌘⇧V
Show Snippets   ⌘⇧C    (disabled in v0.1)
─────────────────────
Pause Capture
Clear All History...
─────────────────────
Settings...
About ClipNoteX
─────────────────────
Quit            ⌘Q
```

---

## 4. クリティカルなコードスケルトン

実ファイルとして配置済み:
- `Cargo.toml` (workspace)
- `rust-toolchain.toml`
- `crates/*/Cargo.toml` と `src/lib.rs` (各 crate の最小スケルトン)
- `.github/workflows/ci.yml`
- `.github/ISSUE_TEMPLATE/formatter_request.yml`
- `.gitignore`

`apps/desktop/` は `pnpm create tauri-app --template react-ts` で生成する想定（雛形は対話式のためここでは作らず、コマンド 1 行で行う）。

---

## 6. 懸念点対応マトリクス（Rev 2 追加）

指摘された 8 点の懸念について、「どのファイルで対処したか / 何が残っているか」を一覧化する。

| # | 懸念 | 対処済み (コード) | 残作業 |
|---|---|---|---|
| 1 | macOS `nonactivatingPanel` で IME が動かない | `macos.rs` — 遅延アクティベーション stub + TODO(M3-mac) / `commands.rs` — `enable_input_focus` command / `global.css` — UI は CSS 完備 | M8 で `enable_input_focus` の objc2 実装と `useFocusManagement.ts` の実装 |
| 2 | Windows HGLOBAL の 1 バイトミスでクラッシュ | **`hglobal.rs`** — GlobalLock NULL / GlobalSize 0 / biSize 検証 / Guard drop / `dib_to_png` / `hdrop_paths` 全実装 + テスト | M3-win で `windows.rs` からこの module を呼び出す |
| 3 | macOS `data(forType:)` の遅延読込とレースコンディション | **`macos.rs`** — `spawn_blocking` 内で単一スナップショット順次読込 / **changeCount 前後比較で RaceRetry を返す** ロジック実装 | M3-mac で stub を実 objc2 API に置換 |
| 4 | global-hotkey 登録失敗 (VSCode / IME が Cmd+Shift+V を奪う) | **`clipnotex-hotkey/src/lib.rs`** — `RegistrationResult` + `is_conflict` フラグ / `register_all` で全件収集 | M8 で設定 UI に失敗を表示 / 代替ショートカット提案 UI |
| 5 | Windows KEYEVENTF_UNICODE が IME に吸われる | **`paste/src/ime.rs`** — `ImmGetContext` / `ImmGetOpenStatus` で IME ON 検出 + **`lib.rs`** — `paste_stage_b_text` で IME ON なら keystroke スキップし Stage A 降格 | v0.4 で Stage B を本実装するときに `try_direct_inject` を実装 |
| 6 | DIB→PNG→サムネイル生成が CPU 重くて UI を固める | **`thumbnail.rs`** — bounded mpsc channel (64) + `spawn_blocking` + 原子的ファイル書込 + `ThumbnailState::Pending` / `global.css` — shimmer placeholder CSS | M8 で UI 側が `ThumbnailState` を受け取って placeholder を表示 / `CoreEvent::ThumbnailReady` の emit |
| 7 | redb 長時間トランザクションでデッドロック | `store.rs` — blob 書込は tx 外 / 50ms 超過で warn / `capture.rs` — `spawn_blocking` 内で完結 / `quota.rs` — eviction を独立呼出 | M11 で evict を実装するときに同規律を守る |
| 8 | Windows WebView2 で ClearType により bold が潰れる | **`global.css`** — `-webkit-font-smoothing: antialiased` / `text-rendering: optimizeLegibility` / JetBrains Mono `@font-face` / システムフォントスタック明示 | `apps/desktop/public/fonts/` に JetBrains Mono woff2 を配置 (M8 前) |

### 懸念点対応の実装優先度

クリティカルパス (M0〜M7) に直撃する懸念は **1, 2, 3, 7**。これらは M3 着手前に対処済みの状態。

| 緊急度 | 懸念 | 対応フェーズ |
|---|---|---|
| 🔴 今すぐ (M3) | 2 HGLOBAL / 3 changeCount レース | hglobal.rs + macos.rs の stub をそのまま実 API で埋める |
| 🟠 M8 前 | 1 IME (mac) / 8 WebView フォント | enable_input_focus 実装 + woff2 配置 |
| 🟡 M8 並行 | 6 サムネイル / 4 HK 衝突 UI | thumbnail worker の emit / HK 失敗を設定 UI に表示 |
| 🟢 v0.4 | 5 Windows IME + Stage B | ime.rs の try_direct_inject 実装 |

---

## 5. テスト戦略

### 5.1 単体テスト（cargo test, 各 crate 内）

- `clipnotex-store`: AEAD round-trip / redb 書込読込 / eviction（既存 / 新規 / pinned 保護）
- `clipnotex-clipboard`: モック pasteboard / safelist 分類 / self-write guard の TTL
- `clipnotex-app`: ExclusionFilter のマッチ規則（bundle_id / fuzzy / glob）

### 5.2 結合テスト

`tests/` にトップレベルで配置。実 OS のクリップボードを使うため `#[ignore]` をデフォルトに、`cargo test -- --ignored` で実行。

```rust
#[test] #[ignore]
fn end_to_end_text() {
    let app = TestApp::spawn();
    app.set_os_clipboard("hello").unwrap();
    app.wait_for_capture(Duration::from_secs(2)).unwrap();
    app.trigger_hotkey(HotkeyId::ShowHistory);
    app.assert_history_first("hello");
}
```

### 5.3 手動 QA チェックリスト（v0.1 リリース前）

- [ ] mac: Slack / VSCode / Chrome / Finder / Terminal でコピー → 履歴に入る
- [ ] win: Slack / VSCode / Chrome / Explorer / メモ帳 でコピー → 履歴に入る
- [ ] 1Password (mac/win) でパスワードコピー → 履歴に入らない
- [ ] Bitwarden で同上
- [ ] 大きいテキスト (10MB) コピー → 履歴に入る・UI が固まらない
- [ ] 連続コピー (50 回 / 5 秒) → 取り損ねなし、重複は 1 件にまとまる
- [ ] HK 衝突時に設定 UI で警告表示
- [ ] mac: 日本語 IME で検索バーが動く（遅延アクティベーション）
- [ ] mac: ペースト後にフォーカスが元アプリに戻る
- [ ] PNG コピー → 履歴に入る → ペーストして元アプリで貼れる
- [ ] 件数上限 / 容量上限の eviction が両ポリシーで効く
- [ ] アプリ再起動後も履歴と設定が復元される
- [ ] OS 再起動後 (auto-start 有効時) にトレイに自動常駐

---

## 6. v0.1 の現実的なスケジュール感（目安）

1 人フルタイムで約 6〜8 週間、週末プロジェクトだと 4〜6 ヶ月。

| 週 | マイルストーン |
|---|---|
| 1   | M0 + M1 |
| 2   | M2 |
| 3〜4 | M3 (mac/win 並行) |
| 5   | M4 + M5 + M6 + **M7 貫通** ← 最重要マイルストーン |
| 6   | M8 (UI ポップアップ + IME) |
| 7   | M9 + M10 + M11 |
| 8   | M12 + M13 + M14 + M15 |

M7 (paste round-trip) を目標に据えて先に通すのが、開発のモチベーションと設計検証の両面で効く。

---

*本書は実装の進行に応じて更新する生きたドキュメント。各マイルストーン完了時に「実装で気付いたズレ」を DESIGN.md と本書に反映する。*
