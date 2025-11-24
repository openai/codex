#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi does not support panic recovery

use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

use futures::executor::block_on;

use std::net::TcpListener;

#[test]
#[should_panic(
    expected = "there is no reactor running, must be called from the context of a Tokio 1.x runtime"
)]
fn timeout_panics_when_no_tokio_context() {
    block_on(timeout_value());
}

#[test]
#[should_panic(
    expected = "there is no reactor running, must be called from the context of a Tokio 1.x runtime"
)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn panics_when_no_reactor() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap();
    block_on(TcpStream::connect(&addr)).unwrap();
}

async fn timeout_value() {
    let (_tx, rx) = oneshot::channel::<()>();
    let dur = Duration::from_millis(10);
    let _ = timeout(dur, rx).await;
}

#[test]
#[should_panic(
    expected = "there is no reactor running, must be called from the context of a Tokio 1.x runtime"
)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn io_panics_when_no_tokio_context() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

    listener.set_nonblocking(true).unwrap();

    let _ = tokio::net::TcpListener::from_std(listener);
}
