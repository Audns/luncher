use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::clipboard::models::EntryMeta;
use crate::protocol::{DaemonRequest, DaemonResponse};

const DAEMON_WAIT_ATTEMPTS: usize = 20;
const DAEMON_WAIT_STEP_MS: u64 = 50;

pub async fn ensure_daemon() -> Result<(), String> {
    if ping().await.is_ok() {
        return Ok(());
    }

    let _ = spawn_daemon_process();

    for _ in 0..DAEMON_WAIT_ATTEMPTS {
        if ping().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(DAEMON_WAIT_STEP_MS)).await;
    }

    let socket = socket_path().map_err(|err| err.to_string())?;
    Err(format!("connecting to daemon at {}", socket.display()))
}

pub async fn get_clipboard_history(limit: usize) -> Result<Vec<EntryMeta>, String> {
    ensure_daemon().await?;
    match request(DaemonRequest::GetClipboardHistory { limit }).await? {
        DaemonResponse::ClipboardHistory(entries) => Ok(entries),
        DaemonResponse::Error(err) => Err(err),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

pub async fn paste_clipboard(id: u64) -> Result<(), String> {
    ensure_daemon().await?;
    match request(DaemonRequest::PasteClipboard { id }).await? {
        DaemonResponse::ClipboardPasted => Ok(()),
        DaemonResponse::Error(err) => Err(err),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

pub async fn get_clipboard_content(id: u64) -> Result<String, String> {
    ensure_daemon().await?;
    match request(DaemonRequest::GetClipboardContent { id }).await? {
        DaemonResponse::ClipboardContent(content) => Ok(content),
        DaemonResponse::Error(err) => Err(err),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

async fn ping() -> Result<(), String> {
    match request(DaemonRequest::Ping).await? {
        DaemonResponse::Pong => Ok(()),
        DaemonResponse::Error(err) => Err(err),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

pub async fn request(req: DaemonRequest) -> Result<DaemonResponse, String> {
    let socket = socket_path().map_err(|err| err.to_string())?;
    let mut stream = UnixStream::connect(&socket)
        .await
        .map_err(|err| format!("connecting to daemon at {}: {err}", socket.display()))?;

    let encoded = postcard::to_allocvec(&req).map_err(|err| err.to_string())?;
    let len = (encoded.len() as u32).to_le_bytes();
    stream.write_all(&len).await.map_err(|err| err.to_string())?;
    stream
        .write_all(&encoded)
        .await
        .map_err(|err| err.to_string())?;

    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|err| err.to_string())?;
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; resp_len];
    stream
        .read_exact(&mut body)
        .await
        .map_err(|err| err.to_string())?;

    postcard::from_bytes(&body).map_err(|err| err.to_string())
}

fn socket_path() -> anyhow::Result<PathBuf> {
    let base = dirs::runtime_dir().ok_or_else(|| anyhow::anyhow!("XDG_RUNTIME_DIR not set"))?;
    Ok(base.join("luncher.sock"))
}

fn spawn_daemon_process() -> std::io::Result<std::process::Child> {
    let mut last_err = None;

    for candidate in daemon_candidates() {
        let mut cmd = std::process::Command::new(&candidate);
        match cmd
            .arg("--daemon")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(err) => last_err = Some(err),
        }
    }

    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "luncher daemon not found")
    }))
}

fn daemon_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        candidates.push(exe);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("target/debug/luncher"));
    candidates.push(manifest_dir.join("target/release/luncher"));
    candidates.push(PathBuf::from("luncher"));
    candidates
}
