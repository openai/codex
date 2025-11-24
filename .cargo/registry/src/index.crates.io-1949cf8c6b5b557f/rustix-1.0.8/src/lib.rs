//! `rustix` provides efficient memory-safe and [I/O-safe] wrappers to
//! POSIX-like, Unix-like, Linux, and Winsock syscall-like APIs, with
//! configurable backends.
//!
//! With rustix, you can write code like this:
//!
//! ```
//! # #[cfg(feature = "net")]
//! # fn read(sock: std::net::TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
//! # use rustix::net::RecvFlags;
//! let (nread, _received) = rustix::net::recv(&sock, buf, RecvFlags::PEEK)?;
//! # let _ = nread;
//! # Ok(())
//! # }
//! ```
//!
//! instead of like this:
//!
//! ```
//! # #[cfg(feature = "net")]
//! # fn read(sock: std::net::TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
//! # #[cfg(unix)]
//! # use std::os::unix::io::AsRawFd;
//! # #[cfg(target_os = "wasi")]
//! # use std::os::wasi::io::AsRawFd;
//! # #[cfg(windows)]
//! # use windows_sys::Win32::Networking::WinSock as libc;
//! # #[cfg(windows)]
//! # use std::os::windows::io::AsRawSocket;
//! # const MSG_PEEK: i32 = libc::MSG_PEEK;
//! let nread = unsafe {
//!     #[cfg(any(unix, target_os = "wasi"))]
//!     let raw = sock.as_raw_fd();
//!     #[cfg(windows)]
//!     let raw = sock.as_raw_socket();
//!     match libc::recv(
//!         raw as _,
//!         buf.as_mut_ptr().cast(),
//!         buf.len().try_into().unwrap_or(i32::MAX as _),
//!         MSG_PEEK,
//!     ) {
//!         -1 => return Err(std::io::Error::last_os_error()),
//!         nread => nread as usize,
//!     }
//! };
//! # let _ = nread;
//! # Ok(())
//! # }
//! ```
//!
//! rustix's APIs perform the following tasks:
//!  - Error values are translated to [`Result`]s.
//!  - Buffers are passed as Rust slices.
//!  - Out-parameters are presented as return values.
//!  - Path arguments use [`Arg`], so they accept any string type.
//!  - File descriptors are passed and returned via [`AsFd`] and [`OwnedFd`]
//!    instead of bare integers, ensuring I/O safety.
//!  - Constants use `enum`s and [`bitflags`] types, and enable [support for
//!    externally defined flags].
//!  - Multiplexed functions (eg. `fcntl`, `ioctl`, etc.) are de-multiplexed.
//!  - Variadic functions (eg. `openat`, etc.) are presented as non-variadic.
//!  - Functions that return strings automatically allocate sufficient memory
//!    and retry the syscall as needed to determine the needed length.
//!  - Functions and types which need `l` prefixes or `64` suffixes to enable
//!    large-file support (LFS) are used automatically. File sizes and offsets
//!    are always presented as `u64` and `i64`.
//!  - Behaviors that depend on the sizes of C types like `long` are hidden.
//!  - In some places, more human-friendly and less historical-accident names
//!    are used (and documentation aliases are used so that the original names
//!    can still be searched for).
//!  - Provide y2038 compatibility, on platforms which support this.
//!  - Correct selected platform bugs, such as behavioral differences when
//!    running under seccomp.
//!  - Use `timespec` for timestamps and timeouts instead of `timeval` and
//!    `c_int` milliseconds.
//!
//! Things they don't do include:
//!  - Detecting whether functions are supported at runtime, except in specific
//!    cases where new interfaces need to be detected to support y2038 and LFS.
//!  - Hiding significant differences between platforms.
//!  - Restricting ambient authorities.
//!  - Imposing sandboxing features such as filesystem path or network address
//!    sandboxing.
//!
//! See [`cap-std`], [`system-interface`], and [`io-streams`] for libraries
//! which do hide significant differences between platforms, and [`cap-std`]
//! which does perform sandboxing and restricts ambient authorities.
//!
//! [`cap-std`]: https://crates.io/crates/cap-std
//! [`system-interface`]: https://crates.io/crates/system-interface
//! [`io-streams`]: https://crates.io/crates/io-streams
//! [`bitflags`]: bitflags
//! [`AsFd`]: crate::fd::AsFd
//! [`OwnedFd`]: crate::fd::OwnedFd
//! [I/O-safe]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
//! [`Arg`]: path::Arg
//! [support for externally defined flags]: bitflags#externally-defined-flags

