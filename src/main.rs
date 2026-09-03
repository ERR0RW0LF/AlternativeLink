use std::{
    io::{self, Write, stdin, stdout}, 
    net::Ipv4Addr, 
    process::exit, 
    sync::{Arc, atomic::{AtomicBool, Ordering}}, 
    time::Duration
};
use clap::{Parser, builder::styling};
use tokio::{sync::{Notify, watch::{self, Receiver}}, time::sleep};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;
use if_addrs::IfAddr;
use tracing::{Level, debug, info, trace, warn};

struct SharedState {
    finished: CancellationToken,
    other_link_ip: watch::Sender<Option<Ipv4Addr>>,
}

enum Message {
    Discover(Code, Ipv4Addr),
    Ping(Ipv4Addr),
    Ack(Ipv4Addr),
}

impl Message {
    fn sender_ip(&self) -> Ipv4Addr {
        match self {
            Message::Discover(_, ip) | Message::Ping(ip) | Message::Ack(ip) => *ip,
        }
    }
}


impl TryFrom<&str> for Message {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {


        match value.trim().split(' ').collect::<Vec<_>>().as_slice() {
            [cmd, ip] if *cmd == "PING" => {
                match ip.parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ping(d))},
                    Err(_) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            [cmd, ip] if *cmd == "ACK" => {
                match ip.parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ack(d))},
                    Err(_) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            [cmd, ip] if let Ok(g) = ip.parse::<Ipv4Addr>() => {

                if let Ok(code) = Code::try_from(*cmd) {
                    Ok(Message::Discover(code, g))
                } else {
                    Err("Creating the code didn't work")
                }
                
            },
            _ => {Err("Conversion didn't work")}
        }
    }
    
}

#[derive(PartialEq, Clone, Copy)]
struct Code {
    code_uint: usize
}

impl TryFrom<&str> for Code {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.replace("-", "");
        if let Ok(result) = value.parse::<usize>() {
            Ok(Code { code_uint: result })
        } else {
            Err("Parsing didn't work")
        }
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.code_uint.to_string();
        write!(f, "{}", s.chars().collect::<Vec<_>>().chunks(1)
            .map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-"))
    }
}

const CODE: usize = 1337;
const PORT: u16 = 1337;

async fn listen_all_messages(
    sock: tokio::net::UdpSocket, shared_state: Arc<SharedState>, 
    own_ipaddr: Ipv4Addr, 
    direct_working: Arc<AtomicBool>, direct_working_notify: Arc<Notify>,
    args: Cli,
) -> io::Result<()> {
    let mut other_ip_rec: Option<Ipv4Addr> = None;
    
    let mut buf = [0; 1024];
    let my_code = if let Ok(c) = Code::try_from(&args.code as &str) {
        c
    } else {
        Code {code_uint: CODE}
    };
    loop {
        tokio::select! {
            _ = shared_state.finished.cancelled() => break,
            t = sock.recv_from(&mut buf) => {
                let len = match t {
                    Ok((len, _)) => {len},
                    Err(e) => {debug!("{}", e);continue;}
                };
                
                let s = match str::from_utf8(&buf[..len] as &[u8]) {
                    Ok(v) => v,
                    Err(e) => {debug!("{}", e);continue;}
                };
                
                let message = match Message::try_from(s) {
                    Ok(v) => {v},
                    Err(e) => {debug!("{}", e);continue;}
                };

                // gate for against own messages 
                if message.sender_ip() == own_ipaddr { continue; } 
                //println!("{:?} bytes received from {:?}", len, addr);
                

                match message {
                    Message::Discover(code, ip) => {
                        match other_ip_rec {
                            Some(_) => {},
                            None => {
                                if my_code == code {
                                    other_ip_rec = Some(ip);
                                    if let Err(e) = shared_state.other_link_ip.send(Some(ip)) {
                                        warn!("Couldn't send the discoverd ip using the sender on the shared channel: {}", e)
                                    }
                                }
                            }
                        }
                    },
                    Message::Ping(m) => {
                        debug!("Got PING");
                        let message = format!("ACK {}\n", own_ipaddr);
                        sock.send_to(&message.as_bytes(), format!("{}:{}", m, args.port)).await?;
                    },
                    Message::Ack(m) => {
                        debug!("Got ACK from {}. other_ip_rec is {:?}", m, other_ip_rec);
                        match other_ip_rec {
                            Some(g) => {
                                if m == g {
                                    direct_working.store(true, Ordering::Relaxed);
                                    direct_working_notify.notify_waiters();
                                }
                            },
                            None => {}
                        }
                    },
                }
            }
        }
    }
    Ok(())
}








