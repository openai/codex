//! Uses the `timerfd` crate to sleep using an OS timer.
//!
//! Run with:
//!
//! ```
//! cargo run --example linux-timerfd
//! ```

#[cfg(target_os = "linux")]
fn main() -> std::io::Result<()> {
    use std::io;
    use std::os::unix::io::AsRawFd;
    use std::time::{Duration, Instant};

    use async_io::Async;
    use futures_lite::future;
    use rustix::fd::BorrowedFd;
    use timerfd::{SetTimeFlags, TimerFd, TimerState};

    /// Sleeps using an OS timer.
    async fn sleep(dur: Duration) -> io::Result<()> {
        // Create an OS timer.
        let mut timer = TimerFd::new()?;
        timer.set_state(TimerState::Oneshot(dur), SetTimeFlags::Default);

        // When the OS timer fires, a 64-bit integer can be read from it.
        Async::new(timer)?
            .read_with(|t| {
                // Safety: We assume `as_raw_fd()` returns a valid fd. When we
                // can depend on Rust >= 1.63, where `AsFd` is stabilized, and
                // when `TimerFd` implements it, we can remove this unsafe and
                // simplify this.
                let fd = unsafe { BorrowedFd::borrow_raw(t.as_raw_fd()) };
                rustix::io::read(fd, &mut [0u8; 8]).map_err(io::Error::from)
            })
            .await?;
        Ok(())
    }

    future::block_on(async {
        let start = Instant::now();
        println!("Sleeping...");

        // Sleep for a second using an OS timer.
        sleep(Duration::from_secs(1)).await?;

        println!("Woke up after {:?}", start.elapsed());
        Ok(())
    })
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This example works only on Linux!");
}