#![deny(missing_docs)]
#![allow(stable_features)]
#![cfg_attr(linux_raw, deny(unsafe_code))]
#![cfg_attr(rustc_attrs, feature(rustc_attrs))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(all(wasi_ext, target_os = "wasi", feature = "std"), feature(wasi_ext))]
#![cfg_attr(core_ffi_c, feature(core_ffi_c))]
#![cfg_attr(core_c_str, feature(core_c_str))]
#![cfg_attr(error_in_core, feature(error_in_core))]
#![cfg_attr(all(feature = "alloc", alloc_c_string), feature(alloc_c_string))]
#![cfg_attr(all(feature = "alloc", alloc_ffi), feature(alloc_ffi))]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "rustc-dep-of-std", feature(ip))]
#![cfg_attr(feature = "rustc-dep-of-std", allow(internal_features))]
#![cfg_attr(
    any(feature = "rustc-dep-of-std", core_intrinsics),
    feature(core_intrinsics)
)]
#![cfg_attr(asm_experimental_arch, feature(asm_experimental_arch))]
#![cfg_attr(not(feature = "all-apis"), allow(dead_code))]
// It is common in Linux and libc APIs for types to vary between platforms.
#![allow(clippy::unnecessary_cast)]
// It is common in Linux and libc APIs for types to vary between platforms.
#![allow(clippy::useless_conversion)]
// This clippy lint gets too many false positives.
#![allow(clippy::needless_lifetimes)]
// Until `unnecessary_transmutes` is recognized by our MSRV, don't warn about
// it being unrecognized.
#![allow(unknown_lints)]
// Until `cast_signed` and `cast_unsigned` are supported by our MSRV, don't
// warn about transmutes that could be changed to them.
#![allow(unnecessary_transmutes)]
// Redox and WASI have enough differences that it isn't worth precisely
// conditionalizing all the `use`s for them. Similar for if we don't have
// "all-apis".
#![cfg_attr(
    any(target_os = "redox", target_os = "wasi", not(feature = "all-apis")),
    allow(unused_imports)
)]
// wasip2 conditionally gates stdlib APIs such as `OsStrExt`.
// <https://github.com/rust-lang/rust/issues/130323>
#![cfg_attr(
    all(
        target_os = "wasi",
        target_env = "p2",
        any(feature = "fs", feature = "mount", feature = "net"),
        wasip2,
    ),
    feature(wasip2)
)]

#[cfg(all(feature = "alloc", feature = "rustc-dep-of-std"))]
extern crate rustc_std_workspace_alloc as alloc;

#[cfg(all(feature = "alloc", not(feature = "rustc-dep-of-std")))]
extern crate alloc;

// Use `static_assertions` macros if we have them, or a polyfill otherwise.
#[cfg(all(test, static_assertions))]
#[macro_use]
#[allow(unused_imports)]
extern crate static_assertions;
#[cfg(all(test, not(static_assertions)))]
#[macro_use]
#[allow(unused_imports)]
mod static_assertions;

pub mod buffer;
#[cfg(not(windows))]
#[macro_use]
pub(crate) mod cstr;
#[macro_use]
pub(crate) mod utils;
// Polyfill for `std` in `no_std` builds.
#[cfg_attr(feature = "std", path = "maybe_polyfill/std/mod.rs")]
#[cfg_attr(not(feature = "std"), path = "maybe_polyfill/no_std/mod.rs")]
pub(crate) mod maybe_polyfill;
#[cfg(test)]
#[macro_use]
pub(crate) mod check_types;
#[macro_use]
pub(crate) mod bitcast;

