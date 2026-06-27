use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Notify, RwLock};

use crate::clipboard::backend;
use crate::clipboard::models::EntryMeta;
use crate::clipboard::store::SharedStore;
use crate::config::Config;
use crate::protocol::{DaemonRequest, DaemonResponse};
use crate::search::LauncherItem;

const CLIPBOARD_REFRESH_INTERVAL_MS: u64 = 500;
const LAUNCHER_REFRESH_INTERVAL_MS: u64 = 10_000;

type SharedClipboardEntries = Arc<RwLock<Vec<EntryMeta>>>;
type SharedLauncherEntries = Arc<RwLock<Vec<LauncherItem>>>;

pub fn run(rt: tokio::runtime::Runtime) {
    let cfg = Config::load();
    if let Err(err) = rt.block_on(run_async(cfg.clipboard.history_limit)) {
        eprintln!("[daemon] {err}");
    }
}

async fn run_async(history_limit: usize) -> anyhow::Result<()> {
    let store = backend::open_store().map_err(anyhow::Error::msg)?;
    let clipboard_notify = Arc::new(Notify::new());
    backend::spawn_watcher(Arc::clone(&store), Arc::clone(&clipboard_notify));

    let clipboard_entries: SharedClipboardEntries = Arc::new(RwLock::new(Vec::new()));
    let bumped_entries: SharedClipboardEntries = Arc::new(RwLock::new(Vec::new()));
    let launcher_entries: SharedLauncherEntries = Arc::new(RwLock::new(Vec::new()));
    let _ = refresh_clipboard_entries(&store, &clipboard_entries, history_limit).await;
    let _ = refresh_launcher_entries(&launcher_entries).await;

    let socket = socket_path()?;
    if tokio::net::UnixStream::connect(&socket).await.is_ok() {
        return Ok(());
    }
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    }

    let refresh_clipboard_store = Arc::clone(&store);
    let refresh_clipboard_state = Arc::clone(&clipboard_entries);
    let refresh_notify = Arc::clone(&clipboard_notify);
    tokio::spawn(async move {
        let mut last_error = None;
        loop {
            match refresh_clipboard_entries(
                &refresh_clipboard_store,
                &refresh_clipboard_state,
                history_limit,
            )
            .await
            {
                Ok(()) => last_error = None,
                Err(err) => {
                    let should_log = last_error.as_deref() != Some(err.as_str());
                    if should_log {
                        eprintln!("[daemon] clipboard refresh failed: {err}");
                    }
                    last_error = Some(err);
                }
            }
            tokio::select! {
                () = refresh_notify.notified() => {},
                () = tokio::time::sleep(std::time::Duration::from_millis(CLIPBOARD_REFRESH_INTERVAL_MS)) => {},
            }
        }
    });

    let refresh_launcher_state = Arc::clone(&launcher_entries);
    tokio::spawn(async move {
        loop {
            let _ = refresh_launcher_entries(&refresh_launcher_state).await;
            tokio::time::sleep(std::time::Duration::from_millis(
                LAUNCHER_REFRESH_INTERVAL_MS,
            ))
            .await;
        }
    });

    loop {
        let (stream, _) = listener.accept().await?;
        let store = Arc::clone(&store);
        let clipboard_entries = Arc::clone(&clipboard_entries);
        let bumped_entries = Arc::clone(&bumped_entries);
        let launcher_entries = Arc::clone(&launcher_entries);
        tokio::spawn(async move {
            let _ = handle_connection(
                stream,
                store,
                clipboard_entries,
                bumped_entries,
                launcher_entries,
            )
            .await;
        });
    }
}

async fn refresh_clipboard_entries(
    store: &SharedStore,
    entries: &SharedClipboardEntries,
    history_limit: usize,
) -> Result<(), String> {
    let history = backend::load_clipboard_history(store, history_limit)?;
    *entries.write().await = history;
    Ok(())
}

async fn refresh_launcher_entries(entries: &SharedLauncherEntries) -> Result<(), String> {
    *entries.write().await = crate::modes::launcher::load_items();
    Ok(())
}

async fn handle_connection(
    mut stream: UnixStream,
    store: SharedStore,
    clipboard_entries: SharedClipboardEntries,
    bumped_entries: SharedClipboardEntries,
    launcher_entries: SharedLauncherEntries,
) -> anyhow::Result<()> {
    loop {
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(err) => return Err(err.into()),
        }

        let len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).await?;

        let req: DaemonRequest = postcard::from_bytes(&body)?;
        let response = dispatch(
            req,
            &store,
            &clipboard_entries,
            &bumped_entries,
            &launcher_entries,
        )
        .await;
        let encoded = postcard::to_allocvec(&response)?;
        let resp_len = (encoded.len() as u32).to_le_bytes();
        stream.write_all(&resp_len).await?;
        stream.write_all(&encoded).await?;
    }
}

async fn dispatch(
    req: DaemonRequest,
    store: &SharedStore,
    clipboard_entries: &SharedClipboardEntries,
    bumped_entries: &SharedClipboardEntries,
    launcher_entries: &SharedLauncherEntries,
) -> DaemonResponse {
    match req {
        DaemonRequest::Ping => DaemonResponse::Pong,
        DaemonRequest::GetClipboardHistory { limit } => {
            let bumped = bumped_entries.read().await;
            let entries = clipboard_entries.read().await;
            let mut by_id: std::collections::HashMap<u64, EntryMeta> =
                entries.iter().map(|e| (e.id, e.clone())).collect();
            for b in bumped.iter() {
                by_id.insert(b.id, b.clone());
            }
            let mut merged: Vec<EntryMeta> = by_id.into_values().collect();
            merged.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            DaemonResponse::ClipboardHistory(merged.into_iter().take(limit).collect())
        }
        DaemonRequest::GetClipboardContent { id } => match store.get_by_id(id) {
            Ok(Some(entry)) => DaemonResponse::ClipboardContent(entry.full_content()),
            Ok(None) => DaemonResponse::Error(format!("entry {id} not found")),
            Err(e) => DaemonResponse::Error(e.to_string()),
        },
        DaemonRequest::GetLauncherItems => {
            let entries = launcher_entries.read().await;
            DaemonResponse::LauncherItems(entries.clone())
        }
        DaemonRequest::PasteClipboard { id } => {
            match backend::paste_clipboard(Arc::clone(store), id).await {
                Ok(()) => DaemonResponse::ClipboardPasted,
                Err(err) => DaemonResponse::Error(err.clone()),
            }
        }
        DaemonRequest::BumpClipboardEntry { id } => {
            let mut bumped = bumped_entries.write().await;
            let entries = clipboard_entries.read().await;

            let target = bumped
                .iter()
                .find(|e| e.id == id)
                .cloned()
                .or_else(|| entries.iter().find(|e| e.id == id).cloned());

            if let Some(mut entry) = target {
                entry.timestamp = crate::clipboard::models::now_micros();
                bumped.retain(|e| e.id != id);
                bumped.insert(0, entry);
                DaemonResponse::ClipboardBumped
            } else {
                DaemonResponse::Error(format!("entry {id} not found"))
            }
        }
    }
}

fn socket_path() -> anyhow::Result<std::path::PathBuf> {
    let base = dirs::runtime_dir().ok_or_else(|| anyhow::anyhow!("XDG_RUNTIME_DIR not set"))?;
    Ok(base.join("luncher.sock"))
}
