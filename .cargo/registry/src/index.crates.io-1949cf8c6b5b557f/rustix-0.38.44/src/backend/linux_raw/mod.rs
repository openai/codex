//! The linux_raw backend.
//!
//! This makes Linux syscalls directly, without going through libc.
//!
//! # Safety
//!
//! These files performs raw system calls, and sometimes passes them
//! uninitialized memory buffers. The signatures in this module are currently
//! manually maintained and must correspond with the signatures of the actual
//! Linux syscalls.
//!
//! Some of this could be auto-generated from the Linux header file
//! <linux/syscalls.h>, but we often need more information than it provides,
//! such as which pointers are array slices, out parameters, or in-out
//! parameters, which integers are owned or borrowed file descriptors, etc.

#[macro_use]
mod arch;
mod conv;
mod reg;
#[cfg(any(feature = "time", feature = "process", target_arch = "x86"))]
mod vdso;
#[cfg(any(feature = "time", feature = "process", target_arch = "x86"))]
mod vdso_wrappers;

#[cfg(feature = "event")]
pub(crate) mod event;
#[cfg(any(
    feature = "fs",
    all(
        not(feature = "use-libc-auxv"),
        not(feature = "use-explicitly-provided-auxv"),
        any(
            feature = "param",
            feature = "process",
            feature = "runtime",
            feature = "time",
            target_arch = "x86",
        )
    )
))]
pub(crate) mod fs;
pub(crate) mod io;
#[cfg(feature = "io_uring")]
pub(crate) mod io_uring;
#[cfg(feature = "mm")]
pub(crate) mod mm;
#[cfg(feature = "mount")]
pub(crate) mod mount;
#[cfg(all(feature = "fs", not(feature = "mount")))]
pub(crate) mod mount; // for deprecated mount functions in "fs"
#[cfg(feature = "net")]
pub(crate) mod net;
#[cfg(any(
    feature = "param",
    feature = "process",
    feature = "runtime",
    feature = "time",
    target_arch = "x86",
))]
pub(crate) mod param;
#[cfg(feature = "pipe")]
pub(crate) mod pipe;
#[cfg(feature = "process")]
pub(crate) mod process;
#[cfg(feature = "pty")]
pub(crate) mod pty;
#[cfg(feature = "rand")]
pub(crate) mod rand;
#[cfg(feature = "runtime")]
pub(crate) mod runtime;
#[cfg(feature = "shm")]
pub(crate) mod shm;
#[cfg(feature = "system")]
pub(crate) mod system;
#[cfg(feature = "termios")]
pub(crate) mod termios;
#[cfg(feature = "thread")]
pub(crate) mod thread;
#[cfg(feature = "time")]
pub(crate) mod time;

pub(crate) mod fd {
    pub use crate::maybe_polyfill::os::fd::{
        AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd,
    };
}

// The linux_raw backend doesn't use actual libc, so we define selected
// libc-like definitions in a module called `c`.
pub(crate) mod c;

// Private modules used by multiple public modules.
#[cfg(any(feature = "procfs", feature = "process", feature = "runtime"))]
pub(crate) mod pid;
#[cfg(any(feature = "process", feature = "thread"))]
pub(crate) mod prctl;
#[cfg(any(
    feature = "fs",
    feature = "process",
    feature = "thread",
    all(
        not(feature = "use-libc-auxv"),
        not(feature = "use-explicitly-provided-auxv"),
        any(
            feature = "param",
            feature = "runtime",
            feature = "time",
            target_arch = "x86",
        )
    )
))]
pub(crate) mod ugid;

/// The maximum number of buffers that can be passed into a vectored I/O system
/// call on the current platform.
const MAX_IOV: usize = linux_raw_sys::general::UIO_MAXIOV as usize;
