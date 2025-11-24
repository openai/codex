#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support bind

use tokio::io::{AsyncReadExt, AsyncWriteExt, Interest};
use tokio::net::{TcpListener, TcpStream};
use tokio::try_join;
use tokio_test::task;
use tokio_test::{assert_ok, assert_pending, assert_ready_ok};

use std::future::poll_fn;
use std::io;
use std::task::Poll;
use std::time::Duration;

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn set_linger() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let stream = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();

    assert_ok!(stream.set_linger(Some(Duration::from_secs(1))));
    assert_eq!(stream.linger().unwrap().unwrap().as_secs(), 1);

    assert_ok!(stream.set_linger(None));
    assert!(stream.linger().unwrap().is_none());
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn try_read_write() {
    const DATA: &[u8] = b"this is some data to write to the socket";

    // Create listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    // Create socket pair
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let mut written = DATA.to_vec();

    // Track the server receiving data
    let mut readable = task::spawn(server.readable());
    assert_pending!(readable.poll());

    // Write data.
    client.writable().await.unwrap();
    assert_eq!(DATA.len(), client.try_write(DATA).unwrap());

    // The task should be notified
    while !readable.is_woken() {
        tokio::task::yield_now().await;
    }

    // Fill the write buffer using non-vectored I/O
    loop {
        // Still ready
        let mut writable = task::spawn(client.writable());
        assert_ready_ok!(writable.poll());

        match client.try_write(DATA) {
            Ok(n) => written.extend(&DATA[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                break;
            }
            Err(e) => panic!("error = {e:?}"),
        }
    }

    {
        // Write buffer full
        let mut writable = task::spawn(client.writable());
        assert_pending!(writable.poll());

        // Drain the socket from the server end using non-vectored I/O
        let mut read = vec![0; written.len()];
        let mut i = 0;

        while i < read.len() {
            server.readable().await.unwrap();

            match server.try_read(&mut read[i..]) {
                Ok(n) => i += n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("error = {e:?}"),
            }
        }

        assert_eq!(read, written);
    }

    written.clear();
    client.writable().await.unwrap();

    // Fill the write buffer using vectored I/O
    let data_bufs: Vec<_> = DATA.chunks(10).map(io::IoSlice::new).collect();
    loop {
        // Still ready
        let mut writable = task::spawn(client.writable());
        assert_ready_ok!(writable.poll());

        match client.try_write_vectored(&data_bufs) {
            Ok(n) => written.extend(&DATA[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                break;
            }
            Err(e) => panic!("error = {e:?}"),
        }
    }

    {
        // Write buffer full
        let mut writable = task::spawn(client.writable());
        assert_pending!(writable.poll());

        // Drain the socket from the server end using vectored I/O
        let mut read = vec![0; written.len()];
        let mut i = 0;

        while i < read.len() {
            server.readable().await.unwrap();

            let mut bufs: Vec<_> = read[i..]
                .chunks_mut(0x10000)
                .map(io::IoSliceMut::new)
                .collect();
            match server.try_read_vectored(&mut bufs) {
                Ok(n) => i += n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("error = {e:?}"),
            }
        }

        assert_eq!(read, written);
    }

    // Now, we listen for shutdown
    drop(client);

    loop {
        let ready = server.ready(Interest::READABLE).await.unwrap();

        if ready.is_read_closed() {
            return;
        } else {
            tokio::task::yield_now().await;
        }
    }
}

#[test]
fn buffer_not_included_in_future() {
    use std::mem;

    const N: usize = 4096;

    let fut = async {
        let stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();

        loop {
            stream.readable().await.unwrap();

            let mut buf = [0; N];
            let n = stream.try_read(&mut buf[..]).unwrap();

            if n == 0 {
                break;
            }
        }
    };

    let n = mem::size_of_val(&fut);
    assert!(n < 1000);
}

macro_rules! assert_readable_by_polling {
    ($stream:expr) => {
        assert_ok!(poll_fn(|cx| $stream.poll_read_ready(cx)).await);
    };
}

macro_rules! assert_not_readable_by_polling {
    ($stream:expr) => {
        poll_fn(|cx| {
            assert_pending!($stream.poll_read_ready(cx));
            Poll::Ready(())
        })
        .await;
    };
}

macro_rules! assert_writable_by_polling {
    ($stream:expr) => {
        assert_ok!(poll_fn(|cx| $stream.poll_write_ready(cx)).await);
    };
}

macro_rules! assert_not_writable_by_polling {
    ($stream:expr) => {
        poll_fn(|cx| {
            assert_pending!($stream.poll_write_ready(cx));
            Poll::Ready(())
        })
        .await;
    };
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn poll_read_ready() {
    let (mut client, mut server) = create_pair().await;

    // Initial state - not readable.
    assert_not_readable_by_polling!(server);

    // There is data in the buffer - readable.
    assert_ok!(client.write_all(b"ping").await);
    assert_readable_by_polling!(server);

    // Readable until calls to `poll_read` return `Poll::Pending`.
    let mut buf = [0u8; 4];
    assert_ok!(server.read_exact(&mut buf).await);
    assert_readable_by_polling!(server);
    read_until_pending(&mut server);
    assert_not_readable_by_polling!(server);

    // Detect the client disconnect.
    drop(client);
    assert_readable_by_polling!(server);
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn poll_write_ready() {
    let (mut client, server) = create_pair().await;

    // Initial state - writable.
    assert_writable_by_polling!(client);

    // No space to write - not writable.
    write_until_pending(&mut client);
    assert_not_writable_by_polling!(client);

    // Detect the server disconnect.
    drop(server);
    assert_writable_by_polling!(client);
}

async fn create_pair() -> (TcpStream, TcpStream) {
    let listener = assert_ok!(TcpListener::bind("127.0.0.1:0").await);
    let addr = assert_ok!(listener.local_addr());
    let (client, (server, _)) = assert_ok!(try_join!(TcpStream::connect(&addr), listener.accept()));
    (client, server)
}

fn read_until_pending(stream: &mut TcpStream) -> usize {
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total = 0;
    loop {
        match stream.try_read(&mut buf) {
            Ok(n) => total += n,
            Err(err) => {
                assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
                break;
            }
        }
    }
    total
}

fn write_until_pending(stream: &mut TcpStream) -> usize {
    let buf = vec![0u8; 1024 * 1024];
    let mut total = 0;
    loop {
        match stream.try_write(&buf) {
            Ok(n) => total += n,
            Err(err) => {
                assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
                break;
            }
        }
    }
    total
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn try_read_buf() {
    const DATA: &[u8] = b"this is some data to write to the socket";

    // Create listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    // Create socket pair
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let mut written = DATA.to_vec();

    // Track the server receiving data
    let mut readable = task::spawn(server.readable());
    assert_pending!(readable.poll());

    // Write data.
    client.writable().await.unwrap();
    assert_eq!(DATA.len(), client.try_write(DATA).unwrap());

    // The task should be notified
    while !readable.is_woken() {
        tokio::task::yield_now().await;
    }

    // Fill the write buffer
    loop {
        // Still ready
        let mut writable = task::spawn(client.writable());
        assert_ready_ok!(writable.poll());

        match client.try_write(DATA) {
            Ok(n) => written.extend(&DATA[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                break;
            }
            Err(e) => panic!("error = {e:?}"),
        }
    }

    {
        // Write buffer full
        let mut writable = task::spawn(client.writable());
        assert_pending!(writable.poll());

        // Drain the socket from the server end
        let mut read = Vec::with_capacity(written.len());
        let mut i = 0;

        while i < read.capacity() {
            server.readable().await.unwrap();

            match server.try_read_buf(&mut read) {
                Ok(n) => i += n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("error = {e:?}"),
            }
        }

        assert_eq!(read, written);
    }

    // Now, we listen for shutdown
    drop(client);

    loop {
        let ready = server.ready(Interest::READABLE).await.unwrap();

        if ready.is_read_closed() {
            return;
        } else {
            tokio::task::yield_now().await;
        }
    }
}

// read_closed is a best effort event, so test only for no false positives.
#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn read_closed() {
    let (client, mut server) = create_pair().await;

    let mut ready_fut = task::spawn(client.ready(Interest::READABLE));
    assert_pending!(ready_fut.poll());

    assert_ok!(server.write_all(b"ping").await);

    let ready_event = assert_ok!(ready_fut.await);

    assert!(!ready_event.is_read_closed());
}

// write_closed is a best effort event, so test only for no false positives.
#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn write_closed() {
    let (mut client, mut server) = create_pair().await;

    // Fill the write buffer.
    let write_size = write_until_pending(&mut client);
    let mut ready_fut = task::spawn(client.ready(Interest::WRITABLE));
    assert_pending!(ready_fut.poll());

    // Drain the socket to make client writable.
    let mut read_size = 0;
    while read_size < write_size {
        server.readable().await.unwrap();
        read_size += read_until_pending(&mut server);
    }

    let ready_event = assert_ok!(ready_fut.await);

    assert!(!ready_event.is_write_closed());
}
