use std::{io::{self, Write, stdin, stdout}, net::Ipv4Addr, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use tokio::{sync::watch, time::sleep};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;
use if_addrs::IfAddr;

struct SharedState {
    finished: CancellationToken,
    direct_working: AtomicBool,
    other_link_ip_sender: watch::Sender<Option<Ipv4Addr>>,
    other_link_ip_receiver: watch::Receiver<Option<Ipv4Addr>>,
}

const CODE: &str = "1-3-3-7";
const PORT: usize = 1337;

async fn listen_all_messages(sock: tokio::net::UdpSocket, shared_state: Arc<SharedState>, ipaddr: Ipv4Addr) -> io::Result<()> {
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
                
                if s.trim().split(" ").collect::<Vec<_>>()[1].parse::<Ipv4Addr>().unwrap_or(ipaddr) == ipaddr {
                    continue;
                } 
                println!("{:?} bytes received from {:?}", len, addr);
        
                match s {
                    v if v.starts_with("PING ") => {
                        println!("Got PING");
                        let remote_ip = v.strip_prefix("PING ").unwrap().strip_suffix("\n").unwrap();
                        let message = format!("ACK {}\n", ipaddr);
                        sock.send_to(&message.as_bytes(), format!("{}:{}", remote_ip, PORT)).await?;
                    },
                    v if v.starts_with("ACK ") => {
                        match other_ip_rec {
                            Some(g) => {
                                let remote_ip: Ipv4Addr = v.strip_prefix("ACK ").unwrap().strip_suffix("\n").unwrap().parse().unwrap_or(Ipv4Addr::new(127,0,0,1));
                                if remote_ip == g {
                                    shared_state.direct_working.store(true, Ordering::Relaxed);
                                }
                            },
                            None => {}
                        }
                    },
                    v if v.starts_with(&format!("{} ", CODE)) => {
                        match other_ip_rec {
                            Some(_) => {},
                            None => {
                                other_ip_rec = match v.strip_prefix(&format!("{} ",CODE)).unwrap().strip_suffix("\n").unwrap().parse::<Ipv4Addr>() {
                                    Ok(g) => {
                                        let _ = shared_state.other_link_ip_sender.send(Some(g));
                                        Some(g)
                                    },
                                    Err(e) => {println!("something is wrong when converting a str to Ipv4Addr: {}", e); None}
                                } 
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}


async fn broadcast_task(sock: tokio::net::UdpSocket, msg: Vec<u8>, cancel: CancellationToken) -> io::Result<()>{
    let mut msg_as_string = String::from_utf8(msg.clone()).unwrap_or("Didn't work".to_string());
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = sock.send_to(&msg, format!("255.255.255.255:{}",PORT)) => {
                print!("Send broadcast: {}", msg_as_string);
                sleep(Duration::from_secs(5)).await;
            }
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

    let token = CancellationToken::new();
    let direct_working = AtomicBool::new(false);
    let (tx, mut rx) = watch::channel(None);

    let shared_state: Arc<SharedState> = Arc::new(SharedState{finished: token.clone(), direct_working: direct_working, other_link_ip_sender: tx, other_link_ip_receiver: rx});
    
    let message = format!("{} {}\n", CODE, ipaddr);

    println!("{}", ipaddr);
    let mut tasks = Vec::new();
    let shared_state_clone = shared_state.clone();
    let token_clone = token.clone();
    tasks.push(tokio::spawn(async move {broadcast_task(sock_broadcast, message.as_bytes().to_vec(), token_clone).await}));
    
    let token_clone = token.clone();
    let shared_state_clone = shared_state.clone();
    tasks.push(tokio::spawn(async move {listen_all_messages(sock_listen, shared_state_clone, ipaddr.clone()).await}));
    
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