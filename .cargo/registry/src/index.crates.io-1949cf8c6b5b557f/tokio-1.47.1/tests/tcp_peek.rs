#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support bind
                                                                   // No `socket` on miri.

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use tokio_test::assert_ok;

use std::thread;
use std::{io::Write, net};

#[tokio::test]
async fn peek() {
    let listener = net::TcpListener::bind("127.0.0.1:0").unwrap();

    let addr = listener.local_addr().unwrap();
    let t = thread::spawn(move || assert_ok!(listener.accept()).0);

    let left = net::TcpStream::connect(addr).unwrap();

    left.set_nonblocking(true).unwrap();

    let mut right = t.join().unwrap();

    right.set_nonblocking(true).unwrap();

    let _ = right.write(&[1, 2, 3, 4]).unwrap();

    let mut left: TcpStream = left.try_into().unwrap();
    let mut buf = [0u8; 16];
    let n = assert_ok!(left.peek(&mut buf).await);
    assert_eq!([1, 2, 3, 4], buf[..n]);

    let n = assert_ok!(left.read(&mut buf).await);
    assert_eq!([1, 2, 3, 4], buf[..n]);
}
