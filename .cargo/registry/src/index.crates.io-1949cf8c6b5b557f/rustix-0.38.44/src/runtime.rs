//! Experimental low-level implementation details for libc-like runtime
//! libraries such as [Origin].
//!
//! ⚠ These are not normal functions. ⚠
//!
//!  - Some of the functions in this module cannot be used in a process which
//!    also has a libc present. This can be true even for functions that have
//!    the same name as a libc function that Rust code can use.
//!
//!  - Some of the functions in this module don't behave exactly the same way
//!    as functions in libc with similar names. Sometimes information about the
//!    differences is included in the Linux documentation under “C
//!    library/kernel differences” sections. But not always.
//!
//!  - The safety requirements of the functions in this module are not fully
//!    documented.
//!
//!  - The API for these functions is not considered stable, and this module is
//!    `doc(hidden)`.
//!
//! ⚠ Caution is indicated. ⚠
//!
//! These functions are for implementing thread-local storage (TLS), managing
//! threads, loaded libraries, and other process-wide resources. Most of
//! `rustix` doesn't care about what other libraries are linked into the
//! program or what they're doing, but the features in this module generally
//! can only be used by one entity within a process.
//!
//! [Origin]: https://github.com/sunfishcode/origin#readme
//!
//! # Safety
//!
//! This module is intended to be used for implementing a runtime library such
//! as libc. Use of these features for any other purpose is likely to create
//! serious problems.
#![allow(unsafe_code)]

use crate::backend;
#[cfg(linux_raw)]
use crate::ffi::CStr;
#[cfg(linux_raw)]
#[cfg(feature = "fs")]
use crate::fs::AtFlags;
#[cfg(linux_raw)]
use crate::io;
#[cfg(linux_raw)]
use crate::pid::Pid;
#[cfg(linux_raw)]
#[cfg(feature = "fs")]
use backend::fd::AsFd;
#[cfg(linux_raw)]
use core::ffi::c_void;

#[cfg(linux_raw)]
pub use crate::signal::Signal;

/// `sigaction`
#[cfg(linux_raw)]
pub type Sigaction = linux_raw_sys::general::kernel_sigaction;

/// `stack_t`
#[cfg(linux_raw)]
pub type Stack = linux_raw_sys::general::stack_t;

/// `sigset_t`
#[cfg(linux_raw)]
pub type Sigset = linux_raw_sys::general::kernel_sigset_t;

/// `siginfo_t`
#[cfg(linux_raw)]
pub type Siginfo = linux_raw_sys::general::siginfo_t;

pub use crate::timespec::{Nsecs, Secs, Timespec};

/// `SIG_*` constants for use with [`sigprocmask`].
#[cfg(linux_raw)]
#[repr(u32)]
pub enum How {
    /// `SIG_BLOCK`
    BLOCK = linux_raw_sys::general::SIG_BLOCK,

    /// `SIG_UNBLOCK`
    UNBLOCK = linux_raw_sys::general::SIG_UNBLOCK,

    /// `SIG_SETMASK`
    SETMASK = linux_raw_sys::general::SIG_SETMASK,
}

#[cfg(target_arch = "x86")]
#[inline]
pub unsafe fn set_thread_area(u_info: &mut UserDesc) -> io::Result<()> {
    backend::runtime::syscalls::tls::set_thread_area(u_info)
}

#[cfg(target_arch = "arm")]
#[inline]
pub unsafe fn arm_set_tls(data: *mut c_void) -> io::Result<()> {
    backend::runtime::syscalls::tls::arm_set_tls(data)
}

/// `prctl(PR_SET_FS, data)`—Set the x86-64 `fs` register.
///
/// # Safety
///
/// This is a very low-level feature for implementing threading libraries.
/// See the references links above.
#[cfg(target_arch = "x86_64")]
#[inline]
pub unsafe fn set_fs(data: *mut c_void) {
    backend::runtime::syscalls::tls::set_fs(data)
}

