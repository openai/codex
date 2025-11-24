#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support bind
                                                                   // No socket on miri.

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_test::assert_ok;

#[tokio::test]
async fn echo_server() {
    const ITER: usize = 1024;

    let (tx, rx) = oneshot::channel();

    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(srv.local_addr());

    let msg = "foo bar baz";
    tokio::spawn(async move {
        let mut stream = assert_ok!(TcpStream::connect(&addr).await);

        for _ in 0..ITER {
            // write
            assert_ok!(stream.write_all(msg.as_bytes()).await);

            // read
            let mut buf = [0; 11];
            assert_ok!(stream.read_exact(&mut buf).await);
            assert_eq!(&buf[..], msg.as_bytes());
        }

        assert_ok!(tx.send(()));
    });

    let (mut stream, _) = assert_ok!(srv.accept().await);
    let (mut rd, mut wr) = stream.split();

    let n = assert_ok!(io::copy(&mut rd, &mut wr).await);
    assert_eq!(n, (ITER * msg.len()) as u64);

    assert_ok!(rx.await);
}
