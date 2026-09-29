//! Command construction for background processes that must not allocate a Windows console.
//! Callers retain control of environment, stdio, and process lifetime. Interactive,
//! detached, and private-desktop launches must select their own console policy.

use std::ffi::OsStr;
use std::process::Command;

/// Construct a background command, suppressing console allocation on Windows.
/// Convert with `tokio::process::Command::from` for asynchronous execution.
/// Subsequent Windows `creation_flags` calls must retain `CREATE_NO_WINDOW`.
pub fn background_command(program: impl AsRef<OsStr>) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new(program);
        command.creation_flags(/*flags*/ 0x0800_0000); // CREATE_NO_WINDOW
        command
    }
    #[cfg(not(windows))]
    Command::new(program)
}

#[cfg(all(test, windows))]
#[path = "background_command_tests.rs"]
mod tests;
