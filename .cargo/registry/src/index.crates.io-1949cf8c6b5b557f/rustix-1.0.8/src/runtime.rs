//! Experimental low-level implementation details for libc-like runtime
//! libraries such as [Origin].
//!
//! ⚠ These are not normal functions. ⚠
//!
//!  - Some of the functions in this module cannot be used in a process which
//!    also has a libc present. This can be true even for functions that have
//!    the same name as a libc function that Rust code can use. Such functions
//!    are not marked `unsafe` (unless they are unsafe for other reasons), even
//!    though they invoke Undefined Behavior if called in a process which has a
//!    libc present.
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
//! All that said, there are some functions in this module would could
//! potentially be stabilized and moved to other modules. See also the
//! documentation for specific functions in the [`not_implemented`] module, and
//! the discussion in [#1314].
//!
//! [Origin]: https://github.com/sunfishcode/origin#readme
//! [`not_implemented`]: crate::not_implemented
//! [#1314]: https://github.com/bytecodealliance/rustix/issues/1314
//!
//! # Safety
//!
//! This module is intended to be used for implementing a runtime library such
//! as libc. Use of these features for any other purpose is likely to create
//! serious problems.
#![allow(unsafe_code)]

use crate::ffi::CStr;
#[cfg(feature = "fs")]
use crate::fs::AtFlags;
use crate::pid::Pid;
use crate::{backend, io};
#[cfg(feature = "fs")]
use backend::fd::AsFd;
use core::ffi::c_void;

pub use crate::kernel_sigset::KernelSigSet;
pub use crate::signal::Signal;

/// `kernel_sigaction`
///
/// On some architectures, the `sa_restorer` field is omitted.
///
/// This type does not have the same layout as `libc::sigaction`.
#[allow(missing_docs)]
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct KernelSigaction {
    pub sa_handler_kernel: KernelSighandler,
    pub sa_flags: KernelSigactionFlags,
    #[cfg(not(any(
        target_arch = "csky",
        target_arch = "loongarch64",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv32",
        target_arch = "riscv64"
    )))]
    pub sa_restorer: KernelSigrestore,
    pub sa_mask: KernelSigSet,
}

bitflags::bitflags! {
    /// Flags for use with [`KernelSigaction`].
    ///
    /// This type does not have the same layout as `sa_flags` field in
    /// `libc::sigaction`, however the flags have the same values as their
    /// libc counterparts.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
    pub struct KernelSigactionFlags: crate::ffi::c_ulong {
        /// `SA_NOCLDSTOP`
        const NOCLDSTOP = linux_raw_sys::general::SA_NOCLDSTOP as _;

        /// `SA_NOCLDWAIT` (since Linux 2.6)
        const NOCLDWAIT = linux_raw_sys::general::SA_NOCLDWAIT as _;

        /// `SA_NODEFER`
        const NODEFER = linux_raw_sys::general::SA_NODEFER as _;

        /// `SA_ONSTACK`
        const ONSTACK = linux_raw_sys::general::SA_ONSTACK as _;

        /// `SA_RESETHAND`
        const RESETHAND = linux_raw_sys::general::SA_RESETHAND as _;

        /// `SA_RESTART`
        const RESTART = linux_raw_sys::general::SA_RESTART as _;

        /// `SA_RESTORER`
        #[cfg(not(any(
            target_arch = "csky",
            target_arch = "loongarch64",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "riscv32",
            target_arch = "riscv64"
        )))]
        const RESTORER = linux_raw_sys::general::SA_RESTORER as _;

        /// `SA_SIGINFO` (since Linux 2.2)
        const SIGINFO = linux_raw_sys::general::SA_SIGINFO as _;

        /// `SA_UNSUPPORTED` (since Linux 5.11)
        const UNSUPPORTED = linux_raw_sys::general::SA_UNSUPPORTED as _;

        /// `SA_EXPOSE_TAGBITS` (since Linux 5.11)
        const EXPOSE_TAGBITS = linux_raw_sys::general::SA_EXPOSE_TAGBITS as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `__sigrestore_t`
