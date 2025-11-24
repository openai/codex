<div align="center">
  <h1><code>rustix</code></h1>

  <p>
    <strong>Safe Rust bindings to POSIX/Unix/Linux/Winsock syscalls</strong>
  </p>

  <strong>A <a href="https://bytecodealliance.org/">Bytecode Alliance</a> project</strong>

  <p>
    <a href="https://github.com/bytecodealliance/rustix/actions?query=workflow%3ACI"><img src="https://github.com/bytecodealliance/rustix/workflows/CI/badge.svg" alt="Github Actions CI Status" /></a>
    <a href="https://bytecodealliance.zulipchat.com/#narrow/stream/206238-general"><img src="https://img.shields.io/badge/zulip-join_chat-brightgreen.svg" alt="zulip chat" /></a>
    <a href="https://crates.io/crates/rustix"><img src="https://img.shields.io/crates/v/rustix.svg" alt="crates.io page" /></a>
    <a href="https://docs.rs/rustix"><img src="https://docs.rs/rustix/badge.svg" alt="docs.rs docs" /></a>
  </p>
</div>

`rustix` provides efficient memory-safe and [I/O-safe] wrappers to POSIX-like,
Unix-like, Linux, and Winsock syscall-like APIs, with configurable backends. It
uses Rust references, slices, and return values instead of raw pointers, and
[I/O safety types] instead of raw file descriptors, providing memory safety,
[I/O safety], and [provenance]. It uses `Result`s for reporting errors,
[`bitflags`] instead of bare integer flags, an [`Arg`] trait with optimizations
to efficiently accept any Rust string type, and several other efficient
conveniences.

`rustix` is low-level and, and while the `net` API supports [Windows Sockets 2]
(Winsock), the rest of the APIs do not support Windows; for higher-level and
more portable APIs built on this functionality, see the [`cap-std`], [`memfd`],
[`timerfd`], and [`io-streams`] crates, for example.

`rustix` currently has two backends available:

 * linux_raw, which uses raw Linux system calls and vDSO calls, and is
   supported on Linux on x86-64, x86, aarch64, riscv64gc, powerpc64le,
   arm (v5 onwards), mipsel, and mips64el, with stable, nightly, and 1.63 Rust.
    - By being implemented entirely in Rust, avoiding `libc`, `errno`, and pthread
      cancellation, and employing some specialized optimizations, most functions
      compile down to very efficient code, which can often be fully inlined into
      user code.
    - Most functions in `linux_raw` preserve memory, I/O safety, and pointer
      provenance all the way down to the syscalls.

 * libc, which uses the [`libc`] crate which provides bindings to native `libc`
   libraries on Unix-family platforms, and [`windows-sys`] for Winsock on
   Windows, and is portable to many OS's.

The linux_raw backend is enabled by default on platforms which support it. To
enable the libc backend instead, either enable the "use-libc" cargo feature, or
set the `RUSTFLAGS` environment variable to `--cfg=rustix_use_libc` when
building.

## Cargo features

The modules [`rustix::io`], [`rustix::fd`], and [`rustix::ffi`] are enabled by
default. The rest of the API is conditional with cargo feature flags:

| Name       | Description                                                    |
| ---------- | -------------------------------------------------------------- |
| `event`    | [`rustix::event`]—Polling and event operations.                |
| `fs`       | [`rustix::fs`]—Filesystem operations.                          |
| `io_uring` | [`rustix::io_uring`]—Linux io_uring.                           |
| `mm`       | [`rustix::mm`]—Memory map operations.                          |
| `mount`    | [`rustix::mount`]—Linux mount API.                             |
| `net`      | [`rustix::net`]—Network-related operations.                    |
| `param`    | [`rustix::param`]—Process parameters.                          |
| `pipe`     | [`rustix::pipe`]—Pipe operations.                              |
| `process`  | [`rustix::process`]—Process-associated operations.             |
| `procfs`   | [`rustix::procfs`]—Utilities for reading `/proc` on Linux.     |
| `pty`      | [`rustix::pty`]—Pseudoterminal operations.                     |
| `rand`     | [`rustix::rand`]—Random-related operations.                    |
| `shm`      | [`rustix::shm`]—POSIX shared memory.                           |
| `stdio`    | [`rustix::stdio`]—Stdio-related operations.                    |
| `system`   | [`rustix::system`]—System-related operations.                  |
| `termios`  | [`rustix::termios`]—Terminal I/O stream operations.            |
| `thread`   | [`rustix::thread`]—Thread-associated operations.               |
| `time`     | [`rustix::time`]—Time-related operations.                      |
|            |                                                                |
| `use-libc` | Enable the libc backend.                                       |

