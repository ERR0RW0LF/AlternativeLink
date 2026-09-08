use std::net::Ipv4Addr;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub mod protocol;
pub mod engine;
pub mod discovery;
pub mod helper;

pub struct SharedState {
    pub finished: CancellationToken,
    pub other_link_ip: watch::Sender<Option<Ipv4Addr>>,
}

pub struct SharedForSenders {
    pub sock: tokio::net::UdpSocket,
    pub cancel: CancellationToken
}
