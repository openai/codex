use std::io;
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
#[cfg(unix)]
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use async_io::{Async, Timer};
use futures_lite::{future, prelude::*};
#[cfg(unix)]
use tempfile::tempdir;

const LOREM_IPSUM: &[u8] = b"
Lorem ipsum dolor sit amet, consectetur adipiscing elit.
Donec pretium ante erat, vitae sodales mi varius quis.
Etiam vestibulum lorem vel urna tempor, eu fermentum odio aliquam.
Aliquam consequat urna vitae ipsum pulvinar, in blandit purus eleifend.
";

fn spawn<T: Send + 'static>(
    f: impl Future<Output = T> + Send + 'static,
) -> impl Future<Output = T> + Send + 'static {
    let (s, r) = async_channel::bounded(1);

    thread::spawn(move || {
        future::block_on(async {
            s.send(f.await).await.ok();
        })
    });

    Box::pin(async move { r.recv().await.unwrap() })
}

#[test]
fn tcp_connect() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let addr = listener.get_ref().local_addr()?;
        let task = spawn(async move { listener.accept().await });

        let stream2 = Async::<TcpStream>::connect(addr).await?;
        let stream1 = task.await?.0;

        assert_eq!(
            stream1.get_ref().peer_addr()?,
            stream2.get_ref().local_addr()?,
        );
        assert_eq!(
            stream2.get_ref().peer_addr()?,
            stream1.get_ref().local_addr()?,
        );

        // Now that the listener is closed, connect should fail.
        let err = Async::<TcpStream>::connect(addr).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::ConnectionRefused);

        Ok(())
    })
}

#[test]
fn tcp_peek_read() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let addr = listener.get_ref().local_addr()?;

        let mut stream = Async::<TcpStream>::connect(addr).await?;
        stream.write_all(LOREM_IPSUM).await?;

        let mut buf = [0; 1024];
        let mut incoming = Box::pin(listener.incoming());
        let mut stream = incoming.next().await.unwrap()?;

        let n = stream.peek(&mut buf).await?;
        assert_eq!(&buf[..n], LOREM_IPSUM);
        let n = stream.read(&mut buf).await?;
        assert_eq!(&buf[..n], LOREM_IPSUM);

        Ok(())
    })
}

#[test]
fn tcp_reader_hangup() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let addr = listener.get_ref().local_addr()?;
        let task = spawn(async move { listener.accept().await });

        let mut stream2 = Async::<TcpStream>::connect(addr).await?;
        let stream1 = task.await?.0;

        let task = spawn(async move {
            Timer::after(Duration::from_secs(1)).await;
            drop(stream1);
        });

        while stream2.write_all(LOREM_IPSUM).await.is_ok() {}
        task.await;

        Ok(())
    })
}

#[test]
fn tcp_writer_hangup() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let addr = listener.get_ref().local_addr()?;
        let task = spawn(async move { listener.accept().await });

        let mut stream2 = Async::<TcpStream>::connect(addr).await?;
        let stream1 = task.await?.0;

        let task = spawn(async move {
            Timer::after(Duration::from_secs(1)).await;
            drop(stream1);
        });

        let mut v = vec![];
        stream2.read_to_end(&mut v).await?;
        assert!(v.is_empty());

        task.await;
        Ok(())
    })
}

#[test]
fn udp_send_recv() -> io::Result<()> {
    future::block_on(async {
        let socket1 = Async::<UdpSocket>::bind(([127, 0, 0, 1], 0))?;
        let socket2 = Async::<UdpSocket>::bind(([127, 0, 0, 1], 0))?;
        socket1.get_ref().connect(socket2.get_ref().local_addr()?)?;

        let mut buf = [0u8; 1024];

        socket1.send(LOREM_IPSUM).await?;
        let n = socket2.peek(&mut buf).await?;
        assert_eq!(&buf[..n], LOREM_IPSUM);
        let n = socket2.recv(&mut buf).await?;
        assert_eq!(&buf[..n], LOREM_IPSUM);

        socket2
            .send_to(LOREM_IPSUM, socket1.get_ref().local_addr()?)
            .await?;
        let n = socket1.peek_from(&mut buf).await?.0;
        assert_eq!(&buf[..n], LOREM_IPSUM);
        let n = socket1.recv_from(&mut buf).await?.0;
        assert_eq!(&buf[..n], LOREM_IPSUM);

        Ok(())
    })
}

