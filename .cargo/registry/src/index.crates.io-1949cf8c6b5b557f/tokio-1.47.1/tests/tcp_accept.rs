#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support bind
                                                                   // No `socket` on miri.

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_test::assert_ok;

use std::io;
use std::net::{IpAddr, SocketAddr};

macro_rules! test_accept {
    ($(($ident:ident, $target:expr),)*) => {
        $(
            #[tokio::test]
            async fn $ident() {
                let listener = assert_ok!(TcpListener::bind($target).await);
                let addr = listener.local_addr().unwrap();

                let (tx, rx) = oneshot::channel();

                tokio::spawn(async move {
                    let (socket, _) = assert_ok!(listener.accept().await);
                    assert_ok!(tx.send(socket));
                });

                let cli = assert_ok!(TcpStream::connect(&addr).await);
                let srv = assert_ok!(rx.await);

                assert_eq!(cli.local_addr().unwrap(), srv.peer_addr().unwrap());
            }
        )*
    }
}

test_accept! {
    (ip_str, "127.0.0.1:0"),
    (host_str, "localhost:0"),
    (socket_addr, "127.0.0.1:0".parse::<SocketAddr>().unwrap()),
    (str_port_tuple, ("127.0.0.1", 0)),
    (ip_port_tuple, ("127.0.0.1".parse::<IpAddr>().unwrap(), 0)),
}

use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};
use std::task::{Context, Poll};
use tokio_stream::{Stream, StreamExt};

struct TrackPolls<'a> {
    npolls: Arc<AtomicUsize>,
    listener: &'a mut TcpListener,
}

impl<'a> Stream for TrackPolls<'a> {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.npolls.fetch_add(1, SeqCst);
        self.listener.poll_accept(cx).map(Some)
    }
}

#[tokio::test]
async fn no_extra_poll() {
    let mut listener = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = listener.local_addr().unwrap();

    let (tx, rx) = oneshot::channel();
    let (accepted_tx, mut accepted_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut incoming = TrackPolls {
            npolls: Arc::new(AtomicUsize::new(0)),
            listener: &mut listener,
        };
        assert_ok!(tx.send(Arc::clone(&incoming.npolls)));
        while incoming.next().await.is_some() {
            accepted_tx.send(()).unwrap();
        }
    });

    let npolls = assert_ok!(rx.await);
    tokio::task::yield_now().await;

    // should have been polled exactly once: the initial poll
    assert_eq!(npolls.load(SeqCst), 1);

    let _ = assert_ok!(TcpStream::connect(&addr).await);
    accepted_rx.recv().await.unwrap();

    // should have been polled twice more: once to yield Some(), then once to yield Pending
    assert_eq!(npolls.load(SeqCst), 1 + 2);
}

#[tokio::test]
async fn accept_many() {
    use std::future::{poll_fn, Future};
    use std::sync::atomic::AtomicBool;

    const N: usize = 50;

    let listener = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let listener = Arc::new(listener);
    let addr = listener.local_addr().unwrap();
    let connected = Arc::new(AtomicBool::new(false));

    let (pending_tx, mut pending_rx) = mpsc::unbounded_channel();
    let (notified_tx, mut notified_rx) = mpsc::unbounded_channel();

    for _ in 0..N {
        let listener = listener.clone();
        let connected = connected.clone();
        let pending_tx = pending_tx.clone();
        let notified_tx = notified_tx.clone();

        tokio::spawn(async move {
            let accept = listener.accept();
            tokio::pin!(accept);

            let mut polled = false;

            poll_fn(|cx| {
                if !polled {
                    polled = true;
                    assert!(Pin::new(&mut accept).poll(cx).is_pending());
                    pending_tx.send(()).unwrap();
                    Poll::Pending
                } else if connected.load(SeqCst) {
                    notified_tx.send(()).unwrap();
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;

            pending_tx.send(()).unwrap();
        });
    }

    // Wait for all tasks to have polled at least once
    for _ in 0..N {
        pending_rx.recv().await.unwrap();
    }

    // Establish a TCP connection
    connected.store(true, SeqCst);
    let _sock = TcpStream::connect(addr).await.unwrap();

    // Wait for all notifications
    for _ in 0..N {
        notified_rx.recv().await.unwrap();
    }
}