// linux_raw: Weak symbols are used by the use-libc-auxv feature for
// glibc 2.15 support.
//
// libc: Weak symbols are used to call various functions available in some
// versions of libc and not others.
#[cfg(any(
    all(linux_raw, feature = "use-libc-auxv"),
    all(libc, not(any(windows, target_os = "espidf", target_os = "wasi")))
))]
#[macro_use]
mod weak;

// Pick the backend implementation to use.
#[cfg_attr(libc, path = "backend/libc/mod.rs")]
#[cfg_attr(linux_raw, path = "backend/linux_raw/mod.rs")]
#[cfg_attr(wasi, path = "backend/wasi/mod.rs")]
mod backend;

/// Export the `*Fd` types and traits that are used in rustix's public API.
///
/// This module exports the types and traits from [`std::os::fd`], or polyills
/// on Rust < 1.66 or on Windows.
///
/// On Windows, the polyfill consists of aliases of the socket types and
/// traits, For example, [`OwnedSocket`] is aliased to `OwnedFd`, and so on,
/// and there are blanket impls for `AsFd` etc. that map to `AsSocket` impls.
/// These blanket impls suffice for using the traits, however not for
/// implementing them, so this module also exports `AsSocket` and the other
/// traits as-is so that users can implement them if needed.
///
/// [`OwnedSocket`]: https://doc.rust-lang.org/stable/std/os/windows/io/struct.OwnedSocket.html
pub mod fd {
    pub use super::backend::fd::*;
}

// The public API modules.
#[cfg(feature = "event")]
#[cfg_attr(docsrs, doc(cfg(feature = "event")))]
pub mod event;
pub mod ffi;
#[cfg(not(windows))]
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub mod fs;
pub mod io;
#[cfg(linux_kernel)]
#[cfg(feature = "io_uring")]
#[cfg_attr(docsrs, doc(cfg(feature = "io_uring")))]
pub mod io_uring;
pub mod ioctl;
#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "wasi"
)))]
#[cfg(feature = "mm")]
#[cfg_attr(docsrs, doc(cfg(feature = "mm")))]
pub mod mm;
#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
#[cfg_attr(docsrs, doc(cfg(feature = "mount")))]
pub mod mount;
#[cfg(not(target_os = "wasi"))]
#[cfg(feature = "net")]
#[cfg_attr(docsrs, doc(cfg(feature = "net")))]
pub mod net;
#[cfg(not(any(windows, target_os = "espidf")))]
#[cfg(feature = "param")]
#[cfg_attr(docsrs, doc(cfg(feature = "param")))]
pub mod param;
#[cfg(not(windows))]
#[cfg(any(feature = "fs", feature = "mount", feature = "net"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "fs", feature = "mount", feature = "net")))
)]
pub mod path;
#[cfg(feature = "pipe")]
#[cfg_attr(docsrs, doc(cfg(feature = "pipe")))]
#[cfg(not(any(windows, target_os = "wasi")))]
pub mod pipe;
#[cfg(not(windows))]
#[cfg(feature = "process")]
#[cfg_attr(docsrs, doc(cfg(feature = "process")))]
pub mod process;
#[cfg(not(windows))]
#[cfg(not(target_os = "wasi"))]
#[cfg(feature = "pty")]
#[cfg_attr(docsrs, doc(cfg(feature = "pty")))]
pub mod pty;
#[cfg(not(windows))]
#[cfg(feature = "rand")]
#[cfg_attr(docsrs, doc(cfg(feature = "rand")))]
pub mod rand;
#[cfg(not(any(
    windows,
    target_os = "android",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "wasi"
)))]
#[cfg(feature = "shm")]
#[cfg_attr(docsrs, doc(cfg(feature = "shm")))]
pub mod shm;
#[cfg(not(windows))]
#[cfg(feature = "stdio")]
#[cfg_attr(docsrs, doc(cfg(feature = "stdio")))]
pub mod stdio;
#[cfg(feature = "system")]
#[cfg(not(any(windows, target_os = "wasi")))]
#[cfg_attr(docsrs, doc(cfg(feature = "system")))]
pub mod system;
#[cfg(not(any(windows, target_os = "horizon", target_os = "vita")))]
#[cfg(feature = "termios")]
#[cfg_attr(docsrs, doc(cfg(feature = "termios")))]
pub mod termios;
#[cfg(not(windows))]
#[cfg(feature = "thread")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread")))]
pub mod thread;
#[cfg(not(any(windows, target_os = "espidf")))]
#[cfg(feature = "time")]
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
pub mod time;

