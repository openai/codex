//! Runs a command using waitable handles on Windows.
//!
//! Run with:
//!
//! ```
//! cargo run --example windows-command
//! ```

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use async_io::os::windows::Waitable;
    use std::process::Command;

    futures_lite::future::block_on(async {
        // Spawn a process.
        let process = Command::new("cmd")
            .args(["/C", "echo hello"])
            .spawn()
            .expect("failed to spawn process");

        // Wrap the process in an `Async` object that waits for it to exit.
        let process = Waitable::new(process)?;

        // Wait for the process to exit.
        process.ready().await?;

        Ok(())
    })
}

#[cfg(not(windows))]
fn main() {
    println!("This example is only supported on Windows.");
}
