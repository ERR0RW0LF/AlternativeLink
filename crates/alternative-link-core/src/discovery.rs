use std::{io::{Write, stdin, stdout}, net::Ipv4Addr};

use if_addrs::IfAddr;
use tracing::warn;

pub fn validate_interface(ipaddr: Ipv4Addr) -> bool{
    let mut possible_ips: Vec<Ipv4Addr> = Vec::new();
    for iface in if_addrs::get_if_addrs().unwrap() {
        if let IfAddr::V4(ifv4_addr) = iface.addr
            && !ifv4_addr.is_loopback() {
                possible_ips.push(ifv4_addr.ip);
            }
    }
    possible_ips.contains(&ipaddr)
}


pub fn get_ipaddr() -> Option<Ipv4Addr>{
    let mut possible_ips: Vec<Ipv4Addr> = Vec::new();
    for iface in if_addrs::get_if_addrs().unwrap() {
        if let IfAddr::V4(ifv4_addr) = iface.addr
            && !ifv4_addr.is_loopback() {
                possible_ips.push(ifv4_addr.ip);
            }
    }
    for (i, item) in possible_ips.iter().enumerate() {
        println!("- {number:0>2}: {}",item, number=i)
    }
    let mut buffer = String::new();
    let stdin = stdin();
    print!("Please chose the ip to use (num): ");
    if let Err(e) = stdout().flush() {
        warn!("Problem whilst flushing stdout: {}", e)
    }
    stdin.read_line(&mut buffer).expect("Reading your input didn't work");
    let choice: usize = buffer.trim().parse::<usize>().expect("Next time please a number");
    possible_ips.get(choice).copied()
}