[`rustix::event`]: https://docs.rs/rustix/*/rustix/event/index.html
[`rustix::fs`]: https://docs.rs/rustix/*/rustix/fs/index.html
[`rustix::io_uring`]: https://docs.rs/rustix/*/rustix/io_uring/index.html
[`rustix::mm`]: https://docs.rs/rustix/*/rustix/mm/index.html
[`rustix::mount`]: https://docs.rs/rustix/*/rustix/mount/index.html
[`rustix::net`]: https://docs.rs/rustix/*/rustix/net/index.html
[`rustix::param`]: https://docs.rs/rustix/*/rustix/param/index.html
[`rustix::pipe`]: https://docs.rs/rustix/*/rustix/pipe/index.html
[`rustix::process`]: https://docs.rs/rustix/*/rustix/process/index.html
[`rustix::procfs`]: https://docs.rs/rustix/*/rustix/procfs/index.html
[`rustix::pty`]: https://docs.rs/rustix/*/rustix/pty/index.html
[`rustix::rand`]: https://docs.rs/rustix/*/rustix/rand/index.html
[`rustix::shm`]: https://docs.rs/rustix/*/rustix/shm/index.html
[`rustix::stdio`]: https://docs.rs/rustix/*/rustix/stdio/index.html
[`rustix::system`]: https://docs.rs/rustix/*/rustix/system/index.html
[`rustix::termios`]: https://docs.rs/rustix/*/rustix/termios/index.html
[`rustix::thread`]: https://docs.rs/rustix/*/rustix/thread/index.html
[`rustix::time`]: https://docs.rs/rustix/*/rustix/time/index.html
[`rustix::io`]: https://docs.rs/rustix/*/rustix/io/index.html
[`rustix::fd`]: https://docs.rs/rustix/*/rustix/fd/index.html
[`rustix::ffi`]: https://docs.rs/rustix/*/rustix/ffi/index.html

## 64-bit Large File Support (LFS) and Year 2038 (y2038) support

`rustix` automatically uses 64-bit APIs when available, and avoids exposing
32-bit APIs that would have the year-2038 problem or fail to support large
files. For instance, `rustix::fstatvfs` calls `fstatvfs64`, and returns a
struct that's 64-bit even on 32-bit platforms.

## Similar crates

`rustix` is similar to [`nix`], [`simple_libc`], [`unix`], [`nc`], [`uapi`],
and [`rusl`]. `rustix` is architected for [I/O safety] with most APIs using
[`OwnedFd`] and [`AsFd`] to manipulate file descriptors rather than `File` or
even `c_int`, and supporting multiple backends so that it can use direct
syscalls while still being usable on all platforms `libc` supports. Like `nix`,
`rustix` has an optimized and flexible filename argument mechanism that allows
users to use a variety of string types, including non-UTF-8 string types.

[`relibc`] is a similar project which aims to be a full "libc", including
C-compatible interfaces and higher-level C/POSIX standard-library
functionality; `rustix` just aims to provide safe and idiomatic Rust interfaces
to low-level syscalls. `relibc` also doesn't tend to support features not
supported on Redox, such as `*at` functions like `openat`, which are important
features for `rustix`.

