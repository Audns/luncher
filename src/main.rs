mod app;
mod cli;
mod config;
mod executor;
mod modes;
mod renderer;
mod search;
mod state;
mod stdin;

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
    let cli = cli::parse();
    if !atty::is(atty::Stream::Stdin) {
        modes::script::run_dmenu();
        return;
    }
    match cli.mode.as_str() {
        "script" => modes::script::run(),
        "launcher" => modes::launcher::run(),
        other => {
            eprintln!("Unknown mode: '{other}'. Valid modes: script, launcher");
            std::process::exit(1);
        }
    }
}