/// Set the x86-64 thread ID address.
///
/// # Safety
///
/// This is a very low-level feature for implementing threading libraries.
/// See the references links above.
#[inline]
pub unsafe fn set_tid_address(data: *mut c_void) -> Pid {
    backend::runtime::syscalls::tls::set_tid_address(data)
}

#[cfg(linux_raw)]
#[cfg(target_arch = "x86")]
pub use backend::runtime::tls::UserDesc;

/// `syscall(SYS_exit, status)`—Exit the current thread.
///
/// # Safety
///
/// This is a very low-level feature for implementing threading libraries.
#[inline]
pub unsafe fn exit_thread(status: i32) -> ! {
    backend::runtime::syscalls::tls::exit_thread(status)
}

/// Exit all the threads in the current process' thread group.
///
/// This is equivalent to `_exit` and `_Exit` in libc.
///
/// This does not call any `__cxa_atexit`, `atexit`, or any other destructors.
/// Most programs should use [`std::process::exit`] instead of calling this
/// directly.
///
/// # References
///  - [POSIX `_Exit`]
///  - [Linux `exit_group`]
///  - [Linux `_Exit`]
///
/// [POSIX `_Exit`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/_Exit.html
/// [Linux `exit_group`]: https://man7.org/linux/man-pages/man2/exit_group.2.html
/// [Linux `_Exit`]: https://man7.org/linux/man-pages/man2/_Exit.2.html
#[doc(alias = "_exit")]
#[doc(alias = "_Exit")]
#[inline]
pub fn exit_group(status: i32) -> ! {
    backend::runtime::syscalls::exit_group(status)
}

/// `EXIT_SUCCESS` for use with [`exit_group`].
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stdlib.h.html
/// [Linux]: https://man7.org/linux/man-pages/man3/exit.3.html
pub const EXIT_SUCCESS: i32 = backend::c::EXIT_SUCCESS;

/// `EXIT_FAILURE` for use with [`exit_group`].
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stdlib.h.html
/// [Linux]: https://man7.org/linux/man-pages/man3/exit.3.html
pub const EXIT_FAILURE: i32 = backend::c::EXIT_FAILURE;

/// Return fields from the main executable segment headers ("phdrs") relevant
/// to initializing TLS provided to the program at startup.
///
/// `addr` will always be non-null, even when the TLS data is absent, so that
/// the `addr` and `file_size` parameters are suitable for creating a slice
/// with `slice::from_raw_parts`.
#[inline]
pub fn startup_tls_info() -> StartupTlsInfo {
    backend::runtime::tls::startup_tls_info()
}

/// `(getauxval(AT_PHDR), getauxval(AT_PHENT), getauxval(AT_PHNUM))`—Returns
/// the address, ELF segment header size, and number of ELF segment headers for
/// the main executable.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/getauxval.3.html
#[inline]
pub fn exe_phdrs() -> (*const c_void, usize, usize) {
    backend::param::auxv::exe_phdrs()
}

/// `getauxval(AT_ENTRY)`—Returns the address of the program entrypoint.
///
/// Most code interested in the program entrypoint address should instead use a
/// symbol reference to `_start`. That will be properly PC-relative or
/// relocated if needed, and will come with appropriate pointer type and
/// pointer provenance.
///
/// This function is intended only for use in code that implements those
/// relocations, to compute the ASLR offset. It has type `usize`, so it doesn't
/// carry any provenance, and it shouldn't be used to dereference memory.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/getauxval.3.html
#[inline]
pub fn entry() -> usize {
    backend::param::auxv::entry()
}

/// `getauxval(AT_RANDOM)`—Returns the address of 16 pseudorandom bytes.
///
/// These bytes are for use by libc. For anything else, use the `rand` crate.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/getauxval.3.html
#[inline]
pub fn random() -> *const [u8; 16] {
    backend::param::auxv::random()
}

