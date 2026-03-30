use std::path::PathBuf;

use anyhow::Context;
use wl_clipboard_rs::copy::{MimeType, Options, Source};

use crate::clipboard::models::EntryMeta;
use crate::clipboard::store::{SharedStore, Store};

pub fn open_store() -> Result<SharedStore, String> {
    let path = db_path().map_err(|err| err.to_string())?;
    Store::open(&path)
        .map(std::sync::Arc::new)
        .map_err(|err| err.to_string())
}

pub fn spawn_watcher(store: SharedStore) {
    crate::clipboard::watcher::spawn_watcher(store);
}

pub fn load_clipboard_history(store: &SharedStore, limit: usize) -> Result<Vec<EntryMeta>, String> {
    store
        .get_recent(limit)
        .map(|entries| entries.iter().map(EntryMeta::from).collect())
        .map_err(|err| err.to_string())
}

pub async fn paste_clipboard(store: SharedStore, id: u64) -> Result<(), String> {
    let entry = store
        .get_by_id(id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("entry {id} not found"))?;

    let data = entry.data.clone();
    let mime = entry.mime_type.clone();
    tokio::task::spawn_blocking(move || {
        let options = Options::new();
        options.copy(Source::Bytes(data.to_vec().into()), MimeType::Specific(mime))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

fn db_path() -> anyhow::Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("XDG_DATA_HOME not set"))?;
    let legacy = base.join("clipbowl").join("history.redb");
    if legacy.exists() {
        return Ok(legacy);
    }

    let path = base.join("luncher").join("history.redb");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    Ok(path)
}