///
/// This type differs from `libc::sigrestore_t`, but can be transmuted to it.
pub type KernelSigrestore = Option<unsafe extern "C" fn()>;

/// `__kernel_sighandler_t`
///
/// This type differs from `libc::sighandler_t`, but can be transmuted to it.
pub type KernelSighandler = Option<unsafe extern "C" fn(arg1: crate::ffi::c_int)>;

/// Return a special “ignore” signal handler for ignoring signals.
///
/// This isn't the `SIG_IGN` value itself; it's a function that returns the
/// `SIG_IGN` value.
///
/// If you're looking for `kernel_sig_dfl`; use [`KERNEL_SIG_DFL`].
#[doc(alias = "SIG_IGN")]
#[must_use]
pub const fn kernel_sig_ign() -> KernelSighandler {
    linux_raw_sys::signal_macros::sig_ign()
}

/// A special “default” signal handler representing the default behavior
/// for handling a signal.
///
/// If you're looking for `KERNEL_SIG_IGN`; use [`kernel_sig_ign`].
#[doc(alias = "SIG_DFL")]
pub const KERNEL_SIG_DFL: KernelSighandler = linux_raw_sys::signal_macros::SIG_DFL;

/// `stack_t`
///
/// This type is guaranteed to have the same layout as `libc::stack_t`.
///
/// If we want to expose this in public APIs, we should encapsulate the
/// `linux_raw_sys` type.
pub use linux_raw_sys::general::stack_t as Stack;

/// `siginfo_t`
///
/// This type is guaranteed to have the same layout as `libc::siginfo_t`.
///
/// If we want to expose this in public APIs, we should encapsulate the
/// `linux_raw_sys` type.
pub use linux_raw_sys::general::siginfo_t as Siginfo;

pub use crate::timespec::{Nsecs, Secs, Timespec};

/// `SIG_*` constants for use with [`kernel_sigprocmask`].
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
#[doc(alias = "_exit", alias = "_Exit")]
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

/// `fork()`—Creates a new process by duplicating the calling process.
///
/// On success, `Fork::ParentOf` containing the pid of the child process is
/// returned in the parent, and `Fork::Child` containing the pid of the child
/// process is returned in the child.
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
pub unsafe fn kernel_fork() -> io::Result<Fork> {
    backend::runtime::syscalls::kernel_fork()
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
    ParentOf(Pid),
}

/// `execveat(dirfd, path.as_c_str(), argv, envp, flags)`—Execute a new
/// command using the current process.
///
/// Taking raw-pointers-to-raw-pointers is convenient for c-scape, but we
/// should think about potentially a more Rust-idiomatic API if this is ever
/// made public.
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
#[must_use]
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
/// Taking raw-pointers-to-raw-pointers is convenient for c-scape, but we
/// should think about potentially a more Rust-idiomatic API if this is ever
/// made public.
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
#[must_use]
pub unsafe fn execve(path: &CStr, argv: *const *const u8, envp: *const *const u8) -> io::Errno {
    backend::runtime::syscalls::execve(path, argv, envp)
}

/// `sigaction(signal, &new, &old)`—Modify and/or query a signal handler.
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
pub unsafe fn kernel_sigaction(
    signal: Signal,
    new: Option<KernelSigaction>,
) -> io::Result<KernelSigaction> {
    backend::runtime::syscalls::kernel_sigaction(signal, new)
}

/// `sigaltstack(new, old)`—Modify and/or query a signal stack.
///
/// # Safety
///
/// The memory region described by `new` must readable and writable and larger
/// than the platform minimum signal stack size, and must have a guard region
/// that conforms to the platform conventions for stack guard regions. The
/// flags in `new` must be valid. This function does not diagnose all the
/// errors that libc `sigaltstack` functions are documented as diagnosing.
///
/// While the memory region pointed to by `new` is registered as a signal
/// stack, it must remain readable and writable, and must not be mutated in
/// any way other than by having a signal handler run in it, and must not be
/// the referent of a Rust reference from outside the signal handler.
///
/// If code elsewhere in the program is depending on signal handlers being run
/// on a particular stack, this could break that code's assumptions. And if the
/// caller is depending on signal handlers being run on the stack specified in
/// the call, its assumptions could be broken by code elsewhere in the program
/// calling this function.
///
/// There are probably things out there that assume that all alternate signal
/// stack registration goes through libc, and this does not go through libc.
///
/// There may be further safety hazards not yet documented here.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sigaltstack.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sigaltstack.2.html
#[inline]
pub unsafe fn kernel_sigaltstack(new: Option<Stack>) -> io::Result<Stack> {
    backend::runtime::syscalls::kernel_sigaltstack(new)
}

