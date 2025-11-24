#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support bind
                                                                   // No `socket` on miri.

use std::io::{Error, ErrorKind, Result};
use std::io::{Read, Write};
use std::{net, thread};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::try_join;

#[tokio::test]
async fn split() -> Result<()> {
    const MSG: &[u8] = b"split";

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let (stream1, (mut stream2, _)) = try_join! {
        TcpStream::connect(&addr),
        listener.accept(),
    }?;
    let (mut read_half, mut write_half) = stream1.into_split();

    let ((), (), ()) = try_join! {
        async {
            let len = stream2.write(MSG).await?;
            assert_eq!(len, MSG.len());

            let mut read_buf = vec![0u8; 32];
            let read_len = stream2.read(&mut read_buf).await?;
            assert_eq!(&read_buf[..read_len], MSG);
            Result::Ok(())
        },
        async {
            let len = write_half.write(MSG).await?;
            assert_eq!(len, MSG.len());
            Ok(())
        },
        async {
            let mut read_buf = [0u8; 32];
            let peek_len1 = read_half.peek(&mut read_buf[..]).await?;
            let peek_len2 = read_half.peek(&mut read_buf[..]).await?;
            assert_eq!(peek_len1, peek_len2);

            let read_len = read_half.read(&mut read_buf[..]).await?;
            assert_eq!(peek_len1, read_len);
            assert_eq!(&read_buf[..read_len], MSG);
            Ok(())
        },
    }?;

    Ok(())
}

#[tokio::test]
async fn reunite() -> Result<()> {
    let listener = net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let handle = thread::spawn(move || {
        drop(listener.accept().unwrap());
        drop(listener.accept().unwrap());
    });

    let stream1 = TcpStream::connect(&addr).await?;
    let (read1, write1) = stream1.into_split();

    let stream2 = TcpStream::connect(&addr).await?;
    let (_, write2) = stream2.into_split();

    let read1 = match read1.reunite(write2) {
        Ok(_) => panic!("Reunite should not succeed"),
        Err(err) => err.0,
    };

    read1.reunite(write1).expect("Reunite should succeed");

    handle.join().unwrap();
    Ok(())
}

/// Test that dropping the write half actually closes the stream.
#[tokio::test]
async fn drop_write() -> Result<()> {
    const MSG: &[u8] = b"split";

    let listener = net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(MSG).unwrap();

        let mut read_buf = [0u8; 32];
        let res = match stream.read(&mut read_buf) {
            Ok(0) => Ok(()),
            Ok(len) => Err(Error::new(
                ErrorKind::Other,
                format!("Unexpected read: {len} bytes."),
            )),
            Err(err) => Err(err),
        };

        drop(stream);

        res
    });

    let stream = TcpStream::connect(&addr).await?;
    let (mut read_half, write_half) = stream.into_split();

    let mut read_buf = [0u8; 32];
    let read_len = read_half.read(&mut read_buf[..]).await?;
    assert_eq!(&read_buf[..read_len], MSG);

    // drop it while the read is in progress
    std::thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(10));
        drop(write_half);
    });

    match read_half.read(&mut read_buf[..]).await {
        Ok(0) => {}
        Ok(len) => panic!("Unexpected read: {len} bytes."),
        Err(err) => panic!("Unexpected error: {err}."),
    }

    handle.join().unwrap().unwrap();
    Ok(())
}