#[cfg(linux_raw)]
pub use backend::runtime::tls::StartupTlsInfo;

/// `fork()`—Creates a new process by duplicating the calling process.
///
/// On success, the pid of the child process is returned in the parent, and
/// `None` is returned in the child.
///
/// Unlike its POSIX and libc counterparts, this `fork` does not invoke any
/// handlers (such as those registered with `pthread_atfork`).
///
/// The program environment in the child after a `fork` and before an `execve`
/// is very special. All code that executes in this environment must avoid:
///
///  - Acquiring any other locks that are held in other threads on the parent
///    at the time of the `fork`, as the child only contains one thread, and
///    attempting to acquire such locks will deadlock (though this is [not
///    considered unsafe]).
///
///  - Performing any dynamic allocation using the global allocator, since
///    global allocators may use locks to ensure thread safety, and their locks
///    may not be released in the child process, so attempts to allocate may
///    deadlock (as described in the previous point).
///
///  - Accessing any external state which the parent assumes it has exclusive
///    access to, such as a file protected by a file lock, as this could
///    corrupt the external state.
///
///  - Accessing any random-number-generator state inherited from the parent,
///    as the parent may have the same state and generate the same random
///    numbers, which may violate security invariants.
///
///  - Accessing any thread runtime state, since this function does not update
///    the thread id in the thread runtime, so thread runtime functions could
///    cause undefined behavior.
///
///  - Accessing any memory shared with the parent, such as a [`MAP_SHARED`]
///    mapping, even with anonymous or [`memfd_create`] mappings, as this could
///    cause undefined behavior.
///
///  - Calling any C function which isn't known to be [async-signal-safe], as
///    that could cause undefined behavior. The extent to which this also
///    applies to Rust functions is unclear at this time.
///
///  - And more.
///
/// # Safety
///
/// The child must avoid accessing any memory shared with the parent in a
/// way that invokes undefined behavior. It must avoid accessing any threading
/// runtime functions in a way that invokes undefined behavior. And it must
/// avoid invoking any undefined behavior through any function that is not
/// guaranteed to be async-signal-safe. But, what does async-signal-safe even
/// mean in a Rust program? This documentation does not have all the answers.
///
/// So you're on your own. And on top of all the troubles with `fork` in
/// general, this wrapper implementation is highly experimental.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// # Literary interlude
///
/// > Do not jump on ancient uncles.
/// > Do not yell at average mice.
/// > Do not wear a broom to breakfast.
/// > Do not ask a snake’s advice.
/// > Do not bathe in chocolate pudding.
/// > Do not talk to bearded bears.
/// > Do not smoke cigars on sofas.
/// > Do not dance on velvet chairs.
/// > Do not take a whale to visit
/// > Russell’s mother’s cousin’s yacht.
/// > And whatever else you do do
/// > It is better you
/// > Do not.
///
/// — “Rules”, by Karla Kuskin
///
/// [`MAP_SHARED`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mmap.html
/// [not considered unsafe]: https://doc.rust-lang.org/reference/behavior-not-considered-unsafe.html#deadlocks
/// [`memfd_create`]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fork.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fork.2.html
/// [async-signal-safe]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/V2_chap02.html#tag_15_04_03
pub unsafe fn fork() -> io::Result<Fork> {
    backend::runtime::syscalls::fork()
}

/// Regular Unix `fork` doesn't tell the child its own PID because it assumes
/// the child can just do `getpid`. That's true, but it's more fun if it
/// doesn't have to.
pub enum Fork {
    /// This is returned in the child process after a `fork`. It holds the PID
    /// of the child.
    Child(Pid),

    /// This is returned in the parent process after a `fork`. It holds the PID
    /// of the child.
    Parent(Pid),
}

