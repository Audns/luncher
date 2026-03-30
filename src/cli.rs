use clap::Parser;

#[derive(Parser)]
#[command(name = "luncher", about = "A fast Wayland launcher", version)]
pub struct Cli {
    #[arg(
        short = 'm',
        long = "mode",
        value_name = "MODE",
        help = "Launch mode: script (default), launcher, clipboard",
        default_value = "script"
    )]
    pub mode: String,
}

pub fn parse() -> Cli {
    Cli::parse()
}
