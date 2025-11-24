#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi does not support panic recovery
#![cfg(panic = "unwind")]

use std::task::{Context, Poll};
use std::{error::Error, pin::Pin};
use tokio::io::{self, split, AsyncRead, AsyncWrite, ReadBuf};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

struct RW;

impl AsyncRead for RW {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        buf.put_slice(b"z");
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for RW {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Poll::Ready(Ok(1))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(unix)]
mod unix {
    use std::os::unix::prelude::{AsRawFd, RawFd};

    pub struct MockFd;

    impl AsRawFd for MockFd {
        fn as_raw_fd(&self) -> RawFd {
            0
        }
    }
}

#[test]
fn read_buf_initialize_unfilled_to_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let mut buffer = Vec::<u8>::new();
        let mut read_buf = ReadBuf::new(&mut buffer);

        read_buf.initialize_unfilled_to(2);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn read_buf_advance_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let mut buffer = Vec::<u8>::new();
        let mut read_buf = ReadBuf::new(&mut buffer);

        read_buf.advance(2);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn read_buf_set_filled_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let mut buffer = Vec::<u8>::new();
        let mut read_buf = ReadBuf::new(&mut buffer);

        read_buf.set_filled(2);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn read_buf_put_slice_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let mut buffer = Vec::<u8>::new();
        let mut read_buf = ReadBuf::new(&mut buffer);

        let new_slice = [0x40_u8, 0x41_u8];

        read_buf.put_slice(&new_slice);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn unsplit_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let (r1, _w1) = split(RW);
        let (_r2, w2) = split(RW);
        r1.unsplit(w2);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
fn async_fd_new_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::io::unix::AsyncFd;
    use tokio::runtime::Builder;

    let panic_location_file = test_panic(|| {
        // Runtime without `enable_io` so it has no IO driver set.
        let rt = Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let fd = unix::MockFd;

            let _ = AsyncFd::new(fd);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
fn async_fd_with_interest_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::io::unix::AsyncFd;
    use tokio::io::Interest;
    use tokio::runtime::Builder;

    let panic_location_file = test_panic(|| {
        // Runtime without `enable_io` so it has no IO driver set.
        let rt = Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let fd = unix::MockFd;

            let _ = AsyncFd::with_interest(fd, Interest::READABLE);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
fn async_fd_try_new_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::io::unix::AsyncFd;
    use tokio::runtime::Builder;

    let panic_location_file = test_panic(|| {
        // Runtime without `enable_io` so it has no IO driver set.
        let rt = Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let fd = unix::MockFd;

            let _ = AsyncFd::try_new(fd);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
fn async_fd_try_with_interest_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::io::unix::AsyncFd;
    use tokio::io::Interest;
    use tokio::runtime::Builder;

    let panic_location_file = test_panic(|| {
        // Runtime without `enable_io` so it has no IO driver set.
        let rt = Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let fd = unix::MockFd;

            let _ = AsyncFd::try_with_interest(fd, Interest::READABLE);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}
