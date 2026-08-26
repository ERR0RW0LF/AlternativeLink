use std::{fmt::Display, io::{self, Write, stdin, stdout}, net::Ipv4Addr, ops::Index, process::exit, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use tokio::{sync::watch::{self, Receiver}, time::sleep};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;
use if_addrs::IfAddr;
use tracing::{debug, info, warn};

struct SharedState {
    finished: CancellationToken,
    direct_working: Arc<AtomicBool>,
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
                    Err(e) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            [cmd, ip] if *cmd == "ACK" => {
                match ip.parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ack(d))},
                    Err(e) => {Err("Parsing str to Ipv4Addr")}
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

#[derive(PartialEq)]
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

const CODE: &str = "1-3-3-7";
const PORT: usize = 1337;

async fn listen_all_messages(
    sock: tokio::net::UdpSocket, shared_state: Arc<SharedState>, 
    own_ipaddr: Ipv4Addr, direct_working: Arc<AtomicBool>
) -> io::Result<()> {
    let mut other_ip_rec: Option<Ipv4Addr> = None;
    
    let mut buf = [0; 1024];
    loop {
        tokio::select! {
            _ = shared_state.finished.cancelled() => break,
            t = sock.recv_from(&mut buf) => {
                let (len, addr) = match t {
                    Ok((len, addr)) => {(len,addr)},
                    Err(e) => {continue;}
                };
                
                let s = match str::from_utf8(&buf[..len] as &[u8]) {
                    Ok(v) => v,
                    Err(e) => {continue;}
                };
                
                let message = match Message::try_from(s) {
                    Ok(v) => {v},
                    Err(e) => {continue;}
                };

                // gate for against own messages 
                if message.sender_ip() == own_ipaddr { continue; } 
                //println!("{:?} bytes received from {:?}", len, addr);
                
                match message {
                    Message::Discover(code, ip) => {
                        match other_ip_rec {
                            Some(_) => {},
                            None => {
                                if (String::from(CODE)) == code.to_string() {
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
                        sock.send_to(&message.as_bytes(), format!("{}:{}", m, PORT)).await?;
                    },
                    Message::Ack(m) => {
                        debug!("Got ACK from {}. other_ip_rec is {:?}", m, other_ip_rec);
                        match other_ip_rec {
                            Some(g) => {
                                if m == g {
                                    direct_working.store(true, Ordering::Relaxed);
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


async fn broadcast_task(sock: Arc<tokio::net::UdpSocket>, msg: Vec<u8>, cancel: CancellationToken) -> io::Result<()> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = sock.send_to(&msg, format!("255.255.255.255:{}",PORT)) => {
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
    Ok(())
}

async fn direct_comms_check_task(sock: Arc<tokio::net::UdpSocket>, ip_receiver: watch::Receiver<Option<Ipv4Addr>>, cancel: CancellationToken, direct_working: Arc<AtomicBool>, ipaddr: Ipv4Addr) -> io::Result<()> {
    println!("Test worked {} {}", ip_receiver.borrow().unwrap(), cancel.is_cancelled());
    let mut probe_interval = tokio::time::interval(Duration::from_secs(5));
    
    const MAX_TRIES: usize  = 20;
    let mut timeout_counter: usize = 0;

    loop {
        let is_working = direct_working.load(Ordering::Relaxed);
        if is_working {
            println!("Connection Worked");
            cancel.cancel();
            break;
        }

        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = probe_interval.tick(), if !is_working => {
                let ip_opt = *ip_receiver.borrow();
                if let Some(ip) = ip_opt {
                    match timeout_counter {
                        i if timeout_counter > MAX_TRIES => { warn!("Didn't get a Ack in time, exhausted retries ({}/{}). Exiting.", i,  MAX_TRIES); exit(0)}
                        i if timeout_counter > 0 => { warn!("Retrying {}/{}", i, MAX_TRIES) },
                        _ => {},
                    }
                    info!("Pinging");
                    if let Err(e) = sock.send_to(format!("PING {}\n", ipaddr).as_bytes(), format!("{}:{}",ip,PORT)).await {
                        warn!("Error sending with: {}", e)
                    };
                    timeout_counter += 1;
                }
            },
        }
    }
    Ok(())
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into())
        )
        .init();


    let ipaddr = get_ipaddr().unwrap();
    let sock_listen: UdpSocket = UdpSocket::bind(format!("0.0.0.0:{}",PORT) as String).await?;
    if let Err(e) = sock_listen.set_broadcast(true) {
        warn!("Couldn't set listening socket broadcasting: {}", e)
    }

    let sock_sender: Arc<UdpSocket> = Arc::new(UdpSocket::bind(format!("{}:0",ipaddr) as String).await?);
    if let Err(e) = sock_sender.set_broadcast(true) {
        warn!("Couldn't set sender socket broadcasting: {}", e)
    }


    let token = CancellationToken::new();
    let direct_working = Arc::new(AtomicBool::new(false));
    let (tx, rx) = watch::channel(None);

    let shared_state: Arc<SharedState> = Arc::new(SharedState{finished: token.clone(), direct_working: direct_working.clone(), other_link_ip: tx});
    
    let message = format!("{} {}\n", CODE, ipaddr);

    println!("{}", ipaddr);
    let mut tasks = Vec::new();
    let shared_state_clone = shared_state.clone();
    let token_clone = token.clone();
    let sock_sender_clone = sock_sender.clone();
    tasks.push(tokio::spawn(async move {broadcast_task(sock_sender_clone, message.as_bytes().to_vec(), token_clone).await}));
    
    let token_clone = token.clone();
    let shared_state_clone = shared_state.clone();
    let directly_working_clone = direct_working.clone();
    tasks.push(tokio::spawn(async move {listen_all_messages(sock_listen, shared_state_clone, ipaddr.clone(), directly_working_clone).await}));
    
    let mut other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    loop {
        if other_link_ip_rx_clone.changed().await.is_err() { break; }
        match *other_link_ip_rx_clone.borrow() {
            Some(_) => {break;},
            None => {continue;}
        }
    }

    let token_clone = token.clone();
    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
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

    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    
    let sock_sender_clone = sock_sender.clone();
    tasks.push(tokio::spawn(async move {direct_comms_check_task(sock_sender_clone, other_link_ip_rx_clone, token_clone, direct_working, ipaddr).await}));


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