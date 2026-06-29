use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bytes::Bytes;
use tracing::{info, warn};
use wl_clipboard_rs::paste::{ClipboardType, Error as PasteError, MimeType, Seat, get_contents};

use crate::clipboard::models::ClipboardEntry;
use crate::clipboard::store::{ClipboardRow, SharedStore, hex_encode};

const POLL_INTERVAL: Duration = Duration::from_millis(500);

const SENSITIVE_MIMES: &[&str] = &[
    "x-kde-passwordManagerHint",
    "application/x-kde-passwordmanager",
    "org.freedesktop.secret",
];

/// MIME types that carry a URI list (per the W3C URI list spec, or its KDE / GNOME analogues).
/// `normalize_entry` recognizes any of these and rewrites the stored mime to `text/uri-list`.
const URI_LIST_MIMES: &[&str] = &[
    "text/uri-list",
    "application/x-kde-uri-list",
    "x-special/gnome-copied-files",
];

/// Async watcher loop. Spawn with `tokio::spawn` from the daemon.
pub async fn run_watcher(store: SharedStore, notify: Arc<tokio::sync::Notify>) -> Result<()> {
    info!(
        "watching clipboard (polling wl-clipboard-rs every {}ms)",
        POLL_INTERVAL.as_millis()
    );

    loop {
        match poll_once() {
            Ok(Some(entry)) => {
                let row: ClipboardRow = (&entry).into();
                match store.insert(&row).await {
                    Ok(true) => {
                        info!(
                            "clipboard: stored {} bytes of {}",
                            row.data.len(),
                            row.mime_type
                        );
                        notify.notify_one();
                    }
                    Ok(false) => {}
                    Err(err) => warn!("store insert error: {err}"),
                }
            }
            Ok(None) => {}
            Err(err) => warn!("clipboard poll failed: {err:#}"),
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
fn poll_once() -> Result<Option<ClipboardEntry>> {
    let (mut pipe, mime) =
        match get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Any) {
            Ok(pair) => pair,
            Err(PasteError::ClipboardEmpty | PasteError::NoMimeType | PasteError::NoSeats) => {
                return Ok(None);
            }
            Err(e) => return Err(anyhow::anyhow!("wl-clipboard-rs get_contents: {e}")),
        };

    let mut data = Vec::with_capacity(4096);
    pipe.read_to_end(&mut data)
        .context("reading clipboard pipe")?;
    while data.last() == Some(&0) {
        data.pop();
    }
    if data.is_empty() {
        return Ok(None);
    }

    let sensitive = SENSITIVE_MIMES.iter().any(|s| mime == *s);
    let (mime, data, filename) = normalize_entry(mime, data);
    Ok(Some(ClipboardEntry::with_filename(
        mime,
        Bytes::from(data),
        sensitive,
        Bytes::new(),
        filename,
    )))
}

fn normalize_entry(mime: String, data: Vec<u8>) -> (String, Vec<u8>, Option<String>) {
    let text_content = String::from_utf8_lossy(&data);
    let is_file_uri = text_content.starts_with("file://");
    let is_absolute_path =
        text_content.starts_with('/') && !text_content.contains('\n') && text_content.len() < 4096;

    let looks_like_uri_list =
        URI_LIST_MIMES.iter().any(|m| mime == *m) || is_file_uri || is_absolute_path;

    if looks_like_uri_list {
        let uri_data = if is_absolute_path {
            format!("file://{}", text_content.trim()).into_bytes()
        } else {
            data.clone()
        };

        return match uri_list_filename(&uri_data) {
            Ok(name) => ("text/uri-list".to_string(), uri_data, Some(name)),
            Err(_) => (mime, data, None),
        };
    }

    (mime, data, None)
}

fn uri_list_filename(data: &[u8]) -> Result<String> {
    let uri_str = String::from_utf8_lossy(data);
    let uri = uri_str
        .lines()
        .find(|line| line.starts_with("file://"))
        .ok_or_else(|| anyhow::anyhow!("no file:// URI found"))?;

    let path = uri
        .strip_prefix("file://")
        .ok_or_else(|| anyhow::anyhow!("invalid file:// URI"))?;

    let decoded = if path.starts_with('/') {
        percent_decode(path)
    } else {
        path.to_string()
    };

    std::path::Path::new(&decoded)
        .file_name()
        .and_then(|name| name.to_str())
        .map(std::string::ToString::to_string)
        .context("file uri has no filename")
}

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                bytes.push(byte);
                continue;
            }
            bytes.push(b'%');
            bytes.extend(hex.bytes());
        } else {
            let mut buf = [0; 4];
            bytes.extend(ch.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

impl From<&ClipboardEntry> for ClipboardRow {
    fn from(entry: &ClipboardEntry) -> Self {
        ClipboardRow::new(
            entry.timestamp,
            entry.mime_type.clone(),
            entry.kind,
            entry.data.clone(),
            entry.thumb.clone(),
            entry.hash,
            hex_encode(&entry.hash),
            entry.sensitive,
            entry.filename.clone(),
        )
    }
}
