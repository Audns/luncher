mod config;
mod executor;
mod renderer;
mod search;
mod state;
mod stdin;

use std::io::Write;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::{Connection, globals::registry_queue_init};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_registry,
    delegate_seat, delegate_shm,
};
use state::AppState;

delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_seat!(AppState);
delegate_shm!(AppState);
delegate_layer!(AppState);
delegate_registry!(AppState);
delegate_keyboard!(AppState);

fn main() {
    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<AppState> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let mut app = AppState::new(&globals, &qh, loop_handle);

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
    std::io::stdout().flush().ok();
}
