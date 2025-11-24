#![cfg(feature = "full")]
#![cfg(windows)]

use std::io;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, PipeMode, ServerOptions};
use tokio::time;
use windows_sys::Win32::Foundation::{ERROR_NO_DATA, ERROR_PIPE_BUSY};

#[tokio::test]
async fn test_named_pipe_client_drop() -> io::Result<()> {
    const PIPE_NAME: &str = r"\\.\pipe\test-named-pipe-client-drop";

    let mut server = ServerOptions::new().create(PIPE_NAME)?;

    let client = ClientOptions::new().open(PIPE_NAME)?;

    server.connect().await?;
    drop(client);

    // instance will be broken because client is gone
    match server.write_all(b"ping").await {
        Err(e) if e.raw_os_error() == Some(ERROR_NO_DATA as i32) => (),
        x => panic!("{:?}", x),
    }

    Ok(())
}

#[tokio::test]
async fn test_named_pipe_single_client() -> io::Result<()> {
    use tokio::io::{AsyncBufReadExt as _, BufReader};

    const PIPE_NAME: &str = r"\\.\pipe\test-named-pipe-single-client";

    let server = ServerOptions::new().create(PIPE_NAME)?;

    let server = tokio::spawn(async move {
        // Note: we wait for a client to connect.
        server.connect().await?;

        let mut server = BufReader::new(server);

        let mut buf = String::new();
        server.read_line(&mut buf).await?;
        server.write_all(b"pong\n").await?;
        Ok::<_, io::Error>(buf)
    });

    let client = tokio::spawn(async move {
        let client = ClientOptions::new().open(PIPE_NAME)?;

        let mut client = BufReader::new(client);

        let mut buf = String::new();
        client.write_all(b"ping\n").await?;
        client.read_line(&mut buf).await?;
        Ok::<_, io::Error>(buf)
    });

    let (server, client) = tokio::try_join!(server, client)?;

    assert_eq!(server?, "ping\n");
    assert_eq!(client?, "pong\n");

    Ok(())
}

#[tokio::test]
async fn test_named_pipe_multi_client() -> io::Result<()> {
    use tokio::io::{AsyncBufReadExt as _, BufReader};

    const PIPE_NAME: &str = r"\\.\pipe\test-named-pipe-multi-client";
    const N: usize = 10;

    // The first server needs to be constructed early so that clients can
    // be correctly connected. Otherwise calling .wait will cause the client to
    // error.
    let mut server = ServerOptions::new().create(PIPE_NAME)?;

    let server = tokio::spawn(async move {
        for _ in 0..N {
            // Wait for client to connect.
            server.connect().await?;
            let mut inner = BufReader::new(server);

            // Construct the next server to be connected before sending the one
            // we already have of onto a task. This ensures that the server
            // isn't closed (after it's done in the task) before a new one is
            // available. Otherwise the client might error with
            // `io::ErrorKind::NotFound`.
            server = ServerOptions::new().create(PIPE_NAME)?;

            tokio::spawn(async move {
                let mut buf = String::new();
                inner.read_line(&mut buf).await?;
                inner.write_all(b"pong\n").await?;
                inner.flush().await?;
                Ok::<_, io::Error>(())
            });
        }

        Ok::<_, io::Error>(())
    });

    let mut clients = Vec::new();

    for _ in 0..N {
        clients.push(tokio::spawn(async move {
            // This showcases a generic connect loop.
            //
            // We immediately try to create a client, if it's not found or the
            // pipe is busy we use the specialized wait function on the client
            // builder.
            let client = loop {
                match ClientOptions::new().open(PIPE_NAME) {
                    Ok(client) => break client,
                    Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => (),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e),
                }

                // Wait for a named pipe to become available.
                time::sleep(Duration::from_millis(10)).await;
            };

            let mut client = BufReader::new(client);

            let mut buf = String::new();
            client.write_all(b"ping\n").await?;
            client.flush().await?;
            client.read_line(&mut buf).await?;
            Ok::<_, io::Error>(buf)
        }));
    }

    for client in clients {
        let result = client.await?;
        assert_eq!(result?, "pong\n");
    }

    server.await??;
    Ok(())
}

