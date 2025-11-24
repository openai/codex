#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support panic recovery or bind

use tokio::net::TcpListener;

use std::net;

#[test]
#[should_panic]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn no_runtime_panics_binding_net_tcp_listener() {
    let listener = net::TcpListener::bind("127.0.0.1:0").expect("failed to bind listener");
    let _ = TcpListener::try_from(listener);
}
