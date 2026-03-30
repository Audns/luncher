use serde::{Deserialize, Serialize};

use crate::clipboard::models::EntryMeta;
use crate::search::LauncherItem;

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    GetClipboardHistory { limit: usize },
    GetLauncherItems,
    PasteClipboard { id: u64 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    Pong,
    ClipboardHistory(Vec<EntryMeta>),
    LauncherItems(Vec<LauncherItem>),
    ClipboardPasted,
    Error(String),
}