/// `execveat(dirfd, path.as_c_str(), argv, envp, flags)`—Execute a new
/// command using the current process.
///
/// # Safety
///
/// The `argv` and `envp` pointers must point to NUL-terminated arrays, and
/// their contents must be pointers to NUL-terminated byte arrays.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/execveat.2.html
#[inline]
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub unsafe fn execveat<Fd: AsFd>(
    dirfd: Fd,
    path: &CStr,
    argv: *const *const u8,
    envp: *const *const u8,
    flags: AtFlags,
) -> io::Errno {
    backend::runtime::syscalls::execveat(dirfd.as_fd(), path, argv, envp, flags)
}

/// `execve(path.as_c_str(), argv, envp)`—Execute a new command using the
/// current process.
///
/// # Safety
///
/// The `argv` and `envp` pointers must point to NUL-terminated arrays, and
/// their contents must be pointers to NUL-terminated byte arrays.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/execve.2.html
#[inline]
pub unsafe fn execve(path: &CStr, argv: *const *const u8, envp: *const *const u8) -> io::Errno {
    backend::runtime::syscalls::execve(path, argv, envp)
}

/// `sigaction(signal, &new, &old)`—Modify or query a signal handler.
///
/// # Safety
///
/// You're on your own. And on top of all the troubles with signal handlers,
/// this implementation is highly experimental. Even further, it differs from
/// the libc `sigaction` in several non-obvious and unsafe ways.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sigaction.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sigaction.2.html
#[inline]
pub unsafe fn sigaction(signal: Signal, new: Option<Sigaction>) -> io::Result<Sigaction> {
    backend::runtime::syscalls::sigaction(signal, new)
}

/// `sigaltstack(new, old)`—Modify or query a signal stack.
///
/// # Safety
///
/// You're on your own. And on top of all the troubles with signal handlers,
/// this implementation is highly experimental.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sigaltstack.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sigaltstack.2.html
#[inline]
pub unsafe fn sigaltstack(new: Option<Stack>) -> io::Result<Stack> {
    backend::runtime::syscalls::sigaltstack(new)
}

/// `tkill(tid, sig)`—Send a signal to a thread.
///
/// # Safety
///
/// You're on your own. And on top of all the troubles with signal handlers,
/// this implementation is highly experimental. Also, this is not `tgkill`, so
/// the warning about the hazard of recycled thread ID's applies.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/tkill.2.html
#[inline]
pub unsafe fn tkill(tid: Pid, sig: Signal) -> io::Result<()> {
    backend::runtime::syscalls::tkill(tid, sig)
}

/// `rt_sigprocmask(how, set, oldset)`—Adjust the process signal mask.
///
/// # Safety
///
/// You're on your own. And on top of all the troubles with signal handlers,
/// this implementation is highly experimental. Even further, it differs from
/// the libc `sigprocmask` in several non-obvious and unsafe ways.
///
/// # References
///  - [Linux `rt_sigprocmask`]
///  - [Linux `pthread_sigmask`]
///
/// [Linux `rt_sigprocmask`]: https://man7.org/linux/man-pages/man2/rt_sigprocmask.2.html
/// [Linux `pthread_sigmask`]: https://man7.org/linux/man-pages/man3/pthread_sigmask.3.html
#[inline]
#[doc(alias = "pthread_sigmask")]
#[doc(alias = "rt_sigprocmask")]
pub unsafe fn sigprocmask(how: How, set: Option<&Sigset>) -> io::Result<Sigset> {
    backend::runtime::syscalls::sigprocmask(how, set)
}

/// `sigpending()`—Query the pending signals.
///
/// # References
///  - [Linux `sigpending`]
///
/// [Linux `sigpending`]: https://man7.org/linux/man-pages/man2/sigpending.2.html
#[inline]
pub fn sigpending() -> Sigset {
    backend::runtime::syscalls::sigpending()
}

