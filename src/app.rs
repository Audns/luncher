use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use std::time::Duration;
use wayland_client::{globals::registry_queue_init, Connection};

use crate::{
    search::LauncherItem,
    state::{AppState, ClipboardUpdate},
};

pub fn run(
    items: Vec<LauncherItem>,
    dmenu_mode: bool,
    clipboard_mode: bool,
    clipboard_handle: Option<tokio::runtime::Handle>,
    clipboard_runtime: Option<tokio::runtime::Runtime>,
) {
    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<AppState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let cfg = crate::config::Config::load();
    let clipboard_updates = clipboard_handle.map(spawn_clipboard_refresh_worker);

    let mut app = AppState::new(
        &globals,
        &qh,
        loop_handle,
        items,
        dmenu_mode,
        clipboard_mode,
        clipboard_updates,
        cfg.case_sensitive,
    );

    let _clipboard_runtime = clipboard_runtime;

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
        app.apply_pending_clipboard_updates();
        app.search.tick();
        if app.needs_redraw && app.configured {
            app.draw(&app.qh.clone());
        }
    }

    use std::io::Write;
    std::io::stdout().flush().ok();
    drop(app);
}

fn spawn_clipboard_refresh_worker(
    handle: tokio::runtime::Handle,
) -> std::sync::mpsc::Receiver<ClipboardUpdate> {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || loop {
        let update = match handle.block_on(crate::modes::clipboard::load_items()) {
            Ok(items) => ClipboardUpdate::Items(items),
            Err(err) => ClipboardUpdate::Error(err),
        };

        if tx.send(update).is_err() {
            break;
        }

        std::thread::sleep(Duration::from_millis(500));
    });

    rx
}
