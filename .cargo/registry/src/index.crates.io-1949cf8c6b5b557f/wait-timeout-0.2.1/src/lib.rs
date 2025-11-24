//! A crate to wait on a child process with a particular timeout.
//!
//! This crate is an implementation for Unix and Windows of the ability to wait
//! on a child process with a timeout specified. On Windows the implementation
//! is fairly trivial as it's just a call to `WaitForSingleObject` with a
//! timeout argument, but on Unix the implementation is much more involved. The
//! current implementation registers a `SIGCHLD` handler and initializes some
//! global state. This handler also works within multi-threaded environments.
//! If your application is otherwise handling `SIGCHLD` then bugs may arise.
//!
//! # Example
//!
//! ```no_run
//! use std::process::Command;
//! use wait_timeout::ChildExt;
//! use std::time::Duration;
//!
//! let mut child = Command::new("foo").spawn().unwrap();
//!
//! let one_sec = Duration::from_secs(1);
//! let status_code = match child.wait_timeout(one_sec).unwrap() {
//!     Some(status) => status.code(),
//!     None => {
//!         // child hasn't exited yet
//!         child.kill().unwrap();
//!         child.wait().unwrap().code()
//!     }
//! };
//! ```

#![deny(missing_docs, warnings)]
#![doc(html_root_url = "https://docs.rs/wait-timeout/0.1")]

#[cfg(unix)]
extern crate libc;

use std::io;
use std::process::{Child, ExitStatus};
use std::time::Duration;

#[cfg(unix)]
#[path = "unix.rs"]
mod imp;
#[cfg(windows)]
#[path = "windows.rs"]
mod imp;

/// Extension methods for the standard `std::process::Child` type.
pub trait ChildExt {
    /// Deprecated, use `wait_timeout` instead.
    #[doc(hidden)]
    fn wait_timeout_ms(&mut self, ms: u32) -> io::Result<Option<ExitStatus>> {
        self.wait_timeout(Duration::from_millis(ms as u64))
    }

    /// Wait for this child to exit, timing out after the duration `dur` has
    /// elapsed.
    ///
    /// If `Ok(None)` is returned then the timeout period elapsed without the
    /// child exiting, and if `Ok(Some(..))` is returned then the child exited
    /// with the specified exit code.
    fn wait_timeout(&mut self, dur: Duration) -> io::Result<Option<ExitStatus>>;
}

impl ChildExt for Child {
    fn wait_timeout(&mut self, dur: Duration) -> io::Result<Option<ExitStatus>> {
        drop(self.stdin.take());
        imp::wait_timeout(self, dur)
    }
}