// "runtime" is also a public API module, but it's only for libc-like users.
#[cfg(not(windows))]
#[cfg(feature = "runtime")]
#[cfg(linux_raw)]
#[cfg_attr(not(document_experimental_runtime_api), doc(hidden))]
#[cfg_attr(docsrs, doc(cfg(feature = "runtime")))]
pub mod runtime;

// Declare "fs" as a non-public module if "fs" isn't enabled but we need it for
// reading procfs.
#[cfg(not(windows))]
#[cfg(not(feature = "fs"))]
#[cfg(all(
    linux_raw,
    not(feature = "use-libc-auxv"),
    not(feature = "use-explicitly-provided-auxv"),
    any(
        feature = "param",
        feature = "runtime",
        feature = "thread",
        feature = "time",
        target_arch = "x86",
    )
))]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub(crate) mod fs;

// Similarly, declare `path` as a non-public module if needed.
#[cfg(not(windows))]
#[cfg(not(any(feature = "fs", feature = "mount", feature = "net")))]
#[cfg(all(
    linux_raw,
    not(feature = "use-libc-auxv"),
    not(feature = "use-explicitly-provided-auxv"),
    any(
        feature = "param",
        feature = "runtime",
        feature = "thread",
        feature = "time",
        target_arch = "x86",
    )
))]
pub(crate) mod path;

// Private modules used by multiple public modules.
#[cfg(not(any(windows, target_os = "espidf")))]
#[cfg(any(feature = "thread", feature = "time"))]
mod clockid;
#[cfg(linux_kernel)]
#[cfg(any(feature = "io_uring", feature = "runtime"))]
mod kernel_sigset;
#[cfg(not(any(windows, target_os = "wasi")))]
#[cfg(any(
    feature = "process",
    feature = "runtime",
    feature = "termios",
    feature = "thread",
    all(bsd, feature = "event"),
    all(linux_kernel, feature = "net")
))]
mod pid;
#[cfg(any(feature = "process", feature = "thread"))]
#[cfg(linux_kernel)]
mod prctl;
#[cfg(not(any(windows, target_os = "espidf", target_os = "wasi")))]
#[cfg(any(
    feature = "io_uring",
    feature = "process",
    feature = "runtime",
    all(bsd, feature = "event")
))]
mod signal;
#[cfg(any(
    feature = "fs",
    feature = "event",
    feature = "process",
    feature = "runtime",
    feature = "thread",
    feature = "time",
    all(feature = "event", any(bsd, linux_kernel, windows, target_os = "wasi")),
    all(
        linux_raw,
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
mod timespec;
#[cfg(not(any(windows, target_os = "wasi")))]
#[cfg(any(
    feature = "fs",
    feature = "process",
    feature = "thread",
    all(
        linux_raw,
        not(feature = "use-libc-auxv"),
        not(feature = "use-explicitly-provided-auxv"),
        any(
            feature = "param",
            feature = "runtime",
            feature = "time",
            target_arch = "x86",
        )
    ),
    all(linux_kernel, feature = "net")
))]
mod ugid;

#[cfg(doc)]
#[cfg_attr(docsrs, doc(cfg(doc)))]
pub mod not_implemented;
