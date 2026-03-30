use clap::Parser;

#[derive(Parser)]
#[command(
    name = "luncher",
    about = "Fast Wayland launcher with script, app, and clipboard modes",
    long_about = "Luncher is a daemon-backed Wayland launcher focused on fast startup. It can run scripts, search desktop applications, and browse clipboard history.",
    after_help = "Examples:\n  luncher --daemon\n  luncher -m script\n  luncher -m launcher\n  luncher -m clipboard"
)]
pub struct Cli {
    #[arg(
        short = 'm',
        long = "mode",
        value_name = "MODE",
        help = "Mode to open",
        long_help = "Mode to open: 'script' reads configured scripts, 'launcher' shows desktop applications, and 'clipboard' shows clipboard history.",
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