/// `tkill(tid, sig)`—Send a signal to a thread.
///
/// # Safety
///
/// Causing an individual thread to abruptly terminate without involving the
/// process' thread runtime (such as the libpthread or the libc) evokes
/// undefined behavior.
///
/// Also, this is not `tgkill`, so the warning about the hazard of recycled
/// thread IDs applies.
///
/// There may be further safety hazards not yet documented here.
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
/// If this is ever exposed publicly, we should think about whether it should
/// mask out signals reserved by libc.
///
/// # Safety
///
/// If there is a libc in the process, the `set` must not contain any signal
/// reserved by the libc.
///
/// If code elsewhere in the program is depending on delivery of a signal for
/// any reason, for example to prevent it from executing some code, this could
/// cause it to miss that signal, and for example execute that code. And if the
/// caller is depending on delivery of a signal for any reason, its assumptions
/// could be broken by code elsewhere in the program calling this function.
///
/// There may be further safety hazards not yet documented here.
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
pub unsafe fn kernel_sigprocmask(how: How, set: Option<&KernelSigSet>) -> io::Result<KernelSigSet> {
    backend::runtime::syscalls::kernel_sigprocmask(how, set)
}

/// `sigpending()`—Query the pending signals.
///
/// If this is ever exposed publicly, we should think about whether it should
/// mask out signals reserved by libc.
///
/// # References
///  - [Linux `sigpending`]
///
/// [Linux `sigpending`]: https://man7.org/linux/man-pages/man2/sigpending.2.html
#[inline]
pub fn kernel_sigpending() -> KernelSigSet {
    backend::runtime::syscalls::kernel_sigpending()
}

/// `sigsuspend(set)`—Suspend the calling thread and wait for signals.
///
/// If this is ever exposed publicly, we should think about whether it should
/// be made to fail if given signals reserved by libc.
///
/// # References
///  - [Linux `sigsuspend`]
///
/// [Linux `sigsuspend`]: https://man7.org/linux/man-pages/man2/sigsuspend.2.html
#[inline]
pub fn kernel_sigsuspend(set: &KernelSigSet) -> io::Result<()> {
    backend::runtime::syscalls::kernel_sigsuspend(set)
}

/// `sigwait(set)`—Wait for signals.
///
/// If this is ever exposed publicly, we should think about whether it should
/// mask out signals reserved by libc.
///
/// # Safety
///
/// If there is a libc in the process, the `set` must not contain any signal
/// reserved by the libc.
///
/// If code elsewhere in the program is depending on delivery of a signal for
/// any reason, for example to prevent it from executing some code, this could
/// cause it to miss that signal, and for example execute that code. And if the
/// caller is depending on delivery of a signal for any reason, its assumptions
/// could be broken by code elsewhere in the program calling this function.
///
/// There may be further safety hazards not yet documented here.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/sigwait.3.html
#[inline]
pub unsafe fn kernel_sigwait(set: &KernelSigSet) -> io::Result<Signal> {
    backend::runtime::syscalls::kernel_sigwait(set)
}

/// `sigwaitinfo(set)`—Wait for signals, returning a [`Siginfo`].
///
/// If this is ever exposed publicly, we should think about whether it should
/// mask out signals reserved by libc.
///
/// # Safety
///
/// If there is a libc in the process, the `set` must not contain any signal
/// reserved by the libc.
///
/// If code elsewhere in the program is depending on delivery of a signal for
/// any reason, for example to prevent it from executing some code, this could
/// cause it to miss that signal, and for example execute that code. And if the
/// caller is depending on delivery of a signal for any reason, its assumptions
/// could be broken by code elsewhere in the program calling this function.
///
/// There may be further safety hazards not yet documented here.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sigwaitinfo.2.html
#[inline]
pub unsafe fn kernel_sigwaitinfo(set: &KernelSigSet) -> io::Result<Siginfo> {
    backend::runtime::syscalls::kernel_sigwaitinfo(set)
}

