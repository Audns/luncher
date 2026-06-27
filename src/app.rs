use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use std::time::Duration;
use wayland_client::{Connection, globals::registry_queue_init};

use crate::{
    protocol::{DaemonRequest, DaemonResponse},
    search::LauncherItem,
    state::{AppState, BackgroundUpdate},
};

#[derive(Clone, Copy)]
pub enum RemoteSource {
    Clipboard,
    Launcher,
}

impl RemoteSource {
    fn refresh_interval(self) -> Duration {
        match self {
            Self::Clipboard => Duration::from_millis(50),
            Self::Launcher => Duration::from_secs(5),
        }
    }
}

pub fn run(
    items: Vec<LauncherItem>,
    dmenu_mode: bool,
    clipboard_mode: bool,
    remote_source: Option<RemoteSource>,
    remote_handle: Option<tokio::runtime::Handle>,
    remote_runtime: Option<tokio::runtime::Runtime>,
    mode: String,
) {
    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<AppState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let cfg = crate::config::Config::load();

    if let (Some(source), Some(handle)) = (remote_source, remote_handle) {
        let (tx, rx) = calloop::channel::channel();
        loop_handle
            .insert_source(rx, |event, (), app| {
                if let calloop::channel::Event::Msg(update) = event {
                    match update {
                        BackgroundUpdate::Items(items) => {
                            app.last_background_error = None;
                            if app.search.replace_items(items) {
                                app.needs_redraw = true;
                            }
                        }
                        BackgroundUpdate::Error(err) => {
                            let should_log =
                                app.last_background_error.as_deref() != Some(err.as_str());
                            app.last_background_error = Some(err.clone());
                            if should_log {
                                eprintln!("[daemon] refresh failed: {err}");
                            }
                        }
                    }
                }
            })
            .unwrap();
        spawn_remote_refresh_worker(source, handle, tx);
    }

    let mut app = AppState::new(
        &globals,
        &qh,
        loop_handle,
        items,
        dmenu_mode,
        clipboard_mode,
        cfg.case_sensitive,
        mode,
    );

    let _remote_runtime = remote_runtime;

    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .unwrap();

    loop {
        event_loop
            .dispatch(Some(Duration::from_millis(16)), &mut app)
            .unwrap();
        if app.exit {
            break;
        }
        app.search.tick();
        if app.needs_redraw && app.configured {
            app.draw(&app.qh.clone());
        }
    }

    use std::io::Write;
    std::io::stdout().flush().ok();
    drop(app);
}

fn spawn_remote_refresh_worker(
    source: RemoteSource,
    handle: tokio::runtime::Handle,
    tx: calloop::channel::Sender<BackgroundUpdate>,
) {
    handle.spawn(async move {
        if crate::clipboard::client::ensure_daemon().await.is_err() {
            return;
        }
        let socket = match crate::clipboard::client::socket_path() {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut stream = match tokio::net::UnixStream::connect(&socket).await {
            Ok(s) => s,
            Err(_) => return,
        };

        let history_limit = crate::config::Config::load().clipboard.history_limit;
        loop {
            let resp = match source {
                RemoteSource::Clipboard => {
                    crate::clipboard::client::request_on_stream(
                        &mut stream,
                        DaemonRequest::GetClipboardHistory {
                            limit: history_limit,
                        },
                    )
                    .await
                }
                RemoteSource::Launcher => {
                    crate::clipboard::client::request_on_stream(
                        &mut stream,
                        DaemonRequest::GetLauncherItems,
                    )
                    .await
                }
            };

            let update = match resp {
                Ok(DaemonResponse::ClipboardHistory(entries)) => {
                    BackgroundUpdate::Items(crate::modes::clipboard::entries_to_items(entries))
                }
                Ok(DaemonResponse::LauncherItems(items)) => BackgroundUpdate::Items(items),
                Ok(other) => BackgroundUpdate::Error(format!("unexpected: {other:?}")),
                Err(err) => BackgroundUpdate::Error(err),
            };

            if tx.send(update).is_err() {
                break;
            }

            tokio::time::sleep(source.refresh_interval()).await;
        }
    });
}
