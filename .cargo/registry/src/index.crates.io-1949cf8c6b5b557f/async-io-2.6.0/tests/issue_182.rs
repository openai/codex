//! https://github.com/smol-rs/async-io/issues/182

use async_io::Async;
use std::net::{TcpStream, ToSocketAddrs};

#[test]
fn networking_initialized() {
    let address = match ToSocketAddrs::to_socket_addrs(&("google.com", 80)) {
        Ok(mut addrs) => addrs.next().unwrap(),
        Err(err) => {
            eprintln!("Got error {err} when looking up google.com, exiting test early.");
            return;
        }
    };

    // Make sure we can access the host normally.
    if TcpStream::connect(address).is_err() {
        return;
    }

    async_io::block_on(async move {
        let _ = Async::<TcpStream>::connect(address).await.unwrap();
    });
}
