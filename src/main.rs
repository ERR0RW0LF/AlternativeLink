use std::{io::{self, Write, stdin, stdout}, net::Ipv4Addr, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use tokio::{sync::watch::{self, Receiver}, time::sleep};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;
use if_addrs::IfAddr;

struct SharedState {
    finished: CancellationToken,
    direct_working: Arc<AtomicBool>,
    other_link_ip: watch::Sender<Option<Ipv4Addr>>,
}

enum Message {
    Discover(Ipv4Addr),
    Ping(Ipv4Addr),
    Ack(Ipv4Addr),
}

impl TryFrom<&str> for Message {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().split(" ").collect::<Vec<&str>>() {
            g if g[0] == CODE => {
                match g[1].parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Discover(d))},
                    Err(e) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            g if g[0] == "PING" => {
                match g[1].parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ping(d))},
                    Err(e) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            g if g[0] == "ACK" => {
                match g[1].parse::<Ipv4Addr>() {
                    Ok(d) => {Ok(Message::Ack(d))},
                    Err(e) => {Err("Parsing str to Ipv4Addr")}
                }
            },
            _ => {Err("Conversion didn't work")}
        }
    }
    
}


const CODE: &str = "1-3-3-7";
const PORT: usize = 1337;

async fn listen_all_messages(sock: tokio::net::UdpSocket, shared_state: Arc<SharedState>, ipaddr: Ipv4Addr, direct_working: Arc<AtomicBool>) -> io::Result<()> {
    let mut other_ip_rec: Option<Ipv4Addr> = None;
    
    let mut buf = [0; 1024];
    loop {
        tokio::select! {
            _ = shared_state.finished.cancelled() => break,
            t = sock.recv_from(&mut buf) => {
                let (len, addr) = t.unwrap();
                
                let s = match str::from_utf8(&buf[..len] as &[u8]) {
                    Ok(v) => v,
                    Err(e) => panic!("Invalid UTF-8 sequence: {}", e)
                };
                
                let message = match Message::try_from(s) {
                    Ok(v) => {v},
                    Err(e) => {continue;}
                };

                if s.trim().split(" ").collect::<Vec<_>>()[1].parse::<Ipv4Addr>().unwrap_or(ipaddr) == ipaddr {
                    continue;
                } 
                //println!("{:?} bytes received from {:?}", len, addr);
                
                match message {
                    Message::Discover(m) => {
                        match other_ip_rec {
                            Some(_) => {},
                            None => {
                                other_ip_rec = Some(m);
                                let _ = shared_state.other_link_ip.send(Some(m));
                            }
                        }
                    },
                    Message::Ping(m) => {
                        println!("Got PING");
                        let message = format!("ACK {}\n", ipaddr);
                        sock.send_to(&message.as_bytes(), format!("{}:{}", m, PORT)).await?;
                    },
                    Message::Ack(m) => {
                        println!("Got ACK from {}. other_ip_rec is {:?}", m, other_ip_rec);
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


async fn broadcast_task(sock: tokio::net::UdpSocket, msg: Vec<u8>, cancel: CancellationToken) -> io::Result<()> {
    let msg_as_string = String::from_utf8(msg.clone()).unwrap_or("Didn't work".to_string());
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = sock.send_to(&msg, format!("255.255.255.255:{}",PORT)) => {
                //print!("Send broadcast: {}", msg_as_string);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
    Ok(())
}

async fn direct_comms_check_task(sock: tokio::net::UdpSocket, ip_receiver: watch::Receiver<Option<Ipv4Addr>>, cancel: CancellationToken, direct_working: Arc<AtomicBool>, ipaddr: Ipv4Addr) -> io::Result<()> {
    println!("Test worked {} {}", ip_receiver.borrow().unwrap(), cancel.is_cancelled());
    let mut probe_interval = tokio::time::interval(Duration::from_secs(5));
    
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
                    println!("Pinging");
                    let _ = sock.send_to(format!("PING {}\n", ipaddr).as_bytes(), format!("{}:{}",ip,PORT)).await;
                }
            },
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()>{
    let ipaddr = get_ipaddr().unwrap();
    let sock_listen: UdpSocket = UdpSocket::bind(format!("{}:{}",ipaddr,PORT) as String).await?;
    let _ = sock_listen.set_broadcast(true);
    let sock_broadcast: UdpSocket = UdpSocket::bind(format!("{}:0",ipaddr) as String).await?;
    let _ = sock_broadcast.set_broadcast(true);
    let sock_direct: UdpSocket = UdpSocket::bind(format!("{}:0",ipaddr) as String).await?;
    let _ = sock_direct.set_broadcast(true);


    let token = CancellationToken::new();
    let direct_working = Arc::new(AtomicBool::new(false));
    let (tx, rx) = watch::channel(None);

    let shared_state: Arc<SharedState> = Arc::new(SharedState{finished: token.clone(), direct_working: direct_working.clone(), other_link_ip: tx});
    
    let message = format!("{} {}\n", CODE, ipaddr);

    println!("{}", ipaddr);
    let mut tasks = Vec::new();
    let shared_state_clone = shared_state.clone();
    let token_clone = token.clone();
    tasks.push(tokio::spawn(async move {broadcast_task(sock_broadcast, message.as_bytes().to_vec(), token_clone).await}));
    
    let token_clone = token.clone();
    let shared_state_clone = shared_state.clone();
    let directly_working_clone = direct_working.clone();
    tasks.push(tokio::spawn(async move {listen_all_messages(sock_listen, shared_state_clone, ipaddr.clone(), directly_working_clone).await}));
    
    let mut other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    loop {
        let _ = other_link_ip_rx_clone.changed().await;
        match *other_link_ip_rx_clone.borrow() {
            Some(_) => {break;},
            None => {continue;}
        }
    }

    let token_clone = token.clone();
    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    let _ = tokio::task::spawn_blocking(move || {
        use std::io::stdin;
        loop {
            let mut buffer = String::new();
            let other_ip = other_link_ip_rx_clone.borrow().unwrap();
            println!("1: Quit\n2: Test Connection to {}", other_ip);
            let stdin = stdin();
            print!("Please chose the option to use (num): ");
            let _ = stdout().flush();
            stdin.read_line(&mut buffer).expect("Reading your input didn't work");
            let choice: usize = buffer.trim().parse::<usize>().unwrap_or(0);

            match choice {
                1 => {token.cancel();break;},
                2 => {break;},
                _ => {},
            }
        }
        
    }).await;

    let other_link_ip_rx_clone: Receiver<Option<Ipv4Addr>> = rx.clone();
    
    tasks.push(tokio::spawn(async move {direct_comms_check_task(sock_direct, other_link_ip_rx_clone, token_clone, direct_working, ipaddr).await}));


    for task in tasks {
        let _ = task.await;
    }
    

    Ok(())
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
    let _ = stdout().flush();
    stdin.read_line(&mut buffer).expect("Reading your input didn't work");
    let choice: usize = buffer.trim().parse::<usize>().expect("Next time please a number");
    match possible_ips.get(choice) {
        Some(ipv4addr) => Some(*ipv4addr),
        _ => None
    }
}