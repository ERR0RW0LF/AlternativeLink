use std::{io::{Write, stdin, stdout}, net::Ipv4Addr, sync::{Arc, atomic::AtomicBool}};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use if_addrs::IfAddr;

struct SharedState {
    finished: CancellationToken,
    direct_working: AtomicBool,
    other_link_ip: watch::Sender<Option<Ipv4Addr>>,
}




#[tokio::main]
async fn main() {
    let ipaddr = get_ipaddr().unwrap();
    let token = CancellationToken::new();
    let direct_working = AtomicBool::new(false);
    let (tx, mut rx) = watch::channel(None);

    let shared_state: Arc<SharedState> = Arc::new(SharedState{finished: token.clone(), direct_working: direct_working, other_link_ip: tx});


    println!("{}", ipaddr)
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