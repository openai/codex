//! This example intends to use the smallest amount of code to make a simple QUIC connection.
//!
//! Checkout the `README.md` for guidance.

use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

mod common;
use common::{make_client_endpoint, make_server_endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
    let (endpoint, server_cert) = make_server_endpoint(server_addr)?;
    // accept a single connection
    let endpoint2 = endpoint.clone();
    tokio::spawn(async move {
        let incoming_conn = endpoint2.accept().await.unwrap();
        let conn = incoming_conn.await.unwrap();
        println!(
            "[server] connection accepted: addr={}",
            conn.remote_address()
        );
        // Dropping all handles associated with a connection implicitly closes it
    });

    let endpoint = make_client_endpoint("0.0.0.0:0".parse().unwrap(), &[&server_cert])?;
    // connect to server
    let connection = endpoint
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    println!("[client] connected: addr={}", connection.remote_address());

    // Waiting for a stream will complete with an error when the server closes the connection
    let _ = connection.accept_uni().await;

    // Make sure the server has a chance to clean up
    endpoint.wait_idle().await;

    Ok(())
}
