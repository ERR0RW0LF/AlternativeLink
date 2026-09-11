use std::{
    io::{self, Write, stdout}, net::Ipv4Addr, process::exit, sync::{Arc, atomic::AtomicBool}, 
};
use clap::{Parser, builder::styling};
use tokio::{sync::{Notify, watch::{self, Receiver}},};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;
use tracing::{Level, error, info, trace, warn};

use alternative_link_core::{ 
    SharedForSenders, SharedState, discovery::{validate_interface}, engine::{EngineArgs, broadcast_task, direct_comms_check_task, listen_all_messages}, helper::report_status,
};



const PORT: u16 = 1337;

const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Green.on_default().bold())
    .usage(styling::AnsiColor::Green.on_default().bold())
    .literal(styling::AnsiColor::Blue.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());

fn get_long_version() -> &'static str {
    Box::leak(format!("{} by {}", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS")).into_boxed_str())
}


#[derive(clap::Parser, Debug, Clone)]
#[command(styles = STYLES)]
#[command(version, author, about, long_version = get_long_version())]
struct Cli {
    /// Interface IP to bind to (skips interactive picker)
    #[arg(short, long)]
    interface: Ipv4Addr,

    /// UDP port to use
    #[arg(short, long, default_value_t = PORT)]
    port: u16,

    /// Shared room/pairing code
    #[arg(short, long, default_value = "1-3-3-7")]
    code: String,

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
    json: bool,

    /// Maximum retries for the direct connection test
    #[arg(long, default_value_t = 20)]
    max_direct_tries: u64
}









/*
mmmm      mmmm         aaaa         iiiiiiiiii     nnnnn      nnn
mmmmm    mmmmm         aaaa         iiiiiiiiii     nnnnnn     nnn
mmmmmm  mmmmmm       aaa  aaa          iiii        nnn nnn    nnn
mmm mmmmmm mmm       aaa  aaa          iiii        nnn  nnn   nnn
mmm  mmmm  mmm     aaa     aaa         iiii        nnn   nnn  nnn
mmm   mm   mmm     aaaaaaaaaaa         iiii        nnn    nnn nnn
mmm        mmm   aaa        aaa     iiiiiiiiii     nnn     nnnnnn
mmm        mmm   aaa        aaa     iiiiiiiiii     nnn      nnnnn
*/
// MAIN
#[tokio::main]
async fn main() -> io::Result<()>{
    let args = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(match args.verbose {
            0 => {Level::WARN},
            1 => {Level::INFO},
            2 => {Level::DEBUG},
            3 => {Level::TRACE},
            i => {println!("We don't have v*{}. Using Info level as fall back.", i); Level::INFO}
        })
        .init();

    let ipaddr = match validate_interface(args.interface) {
        Ok(b) => {
            if b {
                args.interface
            } else {
                error!("{} isn't a valid interface choice.", args.interface);
                exit(1)
            }
        },
        Err(e) => {
            error!("{}", e);
            exit(1)
        }
    };


    trace!(own_ip = ipaddr.to_string());

    let engine_args: EngineArgs = EngineArgs {
        code: args.code.clone(),
        port: args.port,
        json: args.json
    };
    trace!("Build the EngineArgs from the Cli struct");



    let sock_listen: UdpSocket = UdpSocket::bind(format!("0.0.0.0:{}",args.port) as String).await?;
    info!("Listening on port {}", args.port);

    if let Err(e) = sock_listen.set_broadcast(true) {
        warn!("Couldn't set listening socket broadcasting: {}", e)
    }
    trace!("Set broadcasting to true for listening socket.");

    let sock_sender: UdpSocket = UdpSocket::bind(format!("{}:0",ipaddr) as String).await?;
    trace!("Started sender socket.");

    if let Err(e) = sock_sender.set_broadcast(true) {
        warn!("Couldn't set sender socket broadcasting: {}", e)
    }
    trace!("Set broadcasting to true for sender socket.");

    let token = CancellationToken::new();
    trace!("Created CancellationToken.");


    // Building the SharedForSenders
    let shared_for_senders = Arc::new(SharedForSenders {
        sock: sock_sender,
        cancel: token.clone(),
    });

    let direct_working = Arc::new(AtomicBool::new(false));
    trace!("Created direct_working flag using an AtomicBool.");

    let (tx, rx) = watch::channel(None);
    trace!("Created Sender and Receiver for the Ipv4Addr of the other device.");

    let direct_working_notify = Arc::new(Notify::new());
    

    let shared_state: Arc<SharedState> = Arc::new(SharedState{finished: token.clone(), other_link_ip: tx});
    trace!("Made everything into a shared state.");

    let message = format!("{} {}\n", args.code, ipaddr);
    trace!(msg=message);
    info!("Using code {}", args.code);

    let mut tasks = Vec::new();
    let engine_args_clone = engine_args.clone();
    let shared_for_senders_clone: Arc<SharedForSenders> = shared_for_senders.clone();



    tasks.push(tokio::spawn(async move {broadcast_task(shared_for_senders_clone, message.as_bytes().to_vec(), args.broadcast_interval, engine_args_clone).await}));
    trace!("Pushed the broadcast_task to tasks.");

    let shared_state_clone = shared_state.clone();
    let directly_working_clone = direct_working.clone();
    let direct_working_notify_clone = direct_working_notify.clone();
    let engine_args_clone = engine_args.clone();
    tasks.push(tokio::spawn(async move {listen_all_messages(sock_listen, shared_state_clone, ipaddr, directly_working_clone, direct_working_notify_clone, engine_args_clone).await}));
    trace!("Pushed the listen_all_messages to tasks.");
    

    let mut other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    loop {
        if other_link_ip_rx_clone.changed().await.is_err() { break; }
        match *other_link_ip_rx_clone.borrow() {
            Some(ip) => {
                report_status(&engine_args, &format!("Found peer at {}", ip), &[("peer_ip",&ip.to_string())]);
                break;
            },
            None => {continue;}
        }
    }

    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    if !args.auto_test
        && let Err(e) = tokio::task::spawn_blocking(move || {
            use std::io::stdin;
            loop {
                let mut buffer = String::new();
                let other_ip = other_link_ip_rx_clone.borrow().unwrap();
                println!("1: Quit\n2: Test Connection to {}", other_ip);
                let stdin = stdin();
                print!("Please chose the option to use (num): ");
                if let Err(e) = stdout().flush() {
                    warn!("Problem whilst flushing stdout: {}", e)
                }
                stdin.read_line(&mut buffer).expect("Reading your input didn't work");
                let choice: usize = buffer.trim().parse::<usize>().unwrap_or(0);

                match choice {
                    1 => {token.cancel();break;},
                    2 => {break;},
                    _ => {},
                }
            }

        }).await {
            warn!("Something went wrong with the input for testing the direct connection: {}", e)
        
    }

    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    tasks.push(tokio::spawn(async move {direct_comms_check_task(shared_for_senders, other_link_ip_rx_clone, direct_working, direct_working_notify, ipaddr, args.max_direct_tries, engine_args).await}));


    for task in tasks {
        if let Err(e) =  task.await {
            warn!("Something went wrong whilst Joining a task: {}", e)
        }
    }
    

    Ok(())
}


// ==========================================  ========================================== \\
// ==========================================  ========================================== \\
// ==========================================  ========================================== \\
// ==========================================  ========================================== \\





