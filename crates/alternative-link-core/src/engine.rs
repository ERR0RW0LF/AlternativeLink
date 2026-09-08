use std::{io, net::Ipv4Addr, process::exit, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};

use tokio::{sync::{Notify, watch}, time::sleep};
use tracing::{debug, info, trace, warn};

use crate::{SharedForSenders, SharedState, cli::Cli, protocol::{Code, Message}};

const CODE: usize = 1337;
const PORT: u16 = 1337;

pub async fn listen_all_messages(
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
                        sock.send_to(message.as_bytes(), format!("{}:{}", m, args.port)).await?;
                    },
                    Message::Ack(m) => {
                        debug!("Got ACK from {}. other_ip_rec is {:?}", m, other_ip_rec);
                        if let Some(g) = other_ip_rec
                            && m == g {
                                direct_working.store(true, Ordering::Relaxed);
                                direct_working_notify.notify_waiters();
                            }
                    },
                }
            }
        }
    }
    Ok(())
}


pub async fn broadcast_task(shared_state: Arc<SharedForSenders>, msg: Vec<u8>, broadcast_interval: u64, args: Cli) -> io::Result<()> {
    info!("Broadcasting every {}s", broadcast_interval);
    loop {
        tokio::select! {
            _ = shared_state.cancel.cancelled() => break,
            _ = shared_state.sock.send_to(&msg, format!("255.255.255.255:{}",args.port)) => {
                trace!("Send a broadcast, now waiting for {}s", broadcast_interval);
                sleep(Duration::from_secs(broadcast_interval)).await;
            }
        }
    }
    Ok(())
}

fn report_status(args: &Cli, message: &str, json_fields: &[(&str, &str)]) {
    if args.json {
        let fields: Vec<String> = json_fields
            .iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
            .collect();
        println!("{{\"status\":\"{}\",{}}}", message, fields.join(","));
    } else {
        println!("{}", message);
    }
}

pub async fn direct_comms_check_task(
    shared_state: Arc<SharedForSenders>, ip_receiver: watch::Receiver<Option<Ipv4Addr>>, 
    direct_working: Arc<AtomicBool>, direct_working_notify: Arc<Notify>,
    ipaddr: Ipv4Addr,
    max_tries: u64, args: Cli
) -> io::Result<()> {
    trace!("Test worked {} {}", ip_receiver.borrow().unwrap(), shared_state.cancel.is_cancelled());
    let mut probe_interval = tokio::time::interval(Duration::from_secs(5));
    
    let mut timeout_counter: u64 = 0;

    loop {
        let is_working = direct_working.load(Ordering::Relaxed);
        if is_working {
            let ip= if let Some(ipa) = *ip_receiver.borrow() {
                ipa
            } else {
                continue;
            };
            report_status(
                &args, 
                "Connection working", 
                &[("peer_ip", &ip.to_string())],
            );
            shared_state.cancel.cancel();
            break;
        }

        tokio::select! {
            _ = shared_state.cancel.cancelled() => break,
            _ = direct_working_notify.notified() => {
                if direct_working.load(Ordering::Relaxed) {
                    let ip = if let Some(ipa) = *ip_receiver.borrow() {
                        ipa
                    } else {
                        continue;
                    };
                    report_status(
                        &args, 
                        "Connection working", 
                        &[("peer_ip", &ip.to_string())],
                    );
                    shared_state.cancel.cancel();
                    break;
                }
            },
            _ = probe_interval.tick(), if !is_working => {
                let ip_opt = *ip_receiver.borrow();
                if let Some(ip) = ip_opt {
                    match timeout_counter {
                        i if timeout_counter > max_tries => { 
                            report_status(
                                &args, 
                                &format!("Didn't get a Ack in time, exhausted retries ({}/{}). Exiting.", i,  max_tries),
                                &[("retries", &i.to_string()),("max_retries", &max_tries.to_string())]
                            ); 
                            exit(0)
                        }
                        i if timeout_counter > 0 => { 
                            report_status(
                                &args, 
                                &format!("Retrying {}/{}", i, max_tries),
                                &[("retries", &i.to_string()),("max_retries", &max_tries.to_string())]
                            ) 
                        },
                        _ => {},
                    }
                    trace!("Sending Ping");
                    if let Err(e) = shared_state.sock.send_to(format!("PING {}\n", ipaddr).as_bytes(), format!("{}:{}",ip,args.port)).await {
                        warn!("Error sending with: {}", e)
                    };
                    timeout_counter += 1;
                }
            },
        }
    }
    Ok(())
}
