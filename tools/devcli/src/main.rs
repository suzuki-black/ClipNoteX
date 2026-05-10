use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let mut args = std::env::args().skip(1);
    let cmd = args
        .next()
        .ok_or_else(|| anyhow!("usage: clipnotex-cli <history|paste-latest|pack|verify> ..."))?;
    let data_dir = data_dir_from_env()?;
    match cmd.as_str() {
        "history" => match args.next().as_deref() {
            Some("count") => history_count(data_dir),
            Some("list") => {
                let limit: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);
                let query = args.next();
                history_list(data_dir, limit, query.as_deref())
            }
            _ => Err(anyhow!("history subcommands: count | list [limit] [query]")),
        },
        "paste-latest" => paste_latest(data_dir).await,
        "pack" => Err(anyhow!("pack subcommand is reserved for v0.2")),
        other => Err(anyhow!("unknown command: {other}")),
    }
}

fn data_dir_from_env() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("CLIPNOTEX_DATA_DIR") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs_next()?;
    Ok(base.join("ClipNoteX"))
}

fn dirs_next() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home).join("Library").join("Application Support"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("LOCALAPPDATA")?;
        Ok(PathBuf::from(appdata))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home).join(".local").join("share"))
    }
}

fn open_store(data_dir: PathBuf) -> Result<clipnotex_store::StoreService> {
    use clipnotex_store::KeySource;
    Ok(clipnotex_store::StoreService::open(
        data_dir,
        KeySource::Keyring {
            service: "ClipNoteX".into(),
            account: "data_key".into(),
        },
    )?)
}

fn history_count(data_dir: PathBuf) -> Result<()> {
    let svc = open_store(data_dir)?;
    let (n, b) = svc.count_and_bytes()?;
    println!("items: {n}, bytes: {b}");
    Ok(())
}

fn history_list(data_dir: PathBuf, limit: usize, query: Option<&str>) -> Result<()> {
    let svc = open_store(data_dir)?;
    let items = svc.list_recent(limit, query)?;
    if items.is_empty() {
        println!("(no items)");
        return Ok(());
    }
    for item in &items {
        let kind = format!("{:?}", item.primary_kind);
        let preview = item
            .text_preview
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>();
        let app = &item.source_app.display_name;
        println!("[{}] {} | {} | {:?}", item.id, kind, app, preview);
    }
    Ok(())
}

/// Paste the most recent clipboard item via Stage A (snapshot→write→Cmd/Ctrl+V→restore).
async fn paste_latest(data_dir: PathBuf) -> Result<()> {
    use clipnotex_clipboard::SelfWriteGuard;
    use clipnotex_core::model::{PayloadData, PayloadStorage};
    use clipnotex_paste::{PasteController, PasteMode};
    use std::sync::Arc;

    let svc = open_store(data_dir)?;
    let items = svc.list_recent(1, None)?;
    let item = items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no items in history"))?;

    println!(
        "pasting: [{:?}] {}",
        item.primary_kind,
        item.text_preview.as_deref().unwrap_or("(binary)")
    );

    // Open the OS clipboard writer.
    let guard = Arc::new(SelfWriteGuard::new(std::time::Duration::from_secs(5)));
    let (_watcher, writer) = clipnotex_clipboard::open(guard.clone())?;
    let writer: Arc<dyn clipnotex_clipboard::ClipboardWriter> = Arc::from(writer);

    let controller = PasteController::new(writer, guard);

    // v0.1 stores preview only (payloads are empty inline). Fall back to
    // synthesizing a plain-text payload from text_preview.
    let payloads: Vec<PayloadData> = if item.payloads.is_empty() {
        let text = item
            .text_preview
            .ok_or_else(|| anyhow!("item has no payload and no text_preview"))?;
        vec![PayloadData {
            format_id: "public.utf8-plain-text".into(),
            bytes: text.into_bytes(),
        }]
    } else {
        item.payloads
            .into_iter()
            .filter_map(|p| match p.storage {
                PayloadStorage::Inline(bytes) => Some(PayloadData {
                    format_id: p.format_id,
                    bytes,
                }),
                // Blob/Pack not yet wired — skip.
                _ => None,
            })
            .collect()
    };

    controller
        .paste(payloads, item.digest, PasteMode::Normal)
        .await?;
    println!("paste complete");
    Ok(())
}
