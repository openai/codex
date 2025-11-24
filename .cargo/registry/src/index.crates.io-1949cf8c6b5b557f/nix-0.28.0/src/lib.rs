//! Rust friendly bindings to the various *nix system functions.
//!
//! Modules are structured according to the C header file that they would be
//! defined in.
//!
//! # Features
//!
//! Nix uses the following Cargo features to enable optional functionality.
//! They may be enabled in any combination.
//! * `acct` - Process accounting
//! * `aio` - POSIX AIO
//! * `dir` - Stuff relating to directory iteration
//! * `env` - Manipulate environment variables
//! * `event` - Event-driven APIs, like `kqueue` and `epoll`
//! * `fanotify` - Linux's `fanotify` filesystem events monitoring API
//! * `feature` - Query characteristics of the OS at runtime
//! * `fs` - File system functionality
//! * `hostname` - Get and set the system's hostname
//! * `inotify` - Linux's `inotify` file system notification API
//! * `ioctl` - The `ioctl` syscall, and wrappers for many specific instances
//! * `kmod` - Load and unload kernel modules
//! * `mman` - Stuff relating to memory management
//! * `mount` - Mount and unmount file systems
//! * `mqueue` - POSIX message queues
//! * `net` - Networking-related functionality
//! * `personality` - Set the process execution domain
//! * `poll` - APIs like `poll` and `select`
//! * `process` - Stuff relating to running processes
//! * `pthread` - POSIX threads
//! * `ptrace` - Process tracing and debugging
//! * `quota` - File system quotas
//! * `reboot` - Reboot the system
//! * `resource` - Process resource limits
//! * `sched` - Manipulate process's scheduling
//! * `socket` - Sockets, whether for networking or local use
//! * `signal` - Send and receive signals to processes
//! * `term` - Terminal control APIs
//! * `time` - Query the operating system's clocks
//! * `ucontext` - User thread context
//! * `uio` - Vectored I/O
//! * `user` - Stuff relating to users and groups
//! * `zerocopy` - APIs like `sendfile` and `copy_file_range`
#![crate_name = "nix"]
#![cfg(unix)]
#![allow(non_camel_case_types)]
#![cfg_attr(test, deny(warnings))]
#![recursion_limit = "500"]
#![deny(unused)]
#![allow(unused_macros)]
#![cfg_attr(
    not(all(
        feature = "acct",
        feature = "aio",
        feature = "dir",
        feature = "env",
        feature = "event",
        feature = "fanotify",
        feature = "feature",
        feature = "fs",
        feature = "hostname",
        feature = "inotify",
        feature = "ioctl",
        feature = "kmod",
        feature = "mman",
        feature = "mount",
        feature = "mqueue",
        feature = "net",
        feature = "personality",
        feature = "poll",
        feature = "process",
        feature = "pthread",
        feature = "ptrace",
        feature = "quota",
        feature = "reboot",
        feature = "resource",
        feature = "sched",
        feature = "socket",
        feature = "signal",
        feature = "term",
        feature = "time",
        feature = "ucontext",
        feature = "uio",
        feature = "user",
        feature = "zerocopy",
    )),
    allow(unused_imports)
)]
#![deny(unstable_features)]
#![deny(missing_copy_implementations)]
#![deny(missing_debug_implementations)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(clippy::cast_ptr_alignment)]
#![deny(unsafe_op_in_unsafe_fn)]

// Re-exported external crates
pub use libc;

// Private internal modules
#[macro_use]
mod macros;

// Public crates
#[cfg(not(target_os = "redox"))]
feature! {
    #![feature = "dir"]
    pub mod dir;
}
feature! {
    #![feature = "env"]
    pub mod env;
}
#[allow(missing_docs)]
pub mod errno;
feature! {
    #![feature = "feature"]

    #[deny(missing_docs)]
    pub mod features;
}
pub mod fcntl;
feature! {
    #![feature = "net"]

    #[cfg(any(linux_android,
              bsd,
              solarish))]
    #[deny(missing_docs)]
    pub mod ifaddrs;
    #[cfg(not(target_os = "redox"))]
    #[deny(missing_docs)]
    pub mod net;
}
#[cfg(linux_android)]
feature! {
    #![feature = "kmod"]
    pub mod kmod;
}
feature! {
    #![feature = "mount"]
    pub mod mount;
}
#[cfg(any(freebsdlike, target_os = "linux", target_os = "netbsd"))]
feature! {
    #![feature = "mqueue"]
    pub mod mqueue;
}
feature! {
    #![feature = "poll"]
    pub mod poll;
}
#[cfg(not(any(target_os = "redox", target_os = "fuchsia")))]
feature! {
    #![feature = "term"]
    #[deny(missing_docs)]
    pub mod pty;
}
feature! {
    #![feature = "sched"]
    pub mod sched;
}
pub mod sys;
feature! {
    #![feature = "time"]
    pub mod time;
}
// This can be implemented for other platforms as soon as libc
// provides bindings for them.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64"
    )
))]
feature! {
    #![feature = "ucontext"]
    #[allow(missing_docs)]
    pub mod ucontext;
}
pub mod unistd;