async fn broadcast_task(sock: Arc<tokio::net::UdpSocket>, msg: Vec<u8>, cancel: CancellationToken, broadcast_interval: u64, args: Cli) -> io::Result<()> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = sock.send_to(&msg, format!("255.255.255.255:{}",args.port)) => {
                trace!("Send a broadcast, now waiting for {}s", broadcast_interval);
                sleep(Duration::from_secs(broadcast_interval)).await;
            }
        }
    }
    Ok(())
}








async fn direct_comms_check_task(
    sock: Arc<tokio::net::UdpSocket>, ip_receiver: watch::Receiver<Option<Ipv4Addr>>, 
    cancel: CancellationToken, 
    direct_working: Arc<AtomicBool>, direct_working_notify: Arc<Notify>,
    ipaddr: Ipv4Addr,
    max_tries: u64, args: Cli
) -> io::Result<()> {
    println!("Test worked {} {}", ip_receiver.borrow().unwrap(), cancel.is_cancelled());
    let mut probe_interval = tokio::time::interval(Duration::from_secs(5));
    
    let mut timeout_counter: u64 = 0;

    loop {
        let is_working = direct_working.load(Ordering::Relaxed);
        if is_working {
            println!("Connection Worked");
            cancel.cancel();
            break;
        }

        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = direct_working_notify.notified() => {
                if direct_working.load(Ordering::Relaxed) {
                    info!("Connection Worked");
                    cancel.cancel();
                    break;
                }
            },
            _ = probe_interval.tick(), if !is_working => {
                let ip_opt = *ip_receiver.borrow();
                if let Some(ip) = ip_opt {
                    match timeout_counter {
                        i if timeout_counter > max_tries => { warn!("Didn't get a Ack in time, exhausted retries ({}/{}). Exiting.", i,  max_tries); exit(0)}
                        i if timeout_counter > 0 => { warn!("Retrying {}/{}", i, max_tries) },
                        _ => {},
                    }
                    trace!("Pinging");
                    if let Err(e) = sock.send_to(format!("PING {}\n", ipaddr).as_bytes(), format!("{}:{}",ip,args.port)).await {
                        warn!("Error sending with: {}", e)
                    };
                    timeout_counter += 1;
                }
            },
        }
    }
    Ok(())
}



const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Green.on_default().bold())
    .usage(styling::AnsiColor::Green.on_default().bold())
    .literal(styling::AnsiColor::Blue.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());




