use std::{io::{Write, stdin, stdout}, net::Ipv4Addr};

use if_addrs::IfAddr;
use tracing::warn;

use crate::errors::DiscoveryError;


fn get_ip_addresses() -> Result<Vec<Ipv4Addr>, DiscoveryError> {
    let mut possible_ips: Vec<Ipv4Addr> = Vec::new();
    match if_addrs::get_if_addrs() {
        Ok(i) => {
            for iface in i {
                if let IfAddr::V4(ifv4_addr) = iface.addr 
                    && !ifv4_addr.is_loopback() {
                        possible_ips.push(ifv4_addr.ip);
                    }
            }
        },
        Err(e) => {
            return Err(DiscoveryError::UnableToEnumInterfaces {
                os_error: e.to_string(), 
            });
        }
    }

    Ok(possible_ips)
}


pub fn validate_interface(ipaddr: Ipv4Addr) -> Result<bool, DiscoveryError>{
    let possible_ips = get_ip_addresses()?;
    Ok(possible_ips.contains(&ipaddr))
}


pub fn get_ipaddr() -> Result<Option<Ipv4Addr>, DiscoveryError> {
    let possible_ips: Vec<Ipv4Addr> = get_ip_addresses()?;
    for (i, item) in possible_ips.iter().enumerate() {
        println!("- {number:0>2}: {}",item, number=i)
    }
    let mut buffer = String::new();
    let stdin = stdin();
    print!("Please chose the ip to use (num): ");
    if let Err(e) = stdout().flush() {
        warn!("Problem whilst flushing stdout: {}", e)
    }
    if let Err(e) = stdin.read_line(&mut buffer) {return Err(DiscoveryError::StdInRead { os_error: e.to_string() });}
    let choice: usize = match buffer.trim().parse::<usize>() {
        Ok(i) => i,
        Err(e) => {return Err(DiscoveryError::InputParsing { os_error: e.to_string() })},
    };
    Ok(possible_ips.get(choice).copied())
}