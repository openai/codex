//! Uses the `async_io::os::kqueue` module to wait for a process to terminate.
//!
//! Run with:
//!
//! ```
//! cargo run --example kqueue-process
//! ```

#[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
fn main() -> std::io::Result<()> {
    use std::process::Command;

    use async_io::os::kqueue::{Exit, Filter};
    use futures_lite::future;

    future::block_on(async {
        // Spawn a process.
        let process = Command::new("sleep")
            .arg("3")
            .spawn()
            .expect("failed to spawn process");

        // Wrap the process in an `Async` object that waits for it to exit.
        let process = Filter::new(Exit::new(process))?;

        // Wait for the process to exit.
        process.ready().await?;

        Ok(())
    })
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
)))]
fn main() {
    println!("This example only works for kqueue-enabled platforms.");
}
