use crate::clipboard::client;
use crate::protocol::{DaemonRequest, DaemonResponse};
use crate::search::LauncherItem;

pub async fn load_items() -> Result<Vec<LauncherItem>, String> {
    client::ensure_daemon().await?;
    match client::request(DaemonRequest::GetLauncherItems).await? {
        DaemonResponse::LauncherItems(items) => Ok(items),
        DaemonResponse::Error(err) => Err(err),
        other => Err(format!("unexpected response: {other:?}")),
    }
}
