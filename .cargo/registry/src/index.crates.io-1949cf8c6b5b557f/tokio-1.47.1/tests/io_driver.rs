#![warn(rust_2018_idioms)]
// Wasi does not support panic recovery or threading
#![cfg(all(feature = "full", not(target_os = "wasi")))]

use tokio::net::TcpListener;
use tokio::runtime;
use tokio_test::{assert_ok, assert_pending};

use futures::task::{waker_ref, ArcWake};
use std::future::Future;
use std::net::TcpStream;
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex};
use std::task::Context;

struct Task<T> {
    future: Mutex<Pin<Box<T>>>,
}

impl<T: Send> ArcWake for Task<T> {
    fn wake_by_ref(_: &Arc<Self>) {
        // Do nothing...
    }
}

impl<T> Task<T> {
    fn new(future: T) -> Task<T> {
        Task {
            future: Mutex::new(Box::pin(future)),
        }
    }
}

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn test_drop_on_notify() {
    // When the reactor receives a kernel notification, it notifies the
    // task that holds the associated socket. If this notification results in
    // the task being dropped, the socket will also be dropped.
    //
    // Previously, there was a deadlock scenario where the reactor, while
    // notifying, held a lock and the task being dropped attempted to acquire
    // that same lock in order to clean up state.
    //
    // To simulate this case, we create a fake executor that does nothing when
    // the task is notified. This simulates an executor in the process of
    // shutting down. Then, when the task handle is dropped, the task itself is
    // dropped.

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (addr_tx, addr_rx) = mpsc::channel();

    // Define a task that just drains the listener
    let task = Arc::new(Task::new(async move {
        // Create a listener
        let listener = assert_ok!(TcpListener::bind("127.0.0.1:0").await);

        // Send the address
        let addr = listener.local_addr().unwrap();
        addr_tx.send(addr).unwrap();

        loop {
            let _ = listener.accept().await;
        }
    }));

    {
        let _enter = rt.enter();
        let waker = waker_ref(&task);
        let mut cx = Context::from_waker(&waker);
        assert_pending!(task.future.lock().unwrap().as_mut().poll(&mut cx));
    }

    // Get the address
    let addr = addr_rx.recv().unwrap();

    drop(task);

    // Establish a connection to the acceptor
    let _s = TcpStream::connect(addr).unwrap();

    // Force the reactor to turn
    rt.block_on(async {});
}

#[test]
#[should_panic(
    expected = "A Tokio 1.x context was found, but IO is disabled. Call `enable_io` on the runtime builder to enable IO."
)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn panics_when_io_disabled() {
    let rt = runtime::Builder::new_current_thread().build().unwrap();

    rt.block_on(async {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

        listener.set_nonblocking(true).unwrap();

        let _ = tokio::net::TcpListener::from_std(listener);
    });
}
