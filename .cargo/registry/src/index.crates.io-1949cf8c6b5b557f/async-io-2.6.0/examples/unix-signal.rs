//! Uses the `signal-hook` crate to catch the Ctrl-C signal.
//!
//! Run with:
//!
//! ```
//! cargo run --example unix-signal
//! ```

#[cfg(unix)]
fn main() -> std::io::Result<()> {
    use std::os::unix::{io::AsRawFd, net::UnixStream};

    use async_io::Async;
    use futures_lite::{future, prelude::*};

    future::block_on(async {
        // Create a Unix stream that receives a byte on each signal occurrence.
        let (a, mut b) = Async::<UnixStream>::pair()?;
        signal_hook::low_level::pipe::register_raw(signal_hook::consts::SIGINT, a.as_raw_fd())?;
        println!("Waiting for Ctrl-C...");

        // Receive a byte that indicates the Ctrl-C signal occurred.
        b.read_exact(&mut [0]).await?;

        println!("Done!");
        Ok(())
    })
}

#[cfg(not(unix))]
fn main() {
    println!("This example works only on Unix systems!");
}
