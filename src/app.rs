use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use std::time::Duration;
use wayland_client::{globals::registry_queue_init, Connection};

use crate::{
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
            Self::Clipboard => Duration::from_millis(500),
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
) {
    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<AppState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let cfg = crate::config::Config::load();
    let remote_updates = match (remote_source, remote_handle) {
        (Some(source), Some(handle)) => Some(spawn_remote_refresh_worker(source, handle)),
        _ => None,
    };

    let mut app = AppState::new(
        &globals,
        &qh,
        loop_handle,
        items,
        dmenu_mode,
        clipboard_mode,
        remote_updates,
        cfg.case_sensitive,
    );

    let _remote_runtime = remote_runtime;

    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .unwrap();

    loop {
        event_loop
            .dispatch(Some(Duration::from_millis(250)), &mut app)
            .unwrap();
        if app.exit {
            break;
        }
        app.apply_pending_background_updates();
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
) -> std::sync::mpsc::Receiver<BackgroundUpdate> {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || loop {
        let update = match source {
            RemoteSource::Clipboard => handle.block_on(crate::modes::clipboard::load_items()),
            RemoteSource::Launcher => handle.block_on(crate::launcher::client::load_items()),
        };

        let update = match update {
            Ok(items) => BackgroundUpdate::Items(items),
            Err(err) => BackgroundUpdate::Error(err),
        };

        if tx.send(update).is_err() {
            break;
        }

        std::thread::sleep(source.refresh_interval());
    });

    rx
}
