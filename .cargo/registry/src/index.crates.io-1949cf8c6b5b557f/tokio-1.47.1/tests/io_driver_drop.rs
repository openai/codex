#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi does not support bind

use tokio::net::TcpListener;
use tokio::runtime;
use tokio_test::{assert_err, assert_pending, assert_ready, task};

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn tcp_doesnt_block() {
    let rt = rt();

    let listener = {
        let _enter = rt.enter();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

        listener.set_nonblocking(true).unwrap();

        TcpListener::from_std(listener).unwrap()
    };

    drop(rt);

    let mut task = task::spawn(async move {
        assert_err!(listener.accept().await);
    });

    assert_ready!(task.poll());
}

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn drop_wakes() {
    let rt = rt();

    let listener = {
        let _enter = rt.enter();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

        listener.set_nonblocking(true).unwrap();

        TcpListener::from_std(listener).unwrap()
    };

    let mut task = task::spawn(async move {
        assert_err!(listener.accept().await);
    });

    assert_pending!(task.poll());

    drop(rt);

    assert!(task.is_woken());
    assert_ready!(task.poll());
}

fn rt() -> runtime::Runtime {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