#[cfg(any(feature = "poll", feature = "event"))]
mod poll_timeout;

use std::ffi::{CStr, CString, OsStr};
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::{ptr, result, slice};

use errno::Errno;

/// Nix Result Type
pub type Result<T> = result::Result<T, Errno>;

/// Nix's main error type.
///
/// It's a wrapper around Errno.  As such, it's very interoperable with
/// [`std::io::Error`], but it has the advantages of:
/// * `Clone`
/// * `Copy`
/// * `Eq`
/// * Small size
/// * Represents all of the system's errnos, instead of just the most common
/// ones.
pub type Error = Errno;

/// Common trait used to represent file system paths by many Nix functions.
pub trait NixPath {
    /// Is the path empty?
    fn is_empty(&self) -> bool;

    /// Length of the path in bytes
    fn len(&self) -> usize;

    /// Execute a function with this path as a `CStr`.
    ///
    /// Mostly used internally by Nix.
    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T;
}

impl NixPath for str {
    fn is_empty(&self) -> bool {
        NixPath::is_empty(OsStr::new(self))
    }

    fn len(&self) -> usize {
        NixPath::len(OsStr::new(self))
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T,
    {
        OsStr::new(self).with_nix_path(f)
    }
}

impl NixPath for OsStr {
    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T,
    {
        self.as_bytes().with_nix_path(f)
    }
}

impl NixPath for CStr {
    fn is_empty(&self) -> bool {
        self.to_bytes().is_empty()
    }

    fn len(&self) -> usize {
        self.to_bytes().len()
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T,
    {
        Ok(f(self))
    }
}

impl NixPath for [u8] {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T,
    {
        // The real PATH_MAX is typically 4096, but it's statistically unlikely to have a path
        // longer than ~300 bytes. See the the PR description to get stats for your own machine.
        // https://github.com/nix-rust/nix/pull/1656
        //
        // By being smaller than a memory page, we also avoid the compiler inserting a probe frame:
        // https://docs.rs/compiler_builtins/latest/compiler_builtins/probestack/index.html
        const MAX_STACK_ALLOCATION: usize = 1024;

        if self.len() >= MAX_STACK_ALLOCATION {
            return with_nix_path_allocating(self, f);
        }

        let mut buf = MaybeUninit::<[u8; MAX_STACK_ALLOCATION]>::uninit();
        let buf_ptr = buf.as_mut_ptr().cast();

        unsafe {
            ptr::copy_nonoverlapping(self.as_ptr(), buf_ptr, self.len());
            buf_ptr.add(self.len()).write(0);
        }

        match CStr::from_bytes_with_nul(unsafe {
            slice::from_raw_parts(buf_ptr, self.len() + 1)
        }) {
            Ok(s) => Ok(f(s)),
            Err(_) => Err(Errno::EINVAL),
        }
    }
}

#[cold]
#[inline(never)]
fn with_nix_path_allocating<T, F>(from: &[u8], f: F) -> Result<T>
where
    F: FnOnce(&CStr) -> T,
{
    match CString::new(from) {
        Ok(s) => Ok(f(&s)),
        Err(_) => Err(Errno::EINVAL),
    }
}

impl NixPath for Path {
    fn is_empty(&self) -> bool {
        NixPath::is_empty(self.as_os_str())
    }

    fn len(&self) -> usize {
        NixPath::len(self.as_os_str())
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T,
    {
        self.as_os_str().with_nix_path(f)
    }
}

impl NixPath for PathBuf {
    fn is_empty(&self) -> bool {
        NixPath::is_empty(self.as_os_str())
    }

    fn len(&self) -> usize {
        NixPath::len(self.as_os_str())
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&CStr) -> T,
    {
        self.as_os_str().with_nix_path(f)
    }
}
