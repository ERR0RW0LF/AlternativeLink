use std::net::Ipv4Addr;

use clap::builder::styling;

const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Green.on_default().bold())
    .usage(styling::AnsiColor::Green.on_default().bold())
    .literal(styling::AnsiColor::Blue.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());

fn get_long_version() -> &'static str {
    Box::leak(format!("{} by {}", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS")).into_boxed_str())
}

const CODE: usize = 1337;
const PORT: u16 = 1337;

#[derive(clap::Parser, Debug, Clone)]
#[command(styles = STYLES)]
#[command(version, author, about, long_version = get_long_version())]
pub struct Cli {
    /// Interface IP to bind to (skips interactive picker)
    #[arg(short, long)]
    interface: Option<Ipv4Addr>,

    /// UDP port to use
    #[arg(short, long, default_value_t = PORT)]
    pub(crate) port: u16,

    /// Shared room/pairing code
    #[arg(short, long, default_value = "1-3-3-7")]
    pub(crate) code: String,

    /// Broadcast interval in seconds
    #[arg(long, default_value_t = 5)]
    broadcast_interval: u64,

    /// Run non-interactively: auto-test connection as soon as a peer is found
    #[arg(long)]
    auto_test: bool,

    /// Increase log verbosity (-v -vv)
    #[arg(short, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Emit machine-readable JSON status lines instead of prose
    #[arg(long)]
    pub(crate) json: bool,

    /// Maximum retries for the direct connection test
    #[arg(long, default_value_t = 20)]
    max_direct_tries: u64
}