/// `sigtimedwait(set)`—Wait for signals, optionally with a timeout.
///
/// If this is ever exposed publicly, we should think about whether it should
/// mask out signals reserved by libc.
///
/// # Safety
///
/// If there is a libc in the process, the `set` must not contain any signal
/// reserved by the libc.
///
/// If code elsewhere in the program is depending on delivery of a signal for
/// any reason, for example to prevent it from executing some code, this could
/// cause it to miss that signal, and for example execute that code. And if the
/// caller is depending on delivery of a signal for any reason, its assumptions
/// could be broken by code elsewhere in the program calling this function.
///
/// There may be further safety hazards not yet documented here.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sigtimedwait.2.html
#[inline]
pub unsafe fn kernel_sigtimedwait(
    set: &KernelSigSet,
    timeout: Option<&Timespec>,
) -> io::Result<Siginfo> {
    backend::runtime::syscalls::kernel_sigtimedwait(set, timeout)
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
/// don't use it unless you know your code won't share a process with a libc
/// (perhaps because you yourself are implementing a libc).
#[inline]
pub unsafe fn kernel_brk(addr: *mut c_void) -> io::Result<*mut c_void> {
    backend::runtime::syscalls::kernel_brk(addr)
}

/// `SIGRTMIN`—The start of the raw OS “real-time” signal range.
///
/// This is the raw `SIGRTMIN` value from the OS, which is not the same as the
/// `SIGRTMIN` macro provided by libc. Don't use this unless you know your code
/// won't share a process with a libc (perhaps because you yourself are
/// implementing a libc).
pub const KERNEL_SIGRTMIN: i32 = linux_raw_sys::general::SIGRTMIN as i32;

/// `SIGRTMAX`—The last of the raw OS “real-time” signal range.
///
/// This is the raw `SIGRTMAX` value from the OS, which is not the same as the
/// `SIGRTMAX` macro provided by libc. Don't use this unless you know your code
/// won't share a process with a libc (perhaps because you yourself are
/// implementing a libc).
pub const KERNEL_SIGRTMAX: i32 = {
    // Use the actual `SIGRTMAX` value on platforms which define it.
    #[cfg(not(any(
        target_arch = "arm",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64",
    )))]
    {
        linux_raw_sys::general::SIGRTMAX as i32
    }

    // On platforms that don't, derive it from `_NSIG`.
    //
    // In the Linux kernel headers, `_NSIG` refers to the number of signals
    // known to the kernel. It's 64 on most architectures.
    //
    // In libc headers, `_NSIG` refers to the exclusive upper bound of the
    // signals known to the kernel. It's 65 on most architectures.
    //
    // This discrepancy arises because a signal value of 0 is used as a
    // sentinel, and the first `sigset_t` bit is signal 1 instead of 0. The
    // Linux kernel headers and libc headers disagree on the interpretation of
    // `_NSIG` as a result.
    //
    // Here, we use the Linux kernel header value.
    #[cfg(any(
        target_arch = "arm",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64",
    ))]
    {
        linux_raw_sys::general::_NSIG as i32
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assumptions() {
        assert!(libc::SIGSYS < KERNEL_SIGRTMIN);
        assert!(KERNEL_SIGRTMIN <= libc::SIGRTMIN());

        // POSIX guarantees at least 8 RT signals.
        assert!(libc::SIGRTMIN() + 8 <= KERNEL_SIGRTMAX);

        // POSIX guarantees at least 8 RT signals, and it's not uncommon for
        // libc implementations to reserve up to 3 for their own purposes.
        assert!(KERNEL_SIGRTMIN + 8 + 3 <= KERNEL_SIGRTMAX);

        assert!(KERNEL_SIGRTMAX <= libc::SIGRTMAX());
        assert!(libc::SIGRTMAX() as u32 <= linux_raw_sys::general::_NSIG);

        assert!(KERNEL_SIGRTMAX as usize - 1 < core::mem::size_of::<KernelSigSet>() * 8);
    }

    #[test]
    fn test_layouts_matching_libc() {
        use linux_raw_sys::general::siginfo__bindgen_ty_1__bindgen_ty_1;

        // c-scape assumes rustix's `Siginfo` matches libc's. We don't use
        // check_types macros because we want to test compatibility with actual
        // libc, not the `crate::backend::c` which might be our own
        // implementation.
        assert_eq_size!(Siginfo, libc::siginfo_t);
        assert_eq_align!(Siginfo, libc::siginfo_t);
        assert_eq!(
            memoffset::span_of!(Siginfo, ..),
            memoffset::span_of!(Siginfo, __bindgen_anon_1)
        );
        assert_eq!(
            memoffset::span_of!(siginfo__bindgen_ty_1__bindgen_ty_1, si_signo),
            memoffset::span_of!(libc::siginfo_t, si_signo)
        );
        assert_eq!(
            memoffset::span_of!(siginfo__bindgen_ty_1__bindgen_ty_1, si_errno),
            memoffset::span_of!(libc::siginfo_t, si_errno)
        );
        assert_eq!(
            memoffset::span_of!(siginfo__bindgen_ty_1__bindgen_ty_1, si_code),
            memoffset::span_of!(libc::siginfo_t, si_code)
        );

        // c-scape assumes rustix's `Stack` matches libc's. Similar to above.
        assert_eq_size!(Stack, libc::stack_t);
        assert_eq_align!(Stack, libc::stack_t);
        assert_eq!(
            memoffset::span_of!(Stack, ss_sp),
            memoffset::span_of!(libc::stack_t, ss_sp)
        );
        assert_eq!(
            memoffset::span_of!(Stack, ss_flags),
            memoffset::span_of!(libc::stack_t, ss_flags)
        );
        assert_eq!(
            memoffset::span_of!(Stack, ss_size),
            memoffset::span_of!(libc::stack_t, ss_size)
        );
    }

    #[test]
    fn test_layouts_matching_kernel() {
        use linux_raw_sys::general as c;

        // Rustix's versions of these must match the kernel's versions.
        // Some architectures have `sa_restorer`.
        #[cfg(not(any(
            target_arch = "csky",
            target_arch = "loongarch64",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "riscv32",
            target_arch = "riscv64"
        )))]
        check_renamed_struct!(
            KernelSigaction,
            kernel_sigaction,
            sa_handler_kernel,
            sa_flags,
            sa_restorer,
            sa_mask
        );
        // Some architectures omit `sa_restorer`.
        #[cfg(any(
            target_arch = "csky",
            target_arch = "loongarch64",
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "riscv32",
            target_arch = "riscv64"
        ))]
        check_renamed_struct!(
            KernelSigaction,
            kernel_sigaction,
            sa_handler_kernel,
            sa_flags,
            sa_mask
        );
        assert_eq_size!(KernelSigactionFlags, crate::ffi::c_ulong);
        assert_eq_align!(KernelSigactionFlags, crate::ffi::c_ulong);
        check_renamed_type!(KernelSigrestore, __sigrestore_t);
        check_renamed_type!(KernelSighandler, __kernel_sighandler_t);

        assert_eq!(
            libc::SA_NOCLDSTOP,
            KernelSigactionFlags::NOCLDSTOP.bits() as _
        );
        assert_eq!(
            libc::SA_NOCLDWAIT,
            KernelSigactionFlags::NOCLDWAIT.bits() as _
        );
        assert_eq!(libc::SA_NODEFER, KernelSigactionFlags::NODEFER.bits() as _);
        assert_eq!(libc::SA_ONSTACK, KernelSigactionFlags::ONSTACK.bits() as _);
        assert_eq!(
            libc::SA_RESETHAND,
            KernelSigactionFlags::RESETHAND.bits() as _
        );
        assert_eq!(libc::SA_RESTART, KernelSigactionFlags::RESTART.bits() as _);
        assert_eq!(libc::SA_SIGINFO, KernelSigactionFlags::SIGINFO.bits() as _);
    }
}
