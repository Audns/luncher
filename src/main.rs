mod app;
mod cli;
mod clipboard;
mod config;
mod executor;
mod instance;
mod launcher;
mod modes;
mod protocol;
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
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _ = rt.enter();

    run_with_runtime(rt);
}

fn run_with_runtime(rt: tokio::runtime::Runtime) {
    let cli = cli::parse();

    if cli.daemon {
        clipboard::daemon::run(rt);
        return;
    }

    let cfg = config::Config::load();

    let _lock = if cfg.single_instance {
        match instance::SingleInstance::try_acquire() {
            Ok(Some(lock)) => Some(lock),
            Ok(None) => {
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("[single_instance] lock error: {e}");
                None
            }
        }
    } else {
        None
    };

    if cli.mode.is_empty() && !atty::is(atty::Stream::Stdin) {
        modes::script::run_dmenu();
        return;
    }

    match cli.mode.as_str() {
        "script" => modes::script::run(),
        "launcher" => modes::launcher::run(rt),
        "clipboard" => modes::clipboard::run(rt),
        "switcher" => modes::switcher::run(),
        other => {
            eprintln!(
                "Unknown mode: '{other}'. Valid modes: script, launcher, clipboard, switcher"
            );
            std::process::exit(1);
        }
    }
}
