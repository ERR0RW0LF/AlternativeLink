use std::net::Ipv4Addr;

use if_addrs::IfAddr;

use crate::{errors::DiscoveryError, prompts};


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

/// Prints a numbered list of all the ip4vs then asks the user to chose the one to be used
pub fn get_ipaddr() -> Result<Option<Ipv4Addr>, DiscoveryError> {
    let possible_ips: Vec<Ipv4Addr> = get_ip_addresses()?;

    let possible_ips_string: Vec<String> = possible_ips.iter()
        .map(|ip| ip.to_string())
        .collect();

    let answers: Vec<&str> = possible_ips_string.iter()
        .map(|s| s.as_str())
        .collect();
    
    match prompts::list_prompt("What ip should be used?", answers, None) {
        Some(choice_index) => {Ok(possible_ips.get(choice_index).copied())},
        None => {Err(DiscoveryError::ChoiceError { os_error: "nope".to_string() })},
    }
}