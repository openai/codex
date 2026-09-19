//! Uniform child-process API for local subprocesses, with platform-specific spawning.
//!
//! Commands must use the launcher's cleared environment, process group, and default
//! argv[0]. Both implementations expose Tokio stdio handles and kill on drop.

use std::io;
use std::process::Stdio;

use tokio::process::Command;

#[cfg(target_os = "macos")]
#[path = "macos_child.rs"]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::Child;
#[cfg(not(target_os = "macos"))]
pub use tokio::process::Child;

pub fn spawn(mut command: Command) -> io::Result<Child> {
    command
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "macos")]
    {
        Child::spawn(command)
    }
    #[cfg(not(target_os = "macos"))]
    {
        command.spawn()
    }
}