/// `sigsuspend(set)`—Suspend the calling thread and wait for signals.
///
/// # References
///  - [Linux `sigsuspend`]
///
/// [Linux `sigsuspend`]: https://man7.org/linux/man-pages/man2/sigsuspend.2.html
#[inline]
pub fn sigsuspend(set: &Sigset) -> io::Result<()> {
    backend::runtime::syscalls::sigsuspend(set)
}

/// `sigwait(set)`—Wait for signals.
///
/// # Safety
///
/// If code elsewhere in the process is depending on delivery of a signal to
/// prevent it from executing some code, this could cause it to miss that
/// signal and execute that code.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/sigwait.3.html
#[inline]
pub unsafe fn sigwait(set: &Sigset) -> io::Result<Signal> {
    backend::runtime::syscalls::sigwait(set)
}

/// `sigwaitinfo(set)`—Wait for signals, returning a [`Siginfo`].
///
/// # Safety
///
/// If code elsewhere in the process is depending on delivery of a signal to
/// prevent it from executing some code, this could cause it to miss that
/// signal and execute that code.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sigwaitinfo.2.html
#[inline]
pub unsafe fn sigwaitinfo(set: &Sigset) -> io::Result<Siginfo> {
    backend::runtime::syscalls::sigwaitinfo(set)
}

/// `sigtimedwait(set)`—Wait for signals, optionally with a timeout.
///
/// # Safety
///
/// If code elsewhere in the process is depending on delivery of a signal to
/// prevent it from executing some code, this could cause it to miss that
/// signal and execute that code.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sigtimedwait.2.html
#[inline]
pub unsafe fn sigtimedwait(set: &Sigset, timeout: Option<Timespec>) -> io::Result<Siginfo> {
    backend::runtime::syscalls::sigtimedwait(set, timeout)
}

/// `getauxval(AT_SECURE)`—Returns the Linux “secure execution” mode.
///
/// Return a boolean value indicating whether “secure execution” mode was
/// requested, due to the process having elevated privileges. This includes
/// whether the `AT_SECURE` AUX value is set, and whether the initial real UID
/// and GID differ from the initial effective UID and GID.
///
/// The meaning of “secure execution” mode is beyond the scope of this
/// comment.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/getauxval.3.html
#[cfg(any(
    linux_raw,
    any(
        all(target_os = "android", target_pointer_width = "64"),
        target_os = "linux",
    )
))]
#[inline]
pub fn linux_secure() -> bool {
    backend::param::auxv::linux_secure()
}

/// `brk(addr)`—Change the location of the “program break”.
///
/// # Safety
///
/// This is not identical to `brk` in libc. libc `brk` may have bookkeeping
/// that needs to be kept up to date that this doesn't keep up to date, so
/// don't use it unless you are implementing libc.
#[cfg(linux_raw)]
#[inline]
pub unsafe fn brk(addr: *mut c_void) -> io::Result<*mut c_void> {
    backend::runtime::syscalls::brk(addr)
}

/// `__SIGRTMIN`—The start of the realtime signal range.
///
/// This is the raw `SIGRTMIN` value from the OS, which is not the same as the
/// `SIGRTMIN` macro provided by libc. Don't use this unless you are
/// implementing libc.
#[cfg(linux_raw)]
pub const SIGRTMIN: u32 = linux_raw_sys::general::SIGRTMIN;

/// `__SIGRTMAX`—The last of the realtime signal range.
///
/// This is the raw `SIGRTMAX` value from the OS, which is not the same as the
/// `SIGRTMAX` macro provided by libc. Don't use this unless you are
/// implementing libc.
#[cfg(linux_raw)]
pub const SIGRTMAX: u32 = {
    // Use the actual `SIGRTMAX` value on platforms which define it.
    #[cfg(not(any(
        target_arch = "arm",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64",
    )))]
    {
        linux_raw_sys::general::SIGRTMAX
    }

    // On platforms that don't, derive it from `_NSIG`.
    #[cfg(any(
        target_arch = "arm",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64",
    ))]
    {
        linux_raw_sys::general::_NSIG - 1
    }
};