#[tokio::test]
async fn test_named_pipe_multi_client_ready() -> io::Result<()> {
    use tokio::io::Interest;

    const PIPE_NAME: &str = r"\\.\pipe\test-named-pipe-multi-client-ready";
    const N: usize = 10;

    // The first server needs to be constructed early so that clients can
    // be correctly connected. Otherwise calling .wait will cause the client to
    // error.
    let mut server = ServerOptions::new().create(PIPE_NAME)?;

    let server = tokio::spawn(async move {
        for _ in 0..N {
            // Wait for client to connect.
            server.connect().await?;

            let inner_server = server;

            // Construct the next server to be connected before sending the one
            // we already have of onto a task. This ensures that the server
            // isn't closed (after it's done in the task) before a new one is
            // available. Otherwise the client might error with
            // `io::ErrorKind::NotFound`.
            server = ServerOptions::new().create(PIPE_NAME)?;

            tokio::spawn(async move {
                let server = inner_server;

                {
                    let mut read_buf = [0u8; 5];
                    let mut read_buf_cursor = 0;

                    loop {
                        server.readable().await?;

                        let buf = &mut read_buf[read_buf_cursor..];

                        match server.try_read(buf) {
                            Ok(n) => {
                                read_buf_cursor += n;

                                if read_buf_cursor == read_buf.len() {
                                    break;
                                }
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                };

                {
                    let write_buf = b"pong\n";
                    let mut write_buf_cursor = 0;

                    loop {
                        server.writable().await?;
                        let buf = &write_buf[write_buf_cursor..];

                        match server.try_write(buf) {
                            Ok(n) => {
                                write_buf_cursor += n;

                                if write_buf_cursor == write_buf.len() {
                                    break;
                                }
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                }

                Ok::<_, io::Error>(())
            });
        }

        Ok::<_, io::Error>(())
    });

    let mut clients = Vec::new();

    for _ in 0..N {
        clients.push(tokio::spawn(async move {
            // This showcases a generic connect loop.
            //
            // We immediately try to create a client, if it's not found or the
            // pipe is busy we use the specialized wait function on the client
            // builder.
            let client = loop {
                match ClientOptions::new().open(PIPE_NAME) {
                    Ok(client) => break client,
                    Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => (),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e),
                }

                // Wait for a named pipe to become available.
                time::sleep(Duration::from_millis(10)).await;
            };

            let mut read_buf = [0u8; 5];
            let mut read_buf_cursor = 0;
            let write_buf = b"ping\n";
            let mut write_buf_cursor = 0;

            loop {
                let mut interest = Interest::READABLE;
                if write_buf_cursor < write_buf.len() {
                    interest |= Interest::WRITABLE;
                }

                let ready = client.ready(interest).await?;

                if ready.is_readable() {
                    let buf = &mut read_buf[read_buf_cursor..];

                    match client.try_read(buf) {
                        Ok(n) => {
                            read_buf_cursor += n;

                            if read_buf_cursor == read_buf.len() {
                                break;
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }

                if ready.is_writable() {
                    let buf = &write_buf[write_buf_cursor..];

                    if buf.is_empty() {
                        continue;
                    }

                    match client.try_write(buf) {
                        Ok(n) => {
                            write_buf_cursor += n;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
            }

            let buf = String::from_utf8_lossy(&read_buf).into_owned();

            Ok::<_, io::Error>(buf)
        }));
    }

    for client in clients {
        let result = client.await?;
        assert_eq!(result?, "pong\n");
    }

    server.await??;
    Ok(())
}

// This tests that message mode works as expected.
#[tokio::test]
async fn test_named_pipe_mode_message() -> io::Result<()> {
    // it's easy to accidentally get a seemingly working test here because byte pipes
    // often return contents at write boundaries. to make sure we're doing the right thing we
    // explicitly test that it doesn't work in byte mode.
    _named_pipe_mode_message(PipeMode::Message).await?;
    _named_pipe_mode_message(PipeMode::Byte).await
}

async fn _named_pipe_mode_message(mode: PipeMode) -> io::Result<()> {
    let pipe_name = format!(
        r"\\.\pipe\test-named-pipe-mode-message-{}",
        matches!(mode, PipeMode::Message)
    );
    let mut buf = [0u8; 32];

    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .pipe_mode(mode)
        .create(&pipe_name)?;

    let mut client = ClientOptions::new().pipe_mode(mode).open(&pipe_name)?;

    server.connect().await?;

    // this needs a few iterations, presumably Windows waits for a few calls before merging buffers
    for _ in 0..10 {
        client.write_all(b"hello").await?;
        server.write_all(b"world").await?;
    }
    for _ in 0..10 {
        let n = server.read(&mut buf).await?;
        if buf[..n] != b"hello"[..] {
            assert!(matches!(mode, PipeMode::Byte));
            return Ok(());
        }
        let n = client.read(&mut buf).await?;
        if buf[..n] != b"world"[..] {
            assert!(matches!(mode, PipeMode::Byte));
            return Ok(());
        }
    }
    // byte mode should have errored before.
    assert!(matches!(mode, PipeMode::Message));
    Ok(())
}

// This tests `NamedPipeServer::connect` with various access settings.
#[tokio::test]
async fn test_named_pipe_access() -> io::Result<()> {
    const PIPE_NAME: &str = r"\\.\pipe\test-named-pipe-access";

    for (inb, outb) in [(true, true), (true, false), (false, true)] {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let s = ServerOptions::new()
                .access_inbound(inb)
                .access_outbound(outb)
                .create(PIPE_NAME)?;
            let mut connect_fut = tokio_test::task::spawn(s.connect());
            assert!(connect_fut.poll().is_pending());
            tx.send(()).unwrap();
            connect_fut.await
        });

        // Wait for the server to call connect.
        rx.await.unwrap();
        let _ = ClientOptions::new().read(outb).write(inb).open(PIPE_NAME)?;

        server.await??;
    }
    Ok(())
}
