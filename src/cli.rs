use clap::Parser;

use crate::modes::switcher::HyprAction;

#[derive(Parser)]
#[command(
    name = "luncher",
    about = "Fast Wayland launcher with script, app, clipboard, and switcher modes",
    long_about = "Luncher is a daemon-backed Wayland launcher focused on fast startup. It can run scripts, search desktop applications, browse clipboard history, and switch Hyprland workspaces from the current client list.",
    after_help = "Examples:\n  luncher --daemon\n  luncher -m script\n  luncher -m launcher\n  luncher -m clipboard\n  luncher -m switcher\n  luncher -m tool\n  luncher -m exec -f 'spotify'\n  luncher -m fetch -f 'spotify'\n  luncher -m fetch -f 'spotify' --only-script\n  luncher -m fetch -f 'spotify' --only-launcher"
)]
pub struct Cli {
    #[arg(
        short = 'm',
        long = "mode",
        value_name = "MODE",
        help = "Mode to open",
        long_help = "Mode to open: 'script' reads configured scripts, 'launcher' shows desktop applications, 'clipboard' shows clipboard history, 'switcher' lists Hyprland windows, 'tool' outputs all entries as JSON, 'exec' runs a script by name (requires -f), 'fetch' filters entries by pattern (requires -f)"
    )]
    pub mode: Option<String>,

    #[arg(
        short = 'f',
        long = "fix",
        value_name = "NAME",
        help = "Script name for exec/fetch mode",
        long_help = "Script name for exec mode, pattern for fetch mode"
    )]
    pub fix: Option<String>,

    #[arg(long = "only-script", help = "For fetch mode: only search in scripts")]
    pub only_script: bool,

    #[arg(
        long = "only-launcher",
        help = "For fetch mode: only search in launcher"
    )]
    pub only_launcher: bool,

    #[arg(
        long = "action",
        value_name = "ACTION",
        default_value_t = HyprAction::Switch,
        help = "For switcher mode: action to take on the selected window",
        long_help = "For switcher mode: 'pull' moves the window to the current workspace, 'switch' focuses the window's workspace, 'flip' swaps the current workspace's windows with the target workspace's windows"
    )]
    pub action: HyprAction,

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
