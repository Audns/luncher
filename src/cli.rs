use clap::Parser;

#[derive(Parser)]
#[command(
    name = "luncher",
    about = "Fast Wayland launcher with script, app, clipboard, and switcher modes",
    long_about = "Luncher is a daemon-backed Wayland launcher focused on fast startup. It can run scripts, search desktop applications, browse clipboard history, and switch Hyprland workspaces from the current client list.",
    after_help = "Examples:\n  luncher --daemon\n  luncher -m script\n  luncher -m launcher\n  luncher -m clipboard\n  luncher -m switcher"
)]
pub struct Cli {
    #[arg(
        short = 'm',
        long = "mode",
        value_name = "MODE",
        help = "Mode to open",
        long_help = "Mode to open: 'script' reads configured scripts, 'launcher' shows desktop applications, 'clipboard' shows clipboard history, and 'switcher' lists Hyprland windows grouped by workspace.",
        default_value = "script"
    )]
    pub mode: String,

    #[arg(
        long = "daemon",
        default_value_t = false,
        help = "Run the background daemon only",
        long_help = "Run the long-lived background daemon without opening the UI. This is intended for login/session autostart so clipboard history stays active before you open any launcher window."
    )]
    pub daemon: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}
