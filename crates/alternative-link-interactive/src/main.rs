use std::{net::{Ipv4Addr}, process::exit};

use tokio::{select, sync::{Notify, watch::{self, Receiver}}, time::sleep,};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;

use alternative_link_core::{discovery::get_ipaddr, prompts::{number_prompt, yes_no_prompt}, protocol::Code};

/*
Program flow:
Set IP

Change
Configuration?
No Yes

On No
Just start using default values

On Yes
Pairing Code
1-3-3-7
<- Back  Save

Port
1337
<- Back  Save

Save as default?
No   Yes

Starts AlternativeLink

*/



#[tokio::main]
async fn main() {
    let ipaddr: Ipv4Addr = match get_ipaddr() {
        Ok(Some(ip)) => {ip},
        Ok(None) => {eprintln!("No ip set."); exit(1)}
        Err(e) => {eprintln!("Error when setting ip: {}",e);exit(1)}
    };
    
    println!("IP was set to {}", ipaddr); // TODO Decide if this is to be kept fo the final version

    let mut code: usize = 1337;
    let mut port: usize = 1337;

    match yes_no_prompt("Change Configuration", Some(false)) {
        Some(true) => {
            code = if let Some(i) = number_prompt("Pairing Code", Some(code.try_into().unwrap())) {
                i.try_into().unwrap()
            } else {
                code
            };

            port = if let Some(i) = number_prompt("Pairing Code", Some(port.try_into().unwrap())) {
                i.try_into().unwrap()
            } else {
                port
            };

            match yes_no_prompt("Save as default?", Some(false)) {
                Some(true) => {todo!("needs implementation of persistent settings")},
                _ => {},
            }

        },
        Some(false) => {},
        None => {eprintln!("Something definitely has gone wrong with the \"Change Configuration\" prompt.");exit(1)}
    }

    let code = Code::new(code);

    println!("Code was set to {}", code); // TODO Decide if this is to be kept fo the final version
    println!("Port was set to {}", port); // TODO Decide if this is to be kept fo the final version

    let sock_listen: UdpSocket = UdpSocket::bind(format!("0.0.0.0:{}",args.port) as String).await?;

    if let Err(e) = sock_listen.set_broadcast(true) {
    }

    let sock_sender: UdpSocket = UdpSocket::bind(format!("{}:0",ipaddr) as String).await?;

    if let Err(e) = sock_sender.set_broadcast(true) {
    }

    let token = CancellationToken::new();


}

