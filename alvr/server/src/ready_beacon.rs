use crate::SESSION_MANAGER;
use alvr_common::prelude::*;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

/// Accepts and immediately drops connections on the loopback interface. `alvr_launcher` connects
/// to this port to learn that the driver has loaded and the dashboard window is up. Nothing is
/// ever read from or written to these connections.
pub async fn run() -> StrResult {
    let port = SESSION_MANAGER
        .lock()
        .get()
        .to_settings()
        .connection
        .web_server_port;

    let listener =
        trace_err!(TcpListener::bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port)).await)?;

    loop {
        trace_err!(listener.accept().await)?;
    }
}
