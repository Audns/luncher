use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use wayland_client::{Connection, globals::registry_queue_init};

use crate::{search::LauncherItem, state::AppState};

pub fn run(items: Vec<LauncherItem>, dmenu_mode: bool) {
    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<AppState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let cfg = crate::config::Config::load();

    let mut app = AppState::new(
        &globals,
        &qh,
        loop_handle,
        items,
        dmenu_mode,
        cfg.case_sensitive,
    );

    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .unwrap();

    loop {
        event_loop.dispatch(None, &mut app).unwrap();
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
