#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support bind
                                                                   // No `socket` on miri.

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_test::assert_ok;

use futures::join;

#[tokio::test]
async fn connect_v4() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    assert!(addr.is_ipv4());

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (socket, addr) = assert_ok!(srv.accept().await);
        assert_eq!(addr, assert_ok!(socket.peer_addr()));
        assert_ok!(tx.send(socket));
    });

    let mine = assert_ok!(TcpStream::connect(&addr).await);
    let theirs = assert_ok!(rx.await);

    assert_eq!(
        assert_ok!(mine.local_addr()),
        assert_ok!(theirs.peer_addr())
    );
    assert_eq!(
        assert_ok!(theirs.local_addr()),
        assert_ok!(mine.peer_addr())
    );
}

#[tokio::test]
async fn connect_v6() {
    let srv = assert_ok!(TcpListener::bind("[::1]:0").await);
    let addr = assert_ok!(srv.local_addr());
    assert!(addr.is_ipv6());

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (socket, addr) = assert_ok!(srv.accept().await);
        assert_eq!(addr, assert_ok!(socket.peer_addr()));
        assert_ok!(tx.send(socket));
    });

    let mine = assert_ok!(TcpStream::connect(&addr).await);
    let theirs = assert_ok!(rx.await);

    assert_eq!(
        assert_ok!(mine.local_addr()),
        assert_ok!(theirs.peer_addr())
    );
    assert_eq!(
        assert_ok!(theirs.local_addr()),
        assert_ok!(mine.peer_addr())
    );
}

#[tokio::test]
async fn connect_addr_ip_string() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    let addr = format!("127.0.0.1:{}", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(addr).await);
    };

    join!(server, client);
}

#[tokio::test]
async fn connect_addr_ip_str_slice() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    let addr = format!("127.0.0.1:{}", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr[..]).await);
    };

    join!(server, client);
}

#[tokio::test]
async fn connect_addr_host_string() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    let addr = format!("localhost:{}", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(addr).await);
    };

    join!(server, client);
}

#[tokio::test]
async fn connect_addr_ip_port_tuple() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    let addr = (addr.ip(), addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr).await);
    };

    join!(server, client);
}

#[tokio::test]
async fn connect_addr_ip_str_port_tuple() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    let addr = ("127.0.0.1", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr).await);
    };

    join!(server, client);
}

#[tokio::test]
async fn connect_addr_host_str_port_tuple() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());
    let addr = ("localhost", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr).await);
    };

    join!(server, client);
}

/*
 * TODO: bring this back once TCP exposes HUP again
 *
#[cfg(target_os = "linux")]
mod linux {
    use tokio::net::{TcpListener, TcpStream};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_test::assert_ok;

    use mio::unix::UnixReady;

    use futures_util::future::poll_fn;
    use std::io::Write;
    use std::time::Duration;
    use std::{net, thread};

    #[tokio::test]
    fn poll_hup() {
        let addr = assert_ok!("127.0.0.1:0".parse());
        let mut srv = assert_ok!(TcpListener::bind(&addr));
        let addr = assert_ok!(srv.local_addr());

        tokio::spawn(async move {
            let (mut client, _) = assert_ok!(srv.accept().await);
            assert_ok!(client.set_linger(Some(Duration::from_millis(0))));
            assert_ok!(client.write_all(b"hello world").await);

            // TODO: Drop?
        });

        /*
        let t = thread::spawn(move || {
            let mut client = assert_ok!(srv.accept()).0;
            client.set_linger(Some(Duration::from_millis(0))).unwrap();
            client.write(b"hello world").unwrap();
            thread::sleep(Duration::from_millis(200));
        });
        */

        let mut stream = assert_ok!(TcpStream::connect(&addr).await);

        // Poll for HUP before reading.
        future::poll_fn(|| stream.poll_read_ready(UnixReady::hup().into()))
            .wait()
            .unwrap();

        // Same for write half
        future::poll_fn(|| stream.poll_write_ready())
            .wait()
            .unwrap();

        let mut buf = vec![0; 11];

        // Read the data
        future::poll_fn(|| stream.poll_read(&mut buf))
            .wait()
            .unwrap();

        assert_eq!(b"hello world", &buf[..]);

        t.join().unwrap();
    }
}
*/