`rustix` has its own code for making direct syscalls, similar to the
[`syscall`], [`sc`], and [`scall`] crates, using the Rust `asm!` macro.
`rustix` can also use Linux's vDSO mechanism to optimize Linux `clock_gettime`
on all architectures, and all Linux system calls on x86. And `rustix`'s
syscalls report errors using an optimized `Errno` type.

`rustix`'s `*at` functions are similar to the [`openat`] crate, but `rustix`
provides them as free functions rather than associated functions of a `Dir`
type. `rustix`'s `CWD` constant exposes the special `AT_FDCWD` value in a safe
way, so users don't need to open `.` to get a current-directory handle.

`rustix`'s `openat2` function is similar to the [`openat2`] crate, but uses I/O
safety types rather than `RawFd`. `rustix` does not provide dynamic feature
detection, so users must handle the [`NOSYS`] error themselves.

`rustix`'s `termios` module is similar to the [`termios`] crate, but uses I/O
safety types rather than `RawFd`, and the flags parameters to functions such as
`tcsetattr` are `enum`s rather than bare integers. And, rustix calls its
`tcgetattr` function `tcgetattr`, rather than `Termios::from_fd`.

## Minimum Supported Rust Version (MSRV)

This crate currently works on the version of [Rust on Debian stable], which is
currently [Rust 1.63]. This policy may change in the future, in minor version
releases, so users using a fixed version of Rust should pin to a specific
version of this crate.

## Minimum Linux Version

On Linux platforms, rustix requires at least Linux 3.2. This is at most the
oldest Linux version supported by:
 - [any current Rust target], or
 - [kernel.org] at the time of rustix's [MSRV] release.
The specifics of this policy may change in the future, but we intend it to
always reflect “very old” Linux versions.

[MSRV]: #minimum-supported-rust-version-msrv
[Rust 1.63]: https://blog.rust-lang.org/2022/08/11/Rust-1.63.0.html
[any current Rust target]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[kernel.org]: https://www.kernel.org/releases.html
[Rust on Debian stable]: https://packages.debian.org/stable/rust/rustc
[Windows Sockets 2]: https://learn.microsoft.com/en-us/windows/win32/winsock/windows-sockets-start-page-2
[`nix`]: https://crates.io/crates/nix
[`unix`]: https://crates.io/crates/unix
[`nc`]: https://crates.io/crates/nc
[`simple_libc`]: https://crates.io/crates/simple_libc
[`uapi`]: https://crates.io/crates/uapi
[`rusl`]: https://lib.rs/crates/rusl
[`relibc`]: https://gitlab.redox-os.org/redox-os/relibc
[`syscall`]: https://crates.io/crates/syscall
[`sc`]: https://crates.io/crates/sc
[`scall`]: https://crates.io/crates/scall
[`openat`]: https://crates.io/crates/openat
[`openat2`]: https://crates.io/crates/openat2
[I/O safety types]: https://doc.rust-lang.org/stable/std/os/fd/index.html#structs
[`termios`]: https://crates.io/crates/termios
[`libc`]: https://crates.io/crates/libc
[`windows-sys`]: https://crates.io/crates/windows-sys
[`cap-std`]: https://crates.io/crates/cap-std
[`memfd`]: https://crates.io/crates/memfd
[`timerfd`]: https://crates.io/crates/timerfd
[`io-streams`]: https://crates.io/crates/io-streams
[`bitflags`]: https://crates.io/crates/bitflags
[`Arg`]: https://docs.rs/rustix/*/rustix/path/trait.Arg.html
[I/O-safe]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
[I/O safety]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
[provenance]: https://github.com/rust-lang/rust/issues/95228
[`OwnedFd`]: https://doc.rust-lang.org/stable/std/os/fd/struct.OwnedFd.html
[`AsFd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.AsFd.html
[`NOSYS`]: https://docs.rs/rustix/*/rustix/io/struct.Errno.html#associatedconstant.NOSYS