#[derive(clap::Parser, Debug, Clone)]
#[command(styles = STYLES)]
struct Cli {
    /// Interface IP to bind to (skips interactive picker)
    #[arg(short, long)]
    interface: Option<Ipv4Addr>,

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
            i if i <= 0 => {Level::WARN},
            i if i <= 1 => {Level::INFO},
            i if i == 2 => {Level::DEBUG},
            i if i == 3 => {Level::TRACE},
            i => {println!("We don't have v*{}. Using Info level as fall back.", i); Level::INFO}
        })
        .init();

    let ipaddr = match args.interface {
        Some(ipaddr) => {
            if validate_interface(ipaddr) {
                ipaddr
            } else {
                get_ipaddr().unwrap()
            }
        },
        None => {
            get_ipaddr().unwrap()
        }
    };


    trace!(own_ip = ipaddr.to_string());



    let sock_listen: UdpSocket = UdpSocket::bind(format!("0.0.0.0:{}",args.port) as String).await?;
    trace!("Started listening socket.");

    if let Err(e) = sock_listen.set_broadcast(true) {
        warn!("Couldn't set listening socket broadcasting: {}", e)
    }
    trace!("Set broadcasting to true for listening socket.");

    let sock_sender: Arc<UdpSocket> = Arc::new(UdpSocket::bind(format!("{}:0",ipaddr) as String).await?);
    trace!("Started sender socket.");

    if let Err(e) = sock_sender.set_broadcast(true) {
        warn!("Couldn't set sender socket broadcasting: {}", e)
    }
    trace!("Set broadcasting to true for sender socket.");



    let token = CancellationToken::new();
    trace!("Created CancellationToken.");

    let direct_working = Arc::new(AtomicBool::new(false));
    trace!("Created direct_working flag using an AtomicBool.");

    let (tx, rx) = watch::channel(None);
    trace!("Created Sender and Receiver for the Ipv4Addr of the other device.");

    let direct_working_notify = Arc::new(Notify::new());
    

    let shared_state: Arc<SharedState> = Arc::new(SharedState{finished: token.clone(), other_link_ip: tx});
    trace!("Made everything into a shared state.");

    let message = format!("{} {}\n", args.code, ipaddr);
    trace!(msg=message);

    let mut tasks = Vec::new();
    let token_clone = token.clone();
    let sock_sender_clone = sock_sender.clone();
    let args_clone = args.clone();
    tasks.push(tokio::spawn(async move {broadcast_task(sock_sender_clone, message.as_bytes().to_vec(), token_clone, args.broadcast_interval, args_clone).await}));
    trace!("Pushed the broadcast_task to tasks.");

    let shared_state_clone = shared_state.clone();
    let directly_working_clone = direct_working.clone();
    let direct_working_notify_clone = direct_working_notify.clone();
    let args_clone = args.clone();
    tasks.push(tokio::spawn(async move {listen_all_messages(sock_listen, shared_state_clone, ipaddr.clone(), directly_working_clone, direct_working_notify_clone, args_clone).await}));
    trace!("Pushed the listen_all_messages to tasks.");
    

    let mut other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    loop {
        if other_link_ip_rx_clone.changed().await.is_err() { break; }
        match *other_link_ip_rx_clone.borrow() {
            Some(_) => {trace!("other_link_ip is now set. Going to the next step.");break;},
            None => {continue;}
        }
    }

    let token_clone = token.clone();
    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    if !args.auto_test {
        if let Err(e) = tokio::task::spawn_blocking(move || {
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
            warn!("Something whet wrong with the input for testing the direct connection: {}", e)
        }
    }

    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    
    let sock_sender_clone = sock_sender.clone();
    let args_clone = args.clone();
    tasks.push(tokio::spawn(async move {direct_comms_check_task(sock_sender_clone, other_link_ip_rx_clone, token_clone, direct_working, direct_working_notify, ipaddr, args.max_direct_tries, args_clone).await}));


    for task in tasks {
        if let Err(e) =  task.await {
            warn!("Something whet wrong whilst Joining a task: {}", e)
        }
    }
    

    Ok(())
}


// ==========================================  ========================================== \\
// ==========================================  ========================================== \\
// ==========================================  ========================================== \\
// ==========================================  ========================================== \\





fn validate_interface(ipaddr: Ipv4Addr) -> bool{
    let mut possible_ips: Vec<Ipv4Addr> = Vec::new();
    for iface in if_addrs::get_if_addrs().unwrap() {
        match iface.addr {
            IfAddr::V4(ifv4_addr) => {
                if !ifv4_addr.is_loopback() {
                    possible_ips.push(ifv4_addr.ip);
                }
            },
            _ => {}
        }
    }
    possible_ips.contains(&ipaddr)
}


fn get_ipaddr() -> Option<Ipv4Addr>{
    let mut possible_ips: Vec<Ipv4Addr> = Vec::new();
    for iface in if_addrs::get_if_addrs().unwrap() {
        match iface.addr {
            IfAddr::V4(ifv4_addr) => {
                if !ifv4_addr.is_loopback() {
                    possible_ips.push(ifv4_addr.ip);
                }
            },
            _ => {}
        }
    }
    for i in 0..possible_ips.len() {
        println!("- {number:0>2}: {}",possible_ips[i], number=i)
    }
    let mut buffer = String::new();
    let stdin = stdin();
    print!("Please chose the ip to use (num): ");
    if let Err(e) = stdout().flush() {
        warn!("Problem whilst flushing stdout: {}", e)
    }
    stdin.read_line(&mut buffer).expect("Reading your input didn't work");
    let choice: usize = buffer.trim().parse::<usize>().expect("Next time please a number");
    match possible_ips.get(choice) {
        Some(ipv4addr) => Some(*ipv4addr),
        _ => None
    }
}