#[cfg(unix)]
#[test]
fn udp_connect() -> io::Result<()> {
    future::block_on(async {
        let dir = tempdir()?;
        let path = dir.path().join("socket");

        let listener = Async::<UnixListener>::bind(&path)?;

        let mut stream = Async::<UnixStream>::connect(&path).await?;
        stream.write_all(LOREM_IPSUM).await?;

        let mut buf = [0; 1024];
        let mut incoming = Box::pin(listener.incoming());
        let mut stream = incoming.next().await.unwrap()?;

        let n = stream.read(&mut buf).await?;
        assert_eq!(&buf[..n], LOREM_IPSUM);

        Ok(())
    })
}

// This test is broken for now on OpenBSD: https://github.com/rust-lang/rust/issues/116523
#[cfg(all(unix, not(target_os = "openbsd")))]
#[test]
fn uds_connect() -> io::Result<()> {
    future::block_on(async {
        let dir = tempdir()?;
        let path = dir.path().join("socket");
        let listener = Async::<UnixListener>::bind(&path)?;

        let addr = listener.get_ref().local_addr()?;
        let task = spawn(async move { listener.accept().await });

        let stream2 = Async::<UnixStream>::connect(addr.as_pathname().unwrap()).await?;
        let stream1 = task.await?.0;

        assert_eq!(
            stream1.get_ref().peer_addr()?.as_pathname(),
            stream2.get_ref().local_addr()?.as_pathname(),
        );
        assert_eq!(
            stream2.get_ref().peer_addr()?.as_pathname(),
            stream1.get_ref().local_addr()?.as_pathname(),
        );

        // Now that the listener is closed, connect should fail.
        let err = Async::<UnixStream>::connect(addr.as_pathname().unwrap())
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::ConnectionRefused);

        Ok(())
    })
}

#[cfg(unix)]
#[test]
fn uds_send_recv() -> io::Result<()> {
    future::block_on(async {
        let (socket1, socket2) = Async::<UnixDatagram>::pair()?;

        socket1.send(LOREM_IPSUM).await?;
        let mut buf = [0; 1024];
        let n = socket2.recv(&mut buf).await?;
        assert_eq!(&buf[..n], LOREM_IPSUM);

        Ok(())
    })
}

#[cfg(unix)]
#[test]
fn uds_send_to_recv_from() -> io::Result<()> {
    future::block_on(async {
        let dir = tempdir()?;
        let path = dir.path().join("socket");
        let socket1 = Async::<UnixDatagram>::bind(&path)?;
        let socket2 = Async::<UnixDatagram>::unbound()?;

        socket2.send_to(LOREM_IPSUM, &path).await?;
        let mut buf = [0; 1024];
        let n = socket1.recv_from(&mut buf).await?.0;
        assert_eq!(&buf[..n], LOREM_IPSUM);

        Ok(())
    })
}

#[cfg(unix)]
#[test]
fn uds_reader_hangup() -> io::Result<()> {
    future::block_on(async {
        let (socket1, mut socket2) = Async::<UnixStream>::pair()?;

        let task = spawn(async move {
            Timer::after(Duration::from_secs(1)).await;
            drop(socket1);
        });

        while socket2.write_all(LOREM_IPSUM).await.is_ok() {}
        task.await;

        Ok(())
    })
}

#[cfg(unix)]
#[test]
fn uds_writer_hangup() -> io::Result<()> {
    future::block_on(async {
        let (socket1, mut socket2) = Async::<UnixStream>::pair()?;

        let task = spawn(async move {
            Timer::after(Duration::from_secs(1)).await;
            drop(socket1);
        });

        let mut v = vec![];
        socket2.read_to_end(&mut v).await?;
        assert!(v.is_empty());

        task.await;
        Ok(())
    })
}

