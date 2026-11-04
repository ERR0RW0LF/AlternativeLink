use std::{io, net::Ipv4Addr, process::exit, sync::{Arc, atomic::AtomicBool}};

use tokio::{select, sync::{Notify, watch::{self, Receiver, Sender}}};
use tokio_util::sync::CancellationToken;
use tokio::net::UdpSocket;

use alternative_link_core::{SharedForSenders, SharedState, discovery::get_ipaddr, engine::{self, EngineArgs, broadcast_task, direct_comms_check_task, listen_all_messages}, prompts::{list_prompt, number_prompt, yes_no_prompt}, protocol::Code};

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

const BROADCAST_INTERVAL: u64 = 5;

type IpReceiver = Receiver<Option<Ipv4Addr>>;
type IpSender = Sender<Option<Ipv4Addr>>;


#[tokio::main]
async fn main() -> io::Result<()>{
    let ipaddr: Ipv4Addr = match get_ipaddr() {
        Ok(Some(ip)) => {ip},
        Ok(None) => {eprintln!("No ip set."); exit(1)}
        Err(e) => {eprintln!("Error when setting ip: {}",e);exit(1)}
    };
    
    println!("IP was set to {}", ipaddr); // TODO Decide if this is to be kept fo the final version

    let mut code: usize = 1337;
    let mut port: u16 = 1337;

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

    let sock_listen: UdpSocket = UdpSocket::bind(format!("0.0.0.0:{}",port) as String).await?;

    if let Err(_e) = sock_listen.set_broadcast(true) {
    }

    let sock_sender: UdpSocket = UdpSocket::bind(format!("{}:0",ipaddr) as String).await?;

    if let Err(_e) = sock_sender.set_broadcast(true) {
    }

    let token = CancellationToken::new();

    let shared_for_senders = Arc::new(SharedForSenders {
        sock: sock_sender,
        cancel: token.clone(),
    });

    let direct_working = Arc::new(AtomicBool::new(false));

    let (tx, rx) = watch::channel(None);

    let direct_working_notify = Arc::new(Notify::new());


    let shared_state: Arc<SharedState> = Arc::new(SharedState { finished: token.clone(), other_link_ip: tx });

    let message = format!("{} {}\n", code, ipaddr);

    let engine_args: EngineArgs = EngineArgs { 
        code: code.to_string(), 
        port, 
        json: false, 
        auto_test: false, 
        no_stop: false
    };


    let mut tasks = Vec::new();
    let shared_for_senders_clone: Arc<SharedForSenders> = shared_for_senders.clone();
    let engine_args_clone = engine_args.clone();
    tasks.push(tokio::spawn(async move {broadcast_task(shared_for_senders_clone, message.as_bytes().to_vec(), BROADCAST_INTERVAL, engine_args_clone).await}));
    
    let shared_state_clone = shared_state.clone();
    let direct_working_clone = direct_working.clone();
    let direct_working_notify_clone = direct_working_notify.clone();
    let engine_args_clone = engine_args.clone();
    tasks.push(tokio::spawn(async move {listen_all_messages(sock_listen, shared_state_clone, ipaddr, direct_working_clone, direct_working_notify_clone, engine_args_clone).await}));

    let mut other_link_ip_rx_clone: IpReceiver = rx.clone();
    loop {
        select! {
            i = other_link_ip_rx_clone.changed() => {
                if i.is_err() { break; }

                match *other_link_ip_rx_clone.borrow() {
                    Some(_) => {
                        break;
                    },
                    None => { continue; }
                }
            },
            _ = token.cancelled() => break,
        }
    }

    if !token.is_cancelled() {
        let other_link_ip_rx_clone: IpReceiver = rx.clone();
        let engine_args_clone = engine_args.clone();
        
        // Ask user if he wants to quite the program or test the direct connection
        match list_prompt("Test Direct?", vec!["Quit", "Test"], Some(0)) {
            Some(0) => {
                token.cancel();
            },
            Some(1) => {
                tasks.push(tokio::spawn(async move {direct_comms_check_task(shared_for_senders, other_link_ip_rx_clone, direct_working, direct_working_notify, ipaddr, 20, engine_args_clone).await}));
            },
            Some(_) => {}
            None => {}
        }
    }

    Ok(())

}

