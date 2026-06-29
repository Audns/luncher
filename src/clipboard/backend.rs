use wl_clipboard_rs::copy::{MimeType, Options, Source};

use crate::clipboard::models::EntryMeta;
use crate::clipboard::store::{AirStore, SharedStore, db_path};

pub async fn open_store() -> Result<SharedStore, String> {
    let path = db_path()?;
    AirStore::open(&path).await
}

pub async fn load_clipboard_history(
    store: &SharedStore,
    limit: usize,
) -> Result<Vec<EntryMeta>, String> {
    store
        .get_recent(limit)
        .await
        .map(|entries| entries.iter().map(EntryMeta::from).collect())
        .map_err(|err| err.clone())
}

pub async fn paste_clipboard(store: SharedStore, id: u64) -> Result<(), String> {
    let entry = store
        .get_by_id(id)
        .await
        .map_err(|err| err.clone())?
        .ok_or_else(|| format!("entry {id} not found"))?;

    let data = entry.data.clone();
    let mime = entry.mime_type.clone();
    tokio::task::spawn_blocking(move || {
        let options = Options::new();
        options.copy(
            Source::Bytes(data.to_vec().into()),
            MimeType::Specific(mime),
        )
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}