// Test that we correctly re-register interests after we've previously been
// interested in both readable and writable events and then we get only one of
// those (we need to re-register interest on the other).
#[test]
fn tcp_duplex() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let stream1 =
            Arc::new(Async::<TcpStream>::connect(listener.get_ref().local_addr()?).await?);
        let stream2 = Arc::new(listener.accept().await?.0);

        async fn do_read(s: Arc<Async<TcpStream>>) -> io::Result<()> {
            let mut buf = vec![0u8; 4096];
            loop {
                let len = (&*s).read(&mut buf).await?;
                if len == 0 {
                    return Ok(());
                }
            }
        }

        async fn do_write(s: Arc<Async<TcpStream>>) -> io::Result<()> {
            let buf = vec![0u8; 4096];
            for _ in 0..4096 {
                (&*s).write_all(&buf).await?;
            }
            s.get_ref().shutdown(Shutdown::Write)?;
            Ok(())
        }

        // Read from and write to stream1.
        let r1 = spawn(do_read(stream1.clone()));
        let w1 = spawn(do_write(stream1));

        // Sleep a bit, so that reading and writing are both blocked.
        Timer::after(Duration::from_millis(5)).await;

        // Start reading stream2, make stream1 writable.
        let r2 = spawn(do_read(stream2.clone()));

        // Finish writing to stream1.
        w1.await?;
        r2.await?;

        // Start writing to stream2, make stream1 readable.
        let w2 = spawn(do_write(stream2));

        // Will r1 be correctly woken?
        r1.await?;
        w2.await?;

        Ok(())
    })
}

#[test]
fn shutdown() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let addr = listener.get_ref().local_addr()?;
        let ((mut reader, _), writer) =
            future::try_zip(listener.accept(), Async::<TcpStream>::connect(addr)).await?;

        // The writer must be closed in order for `read_to_end()` to finish.
        let mut buf = Vec::new();
        future::try_zip(reader.read_to_end(&mut buf), async {
            writer.get_ref().shutdown(Shutdown::Write)
        })
        .await?;

        Ok(())
    })
}

// prevent source from unregistering by trying to register it twice
#[test]
fn duplicate_socket_insert() -> io::Result<()> {
    future::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
        let addr = listener.as_ref().local_addr()?;

        // attempt to register twice
        assert!(Async::new(&listener).is_err(), "fails upon second insert");

        // Read and Write to confirm socket did not deregister on duplication attempt
        // Write to stream_w
        let mut stream_w = Async::<TcpStream>::connect(addr).await?;
        stream_w.write(LOREM_IPSUM).await?;
        stream_w.get_ref().shutdown(Shutdown::Write)?;

        // Read from stream_r
        let mut stream_r = listener.accept().await?.0;
        let mut buffer = vec![0; LOREM_IPSUM.len()];
        stream_r.read_exact(&mut buffer).await?;

        assert_eq!(buffer, LOREM_IPSUM);

        Ok(())
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn abstract_socket() -> io::Result<()> {
    use std::ffi::OsStr;
    #[cfg(target_os = "android")]
    use std::os::android::net::SocketAddrExt;
    #[cfg(target_os = "linux")]
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};

    future::block_on(async {
        // Bind a listener to a socket.
        let path = OsStr::from_bytes(b"\0smolabstract");
        let addr = SocketAddr::from_abstract_name(b"smolabstract")?;
        let listener = Async::new(UnixListener::bind_addr(&addr)?)?;

        // Future that connects to the listener.
        let connector = async {
            // Connect to the socket.
            let mut stream = Async::<UnixStream>::connect(path).await?;

            // Write some bytes to the stream.
            stream.write_all(LOREM_IPSUM).await?;

            // Read some bytes from the stream.
            let mut buf = vec![0; LOREM_IPSUM.len()];
            stream.read_exact(&mut buf).await?;
            assert_eq!(buf.as_slice(), LOREM_IPSUM);

            io::Result::Ok(())
        };

        // Future that drives the listener.
        let driver = async {
            // Wait for a new connection.
            let (mut stream, _) = listener.accept().await?;

            // Read some bytes from the stream.
            let mut buf = vec![0; LOREM_IPSUM.len()];
            stream.read_exact(&mut buf).await?;
            assert_eq!(buf.as_slice(), LOREM_IPSUM);

            // Write some bytes to the stream.
            stream.write_all(LOREM_IPSUM).await?;

            io::Result::Ok(())
        };

        // Run both in parallel.
        future::try_zip(connector, driver).await?;

        Ok(())
    })
}
