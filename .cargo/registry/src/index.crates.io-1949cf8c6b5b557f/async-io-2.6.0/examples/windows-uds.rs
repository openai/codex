//! Uses the `uds_windows` crate to simulate Unix sockets on Windows.
//!
//! Run with:
//!
//! ```
//! cargo run --example windows-uds
//! ```

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use std::ops::Deref;
    use std::os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket};
    use std::path::PathBuf;

    use async_io::Async;
    use blocking::Unblock;
    use futures_lite::{future, prelude::*};
    use std::io;
    use tempfile::tempdir;

    // n.b.: notgull: uds_windows does not support I/O safety uet, hence the wrapper types

    struct UnixListener(uds_windows::UnixListener);

    impl From<uds_windows::UnixListener> for UnixListener {
        fn from(ul: uds_windows::UnixListener) -> Self {
            Self(ul)
        }
    }

    impl Deref for UnixListener {
        type Target = uds_windows::UnixListener;

        fn deref(&self) -> &uds_windows::UnixListener {
            &self.0
        }
    }

    impl AsSocket for UnixListener {
        fn as_socket(&self) -> BorrowedSocket<'_> {
            unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
        }
    }

    struct UnixStream(uds_windows::UnixStream);

    impl From<uds_windows::UnixStream> for UnixStream {
        fn from(ul: uds_windows::UnixStream) -> Self {
            Self(ul)
        }
    }

    impl Deref for UnixStream {
        type Target = uds_windows::UnixStream;

        fn deref(&self) -> &uds_windows::UnixStream {
            &self.0
        }
    }

    impl AsSocket for UnixStream {
        fn as_socket(&self) -> BorrowedSocket<'_> {
            unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
        }
    }

    impl io::Read for UnixStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            io::Read::read(&mut self.0, buf)
        }
    }

    impl io::Write for UnixStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            io::Write::write(&mut self.0, buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            io::Write::flush(&mut self.0)
        }
    }

    unsafe impl async_io::IoSafe for UnixStream {}

    async fn client(addr: PathBuf) -> io::Result<()> {
        // Connect to the address.
        let stream = Async::new(UnixStream::from(uds_windows::UnixStream::connect(addr)?))?;
        println!("Connected to {:?}", stream.get_ref().peer_addr()?);

        // Pipe the stream to stdout.
        let mut stdout = Unblock::new(std::io::stdout());
        futures_lite::io::copy(stream, &mut stdout).await?;
        Ok(())
    }

    let dir = tempdir()?;
    let path = dir.path().join("socket");

    future::block_on(async {
        // Create a listener.
        let listener = Async::new(UnixListener::from(uds_windows::UnixListener::bind(&path)?))?;
        println!("Listening on {:?}", listener.get_ref().local_addr()?);

        future::try_zip(
            async {
                // Accept the client.
                let (stream, _) = listener.read_with(|l| l.accept()).await?;
                println!("Accepted a client");

                // Send a message, drop the stream, and wait for the client.
                Async::new(UnixStream::from(stream))?
                    .write_all(b"Hello!\n")
                    .await?;
                Ok(())
            },
            client(path),
        )
        .await?;

        Ok(())
    })
}

#[cfg(not(windows))]
fn main() {
    println!("This example works only on Windows!");
}
