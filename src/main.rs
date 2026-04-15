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

    if cli.mode.is_none() && !atty::is(atty::Stream::Stdin) {
        modes::script::run_dmenu();
        return;
    }

    match cli.mode.as_deref().unwrap_or("script") {
        "script" => modes::script::run(),
        "launcher" => modes::launcher::run(rt),
        "clipboard" => modes::clipboard::run(rt),
        "switcher" => modes::switcher::run(cli.pull),
        "tool" => {
            println!("{}", modes::tool::run_json());
        }
        "exec" => {
            if let Some(name) = cli.fix.as_deref() {
                modes::exec::run(name);
            } else {
                eprintln!("exec mode requires -f/--fix argument");
                std::process::exit(1);
            }
        }
        "fetch" => {
            if let Some(pattern) = cli.fix.as_deref() {
                println!("{}", modes::fetch::run(pattern, cfg.case_sensitive, cli.only_script, cli.only_launcher));
            } else {
                eprintln!("fetch mode requires -f/--fix argument");
                std::process::exit(1);
            }
        }
        other => {
            eprintln!(
                "Unknown mode: '{other}'. Valid modes: script, launcher, clipboard, switcher, tool, exec, fetch"
            );
            std::process::exit(1);
        }
    }
}
