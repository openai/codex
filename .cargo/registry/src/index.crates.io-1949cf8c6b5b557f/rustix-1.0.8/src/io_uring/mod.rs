//! Linux [io_uring].
//!
//! This API is very low-level. The main adaptations it makes from the raw
//! Linux io_uring API are the use of appropriately-sized `bitflags`, `enum`,
//! `Result`, `OwnedFd`, `AsFd`, `RawFd`, and `*mut c_void` in place of plain
//! integers.
//!
//! For a higher-level API built on top of this, see the [rustix-uring] crate.
//!
//! # Safety
//!
//! io_uring operates on raw pointers and raw file descriptors. Rustix does not
//! attempt to provide a safe API for these, because the abstraction level is
//! too low for this to be practical. Safety should be introduced in
//! higher-level abstraction layers.
//!
//! # References
//!  - [Linux]
//!  - [io_uring header]
//!
//! [Linux]: https://www.man7.org/linux/man-pages/man7/io_uring.7.html
//! [io_uring]: https://en.wikipedia.org/wiki/Io_uring
//! [io_uring header]: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/io_uring.h?h=v6.13
//! [rustix-uring]: https://crates.io/crates/rustix-uring
#![allow(unsafe_code)]

mod bindgen_types;

use crate::fd::{AsFd, BorrowedFd, OwnedFd, RawFd};
use crate::utils::option_as_ptr;
use crate::{backend, io};
use bindgen_types::*;
use core::cmp::Ordering;
use core::ffi::c_void;
use core::hash::{Hash, Hasher};
use core::mem::size_of;
use core::ptr::null_mut;
use linux_raw_sys::net;

// Export types used in io_uring APIs.
pub use crate::clockid::ClockId;
pub use crate::event::epoll::{
    Event as EpollEvent, EventData as EpollEventData, EventFlags as EpollEventFlags,
};
pub use crate::ffi::c_char;
pub use crate::fs::{
    Advice, AtFlags, Mode, OFlags, RenameFlags, ResolveFlags, Statx, StatxFlags, XattrFlags,
};
pub use crate::io::ReadWriteFlags;
pub use crate::kernel_sigset::KernelSigSet;
pub use crate::net::addr::{SocketAddrLen, SocketAddrOpaque, SocketAddrStorage};
pub use crate::net::{RecvFlags, SendFlags, SocketFlags};
pub use crate::signal::Signal;
pub use crate::thread::futex::{
    Wait as FutexWait, WaitFlags as FutexWaitFlags, WaitPtr as FutexWaitPtr,
    WaitvFlags as FutexWaitvFlags,
};
pub use crate::timespec::{Nsecs, Secs, Timespec};

mod sys {
    pub(super) use linux_raw_sys::io_uring::*;
    #[cfg(test)]
    pub(super) use {
        crate::backend::c::iovec, linux_raw_sys::general::open_how, linux_raw_sys::net::msghdr,
    };
}

/// `msghdr`
#[allow(missing_docs)]
#[repr(C)]
pub struct MsgHdr {
    pub msg_name: *mut c_void,
    pub msg_namelen: SocketAddrLen,
    pub msg_iov: *mut iovec,
    pub msg_iovlen: usize,
    pub msg_control: *mut c_void,
    pub msg_controllen: usize,
    pub msg_flags: RecvFlags,
}

/// `io_uring_setup(entries, params)`—Setup a context for performing
/// asynchronous I/O.
///
/// # Safety
///
/// If [`IoringSetupFlags::ATTACH_WQ`] is set, the `wq_fd` field of
/// `io_uring_params` must be an open file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_setup.2.html
#[inline]
pub unsafe fn io_uring_setup(entries: u32, params: &mut io_uring_params) -> io::Result<OwnedFd> {
    backend::io_uring::syscalls::io_uring_setup(entries, params)
}

/// `io_uring_register(fd, opcode, arg, nr_args)`—Register files or user
/// buffers for asynchronous I/O.
///
/// To pass flags, use [`io_uring_register_with`].
///
/// # Safety
///
/// io_uring operates on raw pointers and raw file descriptors. Users are
/// responsible for ensuring that memory and resources are only accessed in
/// valid ways.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_register.2.html
#[inline]
pub unsafe fn io_uring_register<Fd: AsFd>(
    fd: Fd,
    opcode: IoringRegisterOp,
    arg: *const c_void,
    nr_args: u32,
) -> io::Result<u32> {
    backend::io_uring::syscalls::io_uring_register(fd.as_fd(), opcode, arg, nr_args)
}

/// `io_uring_register_with(fd, opcode, flags, arg, nr_args)`—Register files or
/// user buffers for asynchronous I/O.
///
/// # Safety
///
/// io_uring operates on raw pointers and raw file descriptors. Users are
/// responsible for ensuring that memory and resources are only accessed in
/// valid ways.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_register.2.html
#[inline]
pub unsafe fn io_uring_register_with<Fd: AsFd>(
    fd: Fd,
    opcode: IoringRegisterOp,
    flags: IoringRegisterFlags,
    arg: *const c_void,
    nr_args: u32,
) -> io::Result<u32> {
    backend::io_uring::syscalls::io_uring_register_with(fd.as_fd(), opcode, flags, arg, nr_args)
}

/// `io_uring_enter(fd, to_submit, min_complete, flags, 0, 0)`—Initiate
/// and/or complete asynchronous I/O.
///
/// This version has no `arg` argument. To pass:
///  - a signal mask, use [`io_uring_enter_sigmask`].
///  - an [`io_uring_getevents_arg`], use [`io_uring_enter_arg`] (aka
///    `io_uring_enter2`).
///
/// # Safety
///
/// io_uring operates on raw pointers and raw file descriptors. Users are
/// responsible for ensuring that memory and resources are only accessed in
/// valid ways.
///
/// And, `flags` must not have [`IoringEnterFlags::EXT_ARG`] or
/// [`IoringEnterFlags::EXT_ARG_REG`] set.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_enter.2.html
#[doc(alias = "io_uring_enter2")]
#[inline]
pub unsafe fn io_uring_enter<Fd: AsFd>(
    fd: Fd,
    to_submit: u32,
    min_complete: u32,
    flags: IoringEnterFlags,
) -> io::Result<u32> {
    debug_assert!(!flags.contains(IoringEnterFlags::EXT_ARG));
    debug_assert!(!flags.contains(IoringEnterFlags::EXT_ARG_REG));

    backend::io_uring::syscalls::io_uring_enter(
        fd.as_fd(),
        to_submit,
        min_complete,
        flags,
        null_mut(),
        0,
    )
}

/// `io_uring_enter(fd, to_submit, min_complete, flags, sigmask,
/// sizeof(*sigmask))`— Initiate and/or complete asynchronous I/O, with a
/// signal mask.
///
/// # Safety
///
/// io_uring operates on raw pointers and raw file descriptors. Users are
/// responsible for ensuring that memory and resources are only accessed in
/// valid ways.
///
/// And, `flags` must not have [`IoringEnterFlags::EXT_ARG`] or
/// [`IoringEnterFlags::EXT_ARG_REG`] set.
///
/// And, the `KernelSigSet` referred to by `arg` must not contain any signal
/// numbers reserved by libc.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_enter.2.html
#[doc(alias = "io_uring_enter")]
#[inline]
pub unsafe fn io_uring_enter_sigmask<Fd: AsFd>(
    fd: Fd,
    to_submit: u32,
    min_complete: u32,
    flags: IoringEnterFlags,
    sigmask: Option<&KernelSigSet>,
) -> io::Result<u32> {
    debug_assert!(!flags.contains(IoringEnterFlags::EXT_ARG));
    debug_assert!(!flags.contains(IoringEnterFlags::EXT_ARG_REG));

    backend::io_uring::syscalls::io_uring_enter(
        fd.as_fd(),
        to_submit,
        min_complete,
        flags,
        option_as_ptr(sigmask).cast::<c_void>(),
        size_of::<KernelSigSet>(),
    )
}

/// `io_uring_enter2(fd, to_submit, min_complete, flags, arg, sizeof(*arg))`—
/// Initiate and/or complete asynchronous I/O, with a signal mask and a
/// timeout.
///
/// # Safety
///
/// io_uring operates on raw pointers and raw file descriptors. Users are
/// responsible for ensuring that memory and resources are only accessed in
/// valid ways.
///
/// And, `flags` must have [`IoringEnterFlags::EXT_ARG`] set, and must not have
/// [`IoringEnterFlags::EXT_ARG_REG`] set.
///
/// And, the `KernelSigSet` pointed to by the `io_uring_getenvets_arg` referred
/// to by `arg` must not contain any signal numbers reserved by libc.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_enter.2.html
#[doc(alias = "io_uring_enter")]
#[doc(alias = "io_uring_enter2")]
#[inline]
pub unsafe fn io_uring_enter_arg<Fd: AsFd>(
    fd: Fd,
    to_submit: u32,
    min_complete: u32,
    flags: IoringEnterFlags,
    arg: Option<&io_uring_getevents_arg>,
) -> io::Result<u32> {
    debug_assert!(flags.contains(IoringEnterFlags::EXT_ARG));
    debug_assert!(!flags.contains(IoringEnterFlags::EXT_ARG_REG));

    backend::io_uring::syscalls::io_uring_enter(
        fd.as_fd(),
        to_submit,
        min_complete,
        flags,
        option_as_ptr(arg).cast::<c_void>(),
        size_of::<io_uring_getevents_arg>(),
    )
}

// TODO: Uncomment this when we support `IoringRegisterOp::CQWAIT_REG`.
/*
/// `io_uring_enter2(fd, to_submit, min_complete, flags, offset,
/// sizeof(io_uring_reg_wait))`— Initiate and/or complete asynchronous I/O,
/// using a previously registered `io_uring_reg_wait`.
///
/// `offset` is an offset into an area of wait regions previously registered
/// with [`io_uring_register`] using the [`IoringRegisterOp::CQWAIT_REG`]
/// operation.
///
/// # Safety
///
/// io_uring operates on raw pointers and raw file descriptors. Users are
/// responsible for ensuring that memory and resources are only accessed in
/// valid ways.
///
/// And, `flags` must have [`IoringEnterFlags::EXT_ARG_REG`] set, and must not
/// have [`IoringEnterFlags::EXT_ARG`] set.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.man7.org/linux/man-pages/man2/io_uring_enter.2.html
#[doc(alias = "io_uring_enter")]
#[doc(alias = "io_uring_enter2")]
#[inline]
pub unsafe fn io_uring_enter_reg_wait<Fd: AsFd>(
    fd: Fd,
    to_submit: u32,
    min_complete: u32,
    flags: IoringEnterFlags,
    reg_wait: usize,
) -> io::Result<u32> {
    debug_assert!(!flags.contains(IoringEnterFlags::EXT_ARG));
    debug_assert!(flags.contains(IoringEnterFlags::EXT_ARG_REG));

    backend::io_uring::syscalls::io_uring_enter(
        fd.as_fd(),
        to_submit,
        min_complete,
        flags,
        reg_wait as *mut c_void,
        size_of::<io_uring_reg_wait>(),
    )
}
*/

bitflags::bitflags! {
    /// `IORING_ENTER_*` flags for use with [`io_uring_enter`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringEnterFlags: u32 {
        /// `IORING_ENTER_GETEVENTS`
        const GETEVENTS = sys::IORING_ENTER_GETEVENTS;

        /// `IORING_ENTER_SQ_WAKEUP`
        const SQ_WAKEUP = sys::IORING_ENTER_SQ_WAKEUP;

        /// `IORING_ENTER_SQ_WAIT`
        const SQ_WAIT = sys::IORING_ENTER_SQ_WAIT;

        /// `IORING_ENTER_EXT_ARG` (since Linux 5.11)
        const EXT_ARG = sys::IORING_ENTER_EXT_ARG;

        /// `IORING_ENTER_REGISTERED_RING`
        const REGISTERED_RING = sys::IORING_ENTER_REGISTERED_RING;

        /// `IORING_ENTER_ABS_TIMER` (since Linux 6.12)
        const ABS_TIMER = sys::IORING_ENTER_ABS_TIMER;

        /// `IORING_ENTER_EXT_ARG_REG` (since Linux 6.12)
        const EXT_ARG_REG = sys::IORING_ENTER_EXT_ARG_REG;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `IORING_REGISTER_*` and `IORING_UNREGISTER_*` constants for use with
/// [`io_uring_register`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum IoringRegisterOp {
    /// `IORING_REGISTER_BUFFERS`
    RegisterBuffers = sys::io_uring_register_op::IORING_REGISTER_BUFFERS as _,

    /// `IORING_UNREGISTER_BUFFERS`
    UnregisterBuffers = sys::io_uring_register_op::IORING_UNREGISTER_BUFFERS as _,

    /// `IORING_REGISTER_FILES`
    RegisterFiles = sys::io_uring_register_op::IORING_REGISTER_FILES as _,

    /// `IORING_UNREGISTER_FILES`
    UnregisterFiles = sys::io_uring_register_op::IORING_UNREGISTER_FILES as _,

    /// `IORING_REGISTER_EVENTFD`
    RegisterEventfd = sys::io_uring_register_op::IORING_REGISTER_EVENTFD as _,

    /// `IORING_UNREGISTER_EVENTFD`
    UnregisterEventfd = sys::io_uring_register_op::IORING_UNREGISTER_EVENTFD as _,

    /// `IORING_REGISTER_FILES_UPDATE`
    RegisterFilesUpdate = sys::io_uring_register_op::IORING_REGISTER_FILES_UPDATE as _,

    /// `IORING_REGISTER_EVENTFD_ASYNC`
    RegisterEventfdAsync = sys::io_uring_register_op::IORING_REGISTER_EVENTFD_ASYNC as _,

    /// `IORING_REGISTER_PROBE`
    RegisterProbe = sys::io_uring_register_op::IORING_REGISTER_PROBE as _,

    /// `IORING_REGISTER_PERSONALITY`
    RegisterPersonality = sys::io_uring_register_op::IORING_REGISTER_PERSONALITY as _,

    /// `IORING_UNREGISTER_PERSONALITY`
    UnregisterPersonality = sys::io_uring_register_op::IORING_UNREGISTER_PERSONALITY as _,

    /// `IORING_REGISTER_RESTRICTIONS`
    RegisterRestrictions = sys::io_uring_register_op::IORING_REGISTER_RESTRICTIONS as _,

    /// `IORING_REGISTER_ENABLE_RINGS`
    RegisterEnableRings = sys::io_uring_register_op::IORING_REGISTER_ENABLE_RINGS as _,

    /// `IORING_REGISTER_BUFFERS2`
    RegisterBuffers2 = sys::io_uring_register_op::IORING_REGISTER_BUFFERS2 as _,

    /// `IORING_REGISTER_BUFFERS_UPDATE`
    RegisterBuffersUpdate = sys::io_uring_register_op::IORING_REGISTER_BUFFERS_UPDATE as _,

    /// `IORING_REGISTER_FILES2`
    RegisterFiles2 = sys::io_uring_register_op::IORING_REGISTER_FILES2 as _,

    /// `IORING_REGISTER_FILES_UPDATE2`
    RegisterFilesUpdate2 = sys::io_uring_register_op::IORING_REGISTER_FILES_UPDATE2 as _,

    /// `IORING_REGISTER_IOWQ_AFF`
    RegisterIowqAff = sys::io_uring_register_op::IORING_REGISTER_IOWQ_AFF as _,

    /// `IORING_UNREGISTER_IOWQ_AFF`
    UnregisterIowqAff = sys::io_uring_register_op::IORING_UNREGISTER_IOWQ_AFF as _,

    /// `IORING_REGISTER_IOWQ_MAX_WORKERS`
    RegisterIowqMaxWorkers = sys::io_uring_register_op::IORING_REGISTER_IOWQ_MAX_WORKERS as _,

    /// `IORING_REGISTER_RING_FDS`
    RegisterRingFds = sys::io_uring_register_op::IORING_REGISTER_RING_FDS as _,

    /// `IORING_UNREGISTER_RING_FDS`
    UnregisterRingFds = sys::io_uring_register_op::IORING_UNREGISTER_RING_FDS as _,

    /// `IORING_REGISTER_PBUF_RING`
    RegisterPbufRing = sys::io_uring_register_op::IORING_REGISTER_PBUF_RING as _,

    /// `IORING_UNREGISTER_PBUF_RING`
    UnregisterPbufRing = sys::io_uring_register_op::IORING_UNREGISTER_PBUF_RING as _,

    /// `IORING_REGISTER_SYNC_CANCEL`
    RegisterSyncCancel = sys::io_uring_register_op::IORING_REGISTER_SYNC_CANCEL as _,

    /// `IORING_REGISTER_FILE_ALLOC_RANGE`
    RegisterFileAllocRange = sys::io_uring_register_op::IORING_REGISTER_FILE_ALLOC_RANGE as _,

    /// `IORING_REGISTER_PBUF_STATUS` (since Linux 6.8)
    RegisterPbufStatus = sys::io_uring_register_op::IORING_REGISTER_PBUF_STATUS as _,

    /// `IORING_REGISTER_NAPI` (since Linux 6.9)
    RegisterNapi = sys::io_uring_register_op::IORING_REGISTER_NAPI as _,

    /// `IORING_UNREGISTER_NAPI` (since Linux 6.9)
    UnregisterNapi = sys::io_uring_register_op::IORING_UNREGISTER_NAPI as _,

    /// `IORING_REGISTER_CLOCK` (since Linux 6.12)
    RegisterClock = sys::io_uring_register_op::IORING_REGISTER_CLOCK as _,

    /// `IORING_REGISTER_CLONE_BUFFERS ` (since Linux 6.12)
    RegisterCloneBuffers = sys::io_uring_register_op::IORING_REGISTER_CLONE_BUFFERS as _,

    /// `IORING_REGISTER_SEND_MSG_RING` (since Linux 6.12)
    RegisterSendMsgRing = sys::io_uring_register_op::IORING_REGISTER_SEND_MSG_RING as _,

    /// `IORING_REGISTER_RESIZE_RINGS`(since Linux 6.13)
    RegisterResizeRings = sys::io_uring_register_op::IORING_REGISTER_RESIZE_RINGS as _,
}

bitflags::bitflags! {
    /// `IORING_REGISTER_*` flags for use with [`io_uring_register_with`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringRegisterFlags: u32 {
        /// `IORING_REGISTER_USE_REGISTERED_RING`
        const USE_REGISTERED_RING = sys::io_uring_register_op::IORING_REGISTER_USE_REGISTERED_RING as u32;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `IORING_OP_*` constants for use with [`io_uring_sqe`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum IoringOp {
    /// `IORING_OP_NOP`
    Nop = sys::io_uring_op::IORING_OP_NOP as _,

    /// `IORING_OP_ACCEPT`
    Accept = sys::io_uring_op::IORING_OP_ACCEPT as _,

    /// `IORING_OP_ASYNC_CANCEL`
    AsyncCancel = sys::io_uring_op::IORING_OP_ASYNC_CANCEL as _,

    /// `IORING_OP_CLOSE`
    Close = sys::io_uring_op::IORING_OP_CLOSE as _,

    /// `IORING_OP_CONNECT`
    Connect = sys::io_uring_op::IORING_OP_CONNECT as _,

    /// `IORING_OP_EPOLL_CTL`
    EpollCtl = sys::io_uring_op::IORING_OP_EPOLL_CTL as _,

    /// `IORING_OP_FADVISE`
    Fadvise = sys::io_uring_op::IORING_OP_FADVISE as _,

    /// `IORING_OP_FALLOCATE`
    Fallocate = sys::io_uring_op::IORING_OP_FALLOCATE as _,

    /// `IORING_OP_FILES_UPDATE`
    FilesUpdate = sys::io_uring_op::IORING_OP_FILES_UPDATE as _,

    /// `IORING_OP_FSYNC`
    Fsync = sys::io_uring_op::IORING_OP_FSYNC as _,

    /// `IORING_OP_LINKAT`
    Linkat = sys::io_uring_op::IORING_OP_LINKAT as _,

    /// `IORING_OP_LINK_TIMEOUT`
    LinkTimeout = sys::io_uring_op::IORING_OP_LINK_TIMEOUT as _,

    /// `IORING_OP_MADVISE`
    Madvise = sys::io_uring_op::IORING_OP_MADVISE as _,

    /// `IORING_OP_MKDIRAT`
    Mkdirat = sys::io_uring_op::IORING_OP_MKDIRAT as _,

    /// `IORING_OP_OPENAT`
    Openat = sys::io_uring_op::IORING_OP_OPENAT as _,

    /// `IORING_OP_OPENAT2`
    Openat2 = sys::io_uring_op::IORING_OP_OPENAT2 as _,

    /// `IORING_OP_POLL_ADD`
    PollAdd = sys::io_uring_op::IORING_OP_POLL_ADD as _,

    /// `IORING_OP_POLL_REMOVE`
    PollRemove = sys::io_uring_op::IORING_OP_POLL_REMOVE as _,

    /// `IORING_OP_PROVIDE_BUFFERS`
    ProvideBuffers = sys::io_uring_op::IORING_OP_PROVIDE_BUFFERS as _,

    /// `IORING_OP_READ`
    Read = sys::io_uring_op::IORING_OP_READ as _,

    /// `IORING_OP_READV`
    Readv = sys::io_uring_op::IORING_OP_READV as _,

    /// `IORING_OP_READ_FIXED`
    ReadFixed = sys::io_uring_op::IORING_OP_READ_FIXED as _,

    /// `IORING_OP_RECV`
    Recv = sys::io_uring_op::IORING_OP_RECV as _,

    /// `IORING_OP_RECVMSG`
    Recvmsg = sys::io_uring_op::IORING_OP_RECVMSG as _,

    /// `IORING_OP_REMOVE_BUFFERS`
    RemoveBuffers = sys::io_uring_op::IORING_OP_REMOVE_BUFFERS as _,

    /// `IORING_OP_RENAMEAT`
    Renameat = sys::io_uring_op::IORING_OP_RENAMEAT as _,

    /// `IORING_OP_SEND`
    Send = sys::io_uring_op::IORING_OP_SEND as _,

    /// `IORING_OP_SENDMSG`
    Sendmsg = sys::io_uring_op::IORING_OP_SENDMSG as _,

    /// `IORING_OP_SHUTDOWN`
    Shutdown = sys::io_uring_op::IORING_OP_SHUTDOWN as _,

    /// `IORING_OP_SPLICE`
    Splice = sys::io_uring_op::IORING_OP_SPLICE as _,

    /// `IORING_OP_STATX`
    Statx = sys::io_uring_op::IORING_OP_STATX as _,

    /// `IORING_OP_SYMLINKAT`
    Symlinkat = sys::io_uring_op::IORING_OP_SYMLINKAT as _,

    /// `IORING_OP_SYNC_FILE_RANGE`
    SyncFileRange = sys::io_uring_op::IORING_OP_SYNC_FILE_RANGE as _,

    /// `IORING_OP_TEE`
    Tee = sys::io_uring_op::IORING_OP_TEE as _,

    /// `IORING_OP_TIMEOUT`
    Timeout = sys::io_uring_op::IORING_OP_TIMEOUT as _,

    /// `IORING_OP_TIMEOUT_REMOVE`
    TimeoutRemove = sys::io_uring_op::IORING_OP_TIMEOUT_REMOVE as _,

    /// `IORING_OP_UNLINKAT`
    Unlinkat = sys::io_uring_op::IORING_OP_UNLINKAT as _,

    /// `IORING_OP_WRITE`
    Write = sys::io_uring_op::IORING_OP_WRITE as _,

    /// `IORING_OP_WRITEV`
    Writev = sys::io_uring_op::IORING_OP_WRITEV as _,

    /// `IORING_OP_WRITE_FIXED`
    WriteFixed = sys::io_uring_op::IORING_OP_WRITE_FIXED as _,

    /// `IORING_OP_MSG_RING`
    MsgRing = sys::io_uring_op::IORING_OP_MSG_RING as _,

    /// `IORING_OP_FSETXATTR`
    Fsetxattr = sys::io_uring_op::IORING_OP_FSETXATTR as _,

    /// `IORING_OP_SETXATTR`
    Setxattr = sys::io_uring_op::IORING_OP_SETXATTR as _,

    /// `IORING_OP_FGETXATTR`
    Fgetxattr = sys::io_uring_op::IORING_OP_FGETXATTR as _,

    /// `IORING_OP_GETXATTR`
    Getxattr = sys::io_uring_op::IORING_OP_GETXATTR as _,

    /// `IORING_OP_SOCKET`
    Socket = sys::io_uring_op::IORING_OP_SOCKET as _,

    /// `IORING_OP_URING_CMD`
    UringCmd = sys::io_uring_op::IORING_OP_URING_CMD as _,

    /// `IORING_OP_SEND_ZC`
    SendZc = sys::io_uring_op::IORING_OP_SEND_ZC as _,

    /// `IORING_OP_SENDMSG_ZC`
    SendmsgZc = sys::io_uring_op::IORING_OP_SENDMSG_ZC as _,

    /// `IORING_OP_READ_MULTISHOT` (since Linux 6.7)
    ReadMultishot = sys::io_uring_op::IORING_OP_READ_MULTISHOT as _,

    /// `IORING_OP_WAITID` (since Linux 6.5)
    Waitid = sys::io_uring_op::IORING_OP_WAITID as _,

    /// `IORING_OP_FUTEX_WAIT` (since Linux 6.7)
    FutexWait = sys::io_uring_op::IORING_OP_FUTEX_WAIT as _,

    /// `IORING_OP_FUTEX_WAKE` (since Linux 6.7)
    FutexWake = sys::io_uring_op::IORING_OP_FUTEX_WAKE as _,

    /// `IORING_OP_FUTEX_WAITV` (since Linux 6.7)
    FutexWaitv = sys::io_uring_op::IORING_OP_FUTEX_WAITV as _,

    /// `IORING_OP_FIXED_FD_INSTALL` (since Linux 6.8)
    FixedFdInstall = sys::io_uring_op::IORING_OP_FIXED_FD_INSTALL as _,

    /// `IORING_OP_FTRUNCATE` (since Linux 6.9)
    Ftruncate = sys::io_uring_op::IORING_OP_FTRUNCATE as _,

    /// `IORING_OP_BIND` (since Linux 6.11)
    Bind = sys::io_uring_op::IORING_OP_BIND as _,

    /// `IORING_OP_LISTEN` (since Linux 6.11)
    Listen = sys::io_uring_op::IORING_OP_LISTEN as _,
}

impl Default for IoringOp {
    #[inline]
    fn default() -> Self {
        Self::Nop
    }
}

/// `IORING_RESTRICTION_*` constants for use with [`io_uring_restriction`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u16)]
#[non_exhaustive]
pub enum IoringRestrictionOp {
    /// `IORING_RESTRICTION_REGISTER_OP`
    RegisterOp = sys::io_uring_register_restriction_op::IORING_RESTRICTION_REGISTER_OP as _,

    /// `IORING_RESTRICTION_SQE_FLAGS_ALLOWED`
    SqeFlagsAllowed =
        sys::io_uring_register_restriction_op::IORING_RESTRICTION_SQE_FLAGS_ALLOWED as _,

    /// `IORING_RESTRICTION_SQE_FLAGS_REQUIRED`
    SqeFlagsRequired =
        sys::io_uring_register_restriction_op::IORING_RESTRICTION_SQE_FLAGS_REQUIRED as _,

    /// `IORING_RESTRICTION_SQE_OP`
    SqeOp = sys::io_uring_register_restriction_op::IORING_RESTRICTION_SQE_OP as _,
}

impl Default for IoringRestrictionOp {
    #[inline]
    fn default() -> Self {
        Self::RegisterOp
    }
}

/// `IORING_MSG_*` constants which represent commands for use with
/// [`IoringOp::MsgRing`], (`seq.addr`)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u64)]
#[non_exhaustive]
pub enum IoringMsgringCmds {
    /// `IORING_MSG_DATA`
    Data = sys::io_uring_msg_ring_flags::IORING_MSG_DATA as _,

    /// `IORING_MSG_SEND_FD`
    SendFd = sys::io_uring_msg_ring_flags::IORING_MSG_SEND_FD as _,
}

bitflags::bitflags! {
    /// `IORING_SETUP_*` flags for use with [`io_uring_params`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringSetupFlags: u32 {
        /// `IORING_SETUP_ATTACH_WQ`
        const ATTACH_WQ = sys::IORING_SETUP_ATTACH_WQ;

        /// `IORING_SETUP_CLAMP`
        const CLAMP = sys::IORING_SETUP_CLAMP;

        /// `IORING_SETUP_CQSIZE`
        const CQSIZE = sys::IORING_SETUP_CQSIZE;

        /// `IORING_SETUP_IOPOLL`
        const IOPOLL = sys::IORING_SETUP_IOPOLL;

        /// `IORING_SETUP_R_DISABLED`
        const R_DISABLED = sys::IORING_SETUP_R_DISABLED;

        /// `IORING_SETUP_SQPOLL`
        const SQPOLL = sys::IORING_SETUP_SQPOLL;

        /// `IORING_SETUP_SQ_AFF`
        const SQ_AFF = sys::IORING_SETUP_SQ_AFF;

        /// `IORING_SETUP_SQE128`
        const SQE128 = sys::IORING_SETUP_SQE128;

        /// `IORING_SETUP_CQE32`
        const CQE32 = sys::IORING_SETUP_CQE32;

        /// `IORING_SETUP_SUBMIT_ALL`
        const SUBMIT_ALL = sys::IORING_SETUP_SUBMIT_ALL;

        /// `IORING_SETUP_COOP_TRASKRUN`
        const COOP_TASKRUN = sys::IORING_SETUP_COOP_TASKRUN;

        /// `IORING_SETUP_TASKRUN_FLAG`
        const TASKRUN_FLAG = sys::IORING_SETUP_TASKRUN_FLAG;

        /// `IORING_SETUP_SINGLE_ISSUER`
        const SINGLE_ISSUER = sys::IORING_SETUP_SINGLE_ISSUER;

        /// `IORING_SETUP_DEFER_TASKRUN`
        const DEFER_TASKRUN = sys::IORING_SETUP_DEFER_TASKRUN;

        /// `IORING_SETUP_NO_MMAP`
        const NO_MMAP = sys::IORING_SETUP_NO_MMAP;

        /// `IORING_SETUP_REGISTERED_FD_ONLY`
        const REGISTERED_FD_ONLY = sys::IORING_SETUP_REGISTERED_FD_ONLY;

        /// `IORING_SETUP_NO_SQARRAY`
        const NO_SQARRAY = sys::IORING_SETUP_NO_SQARRAY;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IOSQE_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringSqeFlags: u8 {
        /// `1 << IOSQE_ASYNC_BIT`
        const ASYNC = 1 << sys::io_uring_sqe_flags_bit::IOSQE_ASYNC_BIT as u8;

        /// `1 << IOSQE_BUFFER_SELECT_BIT`
        const BUFFER_SELECT = 1 << sys::io_uring_sqe_flags_bit::IOSQE_BUFFER_SELECT_BIT as u8;

        /// `1 << IOSQE_FIXED_FILE_BIT`
        const FIXED_FILE = 1 << sys::io_uring_sqe_flags_bit::IOSQE_FIXED_FILE_BIT as u8;

        /// 1 << `IOSQE_IO_DRAIN_BIT`
        const IO_DRAIN = 1 << sys::io_uring_sqe_flags_bit::IOSQE_IO_DRAIN_BIT as u8;

        /// `1 << IOSQE_IO_HARDLINK_BIT`
        const IO_HARDLINK = 1 << sys::io_uring_sqe_flags_bit::IOSQE_IO_HARDLINK_BIT as u8;

        /// `1 << IOSQE_IO_LINK_BIT`
        const IO_LINK = 1 << sys::io_uring_sqe_flags_bit::IOSQE_IO_LINK_BIT as u8;

        /// `1 << IOSQE_CQE_SKIP_SUCCESS_BIT`
        const CQE_SKIP_SUCCESS = 1 << sys::io_uring_sqe_flags_bit::IOSQE_CQE_SKIP_SUCCESS_BIT as u8;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_CQE_F_*` flags for use with [`io_uring_cqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringCqeFlags: u32 {
        /// `IORING_CQE_F_BUFFER`
        const BUFFER = bitcast!(sys::IORING_CQE_F_BUFFER);

        /// `IORING_CQE_F_MORE`
        const MORE = bitcast!(sys::IORING_CQE_F_MORE);

        /// `IORING_CQE_F_SOCK_NONEMPTY`
        const SOCK_NONEMPTY = bitcast!(sys::IORING_CQE_F_SOCK_NONEMPTY);

        /// `IORING_CQE_F_NOTIF`
        const NOTIF = bitcast!(sys::IORING_CQE_F_NOTIF);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_FSYNC_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringFsyncFlags: u32 {
        /// `IORING_FSYNC_DATASYNC`
        const DATASYNC = sys::IORING_FSYNC_DATASYNC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_TIMEOUT_*` and `IORING_LINK_TIMEOUT_UPDATE` flags for use with
    /// [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringTimeoutFlags: u32 {
        /// `IORING_TIMEOUT_ABS`
        const ABS = sys::IORING_TIMEOUT_ABS;

        /// `IORING_TIMEOUT_UPDATE`
        const UPDATE = sys::IORING_TIMEOUT_UPDATE;

        /// `IORING_TIMEOUT_BOOTTIME`
        const BOOTTIME = sys::IORING_TIMEOUT_BOOTTIME;

        /// `IORING_TIMEOUT_ETIME_SUCCESS`
        const ETIME_SUCCESS = sys::IORING_TIMEOUT_ETIME_SUCCESS;

        /// `IORING_TIMEOUT_REALTIME`
        const REALTIME = sys::IORING_TIMEOUT_REALTIME;

        /// `IORING_TIMEOUT_CLOCK_MASK`
        const CLOCK_MASK = sys::IORING_TIMEOUT_CLOCK_MASK;

        /// `IORING_TIMEOUT_UPDATE_MASK`
        const UPDATE_MASK = sys::IORING_TIMEOUT_UPDATE_MASK;

        /// `IORING_LINK_TIMEOUT_UPDATE`
        const LINK_TIMEOUT_UPDATE = sys::IORING_LINK_TIMEOUT_UPDATE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `SPLICE_F_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SpliceFlags: u32 {
        /// `SPLICE_F_FD_IN_FIXED`
        const FD_IN_FIXED = sys::SPLICE_F_FD_IN_FIXED;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_MSG_RING_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringMsgringFlags: u32 {
        /// `IORING_MSG_RING_CQE_SKIP`
        const CQE_SKIP = sys::IORING_MSG_RING_CQE_SKIP;

        /// `IORING_MSG_RING_FLAGS_PASS`
        const FLAGS_PASS = sys::IORING_MSG_RING_FLAGS_PASS;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_URING_CMD_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringUringCmdFlags: u32 {
        /// `IORING_URING_CMD_FIXED`
        const FIXED = sys::IORING_URING_CMD_FIXED;

        /// `IORING_URING_CMD_MASK`
        const MASK = sys::IORING_URING_CMD_MASK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_ASYNC_CANCEL_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringAsyncCancelFlags: u32 {
        /// `IORING_ASYNC_CANCEL_ALL`
        const ALL = sys::IORING_ASYNC_CANCEL_ALL;

        /// `IORING_ASYNC_CANCEL_FD`
        const FD = sys::IORING_ASYNC_CANCEL_FD;

        /// `IORING_ASYNC_CANCEL_FD`
        const ANY = sys::IORING_ASYNC_CANCEL_ANY;

        /// `IORING_ASYNC_CANCEL_FD`
        const FD_FIXED = sys::IORING_ASYNC_CANCEL_FD_FIXED;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_FIXED_FD_*` flags for use with [`io_uring_sqe`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringFixedFdFlags: u32 {
        /// `IORING_FIXED_FD_NO_CLOEXEC`
        const NO_CLOEXEC = sys::IORING_FIXED_FD_NO_CLOEXEC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_FEAT_*` flags for use with [`io_uring_params`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringFeatureFlags: u32 {
        /// `IORING_FEAT_CQE_SKIP`
        const CQE_SKIP = sys::IORING_FEAT_CQE_SKIP;

        /// `IORING_FEAT_CUR_PERSONALITY`
        const CUR_PERSONALITY = sys::IORING_FEAT_CUR_PERSONALITY;

        /// `IORING_FEAT_EXT_ARG`
        const EXT_ARG = sys::IORING_FEAT_EXT_ARG;

        /// `IORING_FEAT_FAST_POLL`
        const FAST_POLL = sys::IORING_FEAT_FAST_POLL;

        /// `IORING_FEAT_NATIVE_WORKERS`
        const NATIVE_WORKERS = sys::IORING_FEAT_NATIVE_WORKERS;

        /// `IORING_FEAT_NODROP`
        const NODROP = sys::IORING_FEAT_NODROP;

        /// `IORING_FEAT_POLL_32BITS`
        const POLL_32BITS = sys::IORING_FEAT_POLL_32BITS;

        /// `IORING_FEAT_RSRC_TAGS`
        const RSRC_TAGS = sys::IORING_FEAT_RSRC_TAGS;

        /// `IORING_FEAT_RW_CUR_POS`
        const RW_CUR_POS = sys::IORING_FEAT_RW_CUR_POS;

        /// `IORING_FEAT_SINGLE_MMAP`
        const SINGLE_MMAP = sys::IORING_FEAT_SINGLE_MMAP;

        /// `IORING_FEAT_SQPOLL_NONFIXED`
        const SQPOLL_NONFIXED = sys::IORING_FEAT_SQPOLL_NONFIXED;

        /// `IORING_FEAT_SUBMIT_STABLE`
        const SUBMIT_STABLE = sys::IORING_FEAT_SUBMIT_STABLE;

        /// `IORING_FEAT_LINKED_FILE`
        const LINKED_FILE = sys::IORING_FEAT_LINKED_FILE;

        /// `IORING_FEAT_REG_REG_RING`
        const REG_REG_RING = sys::IORING_FEAT_REG_REG_RING;

        /// `IORING_FEAT_RECVSEND_BUNDLE`
        const RECVSEND_BUNDLE = sys::IORING_FEAT_RECVSEND_BUNDLE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IO_URING_OP_*` flags for use with [`io_uring_probe_op`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringOpFlags: u16 {
        /// `IO_URING_OP_SUPPORTED`
        const SUPPORTED = sys::IO_URING_OP_SUPPORTED as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_RSRC_*` flags for use with [`io_uring_rsrc_register`].
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringRsrcFlags: u32 {
        /// `IORING_RSRC_REGISTER_SPARSE`
        const REGISTER_SPARSE = sys::IORING_RSRC_REGISTER_SPARSE as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_SQ_*` flags.
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringSqFlags: u32 {
        /// `IORING_SQ_NEED_WAKEUP`
        const NEED_WAKEUP = sys::IORING_SQ_NEED_WAKEUP;

        /// `IORING_SQ_CQ_OVERFLOW`
        const CQ_OVERFLOW = sys::IORING_SQ_CQ_OVERFLOW;

        /// `IORING_SQ_TASKRUN`
        const TASKRUN = sys::IORING_SQ_TASKRUN;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_CQ_*` flags.
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringCqFlags: u32 {
        /// `IORING_CQ_EVENTFD_DISABLED`
        const EVENTFD_DISABLED = sys::IORING_CQ_EVENTFD_DISABLED;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// `IORING_POLL_*` flags.
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringPollFlags: u32 {
        /// `IORING_POLL_ADD_MULTI`
        const ADD_MULTI = sys::IORING_POLL_ADD_MULTI;

        /// `IORING_POLL_UPDATE_EVENTS`
        const UPDATE_EVENTS = sys::IORING_POLL_UPDATE_EVENTS;

        /// `IORING_POLL_UPDATE_USER_DATA`
        const UPDATE_USER_DATA = sys::IORING_POLL_UPDATE_USER_DATA;

        /// `IORING_POLL_ADD_LEVEL`
        const ADD_LEVEL = sys::IORING_POLL_ADD_LEVEL;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// send/sendmsg flags (`sqe.ioprio`)
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringSendFlags: u16 {
        /// `IORING_RECVSEND_POLL_FIRST`.
        ///
        /// See also [`IoringRecvFlags::POLL_FIRST`].
        const POLL_FIRST = sys::IORING_RECVSEND_POLL_FIRST as _;

        /// `IORING_RECVSEND_FIXED_BUF`
        ///
        /// See also [`IoringRecvFlags::FIXED_BUF`].
        const FIXED_BUF = sys::IORING_RECVSEND_FIXED_BUF as _;

        /// `IORING_SEND_ZC_REPORT_USAGE` (since Linux 6.2)
        const ZC_REPORT_USAGE = sys::IORING_SEND_ZC_REPORT_USAGE as _;

        /// `IORING_RECVSEND_BUNDLE`
        ///
        /// See also [`IoringRecvFlags::BUNDLE`].
        const BUNDLE = sys::IORING_RECVSEND_BUNDLE as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// recv/recvmsg flags (`sqe.ioprio`)
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringRecvFlags: u16 {
        /// `IORING_RECVSEND_POLL_FIRST`
        ///
        /// See also [`IoringSendFlags::POLL_FIRST`].
        const POLL_FIRST = sys::IORING_RECVSEND_POLL_FIRST as _;

        /// `IORING_RECV_MULTISHOT`
        const MULTISHOT = sys::IORING_RECV_MULTISHOT as _;

        /// `IORING_RECVSEND_FIXED_BUF`
        ///
        /// See also [`IoringSendFlags::FIXED_BUF`].
        const FIXED_BUF = sys::IORING_RECVSEND_FIXED_BUF as _;

        /// `IORING_RECVSEND_BUNDLE`
        ///
        /// See also [`IoringSendFlags::BUNDLE`].
        const BUNDLE = sys::IORING_RECVSEND_BUNDLE as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// accept flags (`sqe.ioprio`)
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct IoringAcceptFlags: u16 {
        /// `IORING_ACCEPT_MULTISHOT`
        const MULTISHOT = sys::IORING_ACCEPT_MULTISHOT as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// recvmsg out flags
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct RecvmsgOutFlags: u32 {
        /// `MSG_EOR`
        const EOR = net::MSG_EOR;

        /// `MSG_TRUNC`
        const TRUNC = net::MSG_TRUNC;

        /// `MSG_CTRUNC`
        const CTRUNC = net::MSG_CTRUNC;

        /// `MSG_OOB`
        const OOB = net::MSG_OOB;

        /// `MSG_ERRQUEUE`
        const ERRQUEUE = net::MSG_ERRQUEUE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[allow(missing_docs)]
pub const IORING_CQE_BUFFER_SHIFT: u32 = sys::IORING_CQE_BUFFER_SHIFT as _;
#[allow(missing_docs)]
pub const IORING_FILE_INDEX_ALLOC: i32 = sys::IORING_FILE_INDEX_ALLOC as _;

// Re-export these as `u64`, which is the `offset` type in `rustix::io::mmap`.
#[allow(missing_docs)]
pub const IORING_OFF_SQ_RING: u64 = sys::IORING_OFF_SQ_RING as _;
#[allow(missing_docs)]
pub const IORING_OFF_CQ_RING: u64 = sys::IORING_OFF_CQ_RING as _;
#[allow(missing_docs)]
pub const IORING_OFF_SQES: u64 = sys::IORING_OFF_SQES as _;

/// `IORING_REGISTER_FILES_SKIP`
// SAFETY: `IORING_REGISTER_FILES_SKIP` is a reserved value that is never
// dynamically allocated, so it'll remain valid for the duration of
// `'static`.
pub const IORING_REGISTER_FILES_SKIP: BorrowedFd<'static> =
    unsafe { BorrowedFd::<'static>::borrow_raw(sys::IORING_REGISTER_FILES_SKIP as RawFd) };

/// `IORING_NOTIF_USAGE_ZC_COPIED` (since Linux 6.2)
pub const IORING_NOTIF_USAGE_ZC_COPIED: i32 = sys::IORING_NOTIF_USAGE_ZC_COPIED as _;

/// A pointer in the io_uring API.
///
/// `io_uring`'s native API represents pointers as `u64` values. In order to
/// preserve strict-provenance, use a `*mut c_void`. On platforms where
/// pointers are narrower than 64 bits, this requires additional padding.
#[repr(C)]
#[cfg_attr(any(target_arch = "arm", target_arch = "powerpc"), repr(align(8)))]
#[derive(Copy, Clone)]
#[non_exhaustive]
pub struct io_uring_ptr {
    #[cfg(all(target_pointer_width = "32", target_endian = "big"))]
    #[doc(hidden)]
    pub __pad32: u32,
    #[cfg(all(target_pointer_width = "16", target_endian = "big"))]
    #[doc(hidden)]
    pub __pad16: u16,

    /// The pointer value.
    pub ptr: *mut c_void,

    #[cfg(all(target_pointer_width = "16", target_endian = "little"))]
    #[doc(hidden)]
    pub __pad16: u16,
    #[cfg(all(target_pointer_width = "32", target_endian = "little"))]
    #[doc(hidden)]
    pub __pad32: u32,
}

impl io_uring_ptr {
    /// Construct a null `io_uring_ptr`.
    #[inline]
    pub const fn null() -> Self {
        Self::new(null_mut())
    }

    /// Construct a new `io_uring_ptr`.
    #[inline]
    pub const fn new(ptr: *mut c_void) -> Self {
        Self {
            ptr,

            #[cfg(target_pointer_width = "16")]
            __pad16: 0,
            #[cfg(any(target_pointer_width = "16", target_pointer_width = "32"))]
            __pad32: 0,
        }
    }
}

impl From<*mut c_void> for io_uring_ptr {
    #[inline]
    fn from(ptr: *mut c_void) -> Self {
        Self::new(ptr)
    }
}

impl PartialEq for io_uring_ptr {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.ptr.eq(&other.ptr)
    }
}

impl Eq for io_uring_ptr {}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for io_uring_ptr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.ptr.partial_cmp(&other.ptr)
    }
}

impl Ord for io_uring_ptr {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.ptr.cmp(&other.ptr)
    }
}

impl Hash for io_uring_ptr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ptr.hash(state)
    }
}

impl Default for io_uring_ptr {
    #[inline]
    fn default() -> Self {
        Self::null()
    }
}

impl core::fmt::Pointer for io_uring_ptr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.ptr.fmt(f)
    }
}

impl core::fmt::Debug for io_uring_ptr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.ptr.fmt(f)
    }
}

/// User data in the io_uring API.
///
/// `io_uring`'s native API represents `user_data` fields as `u64` values. In
/// order to preserve strict-provenance, use a union which allows users to
/// optionally store pointers.
#[repr(C)]
#[derive(Copy, Clone)]
pub union io_uring_user_data {
    /// An arbitrary `u64`.
    pub u64_: u64,

    /// A pointer.
    pub ptr: io_uring_ptr,
}

impl io_uring_user_data {
    /// Create a zero-initialized `Self`.
    pub const fn zeroed() -> Self {
        // Initialize the `u64_` field, which is the size of the full union.
        // This can use `core::mem::zeroed` in Rust 1.75.
        Self { u64_: 0 }
    }

    /// Return the `u64` value.
    #[inline]
    pub const fn u64_(self) -> u64 {
        // SAFETY: All the fields have the same underlying representation.
        unsafe { self.u64_ }
    }

    /// Create a `Self` from a `u64` value.
    #[inline]
    pub const fn from_u64(u64_: u64) -> Self {
        Self { u64_ }
    }

    /// Return the `ptr` pointer value.
    #[inline]
    pub const fn ptr(self) -> *mut c_void {
        // SAFETY: All the fields have the same underlying representation.
        unsafe { self.ptr }.ptr
    }

    /// Create a `Self` from a pointer value.
    #[inline]
    pub const fn from_ptr(ptr: *mut c_void) -> Self {
        Self {
            ptr: io_uring_ptr::new(ptr),
        }
    }
}

impl From<u64> for io_uring_user_data {
    #[inline]
    fn from(u64_: u64) -> Self {
        Self::from_u64(u64_)
    }
}

impl From<*mut c_void> for io_uring_user_data {
    #[inline]
    fn from(ptr: *mut c_void) -> Self {
        Self::from_ptr(ptr)
    }
}

impl PartialEq for io_uring_user_data {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: `io_uring_ptr` and `u64` have the same layout.
        unsafe { self.u64_.eq(&other.u64_) }
    }
}

impl Eq for io_uring_user_data {}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for io_uring_user_data {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // SAFETY: `io_uring_ptr` and `u64` have the same layout.
        unsafe { self.u64_.partial_cmp(&other.u64_) }
    }
}

impl Ord for io_uring_user_data {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // SAFETY: `io_uring_ptr` and `u64` have the same layout.
        unsafe { self.u64_.cmp(&other.u64_) }
    }
}

impl Hash for io_uring_user_data {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        // SAFETY: `io_uring_ptr` and `u64` have the same layout.
        unsafe { self.u64_.hash(state) }
    }
}

impl Default for io_uring_user_data {
    #[inline]
    fn default() -> Self {
        Self::zeroed()
    }
}

impl core::fmt::Debug for io_uring_user_data {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // SAFETY: Just format as a `u64`, since formatting doesn't preserve
        // provenance, and we don't have a discriminant.
        unsafe { self.u64_.fmt(f) }
    }
}

/// An io_uring Submission Queue Entry.
#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct io_uring_sqe {
    pub opcode: IoringOp,
    pub flags: IoringSqeFlags,
    pub ioprio: ioprio_union,
    pub fd: RawFd,
    pub off_or_addr2: off_or_addr2_union,
    pub addr_or_splice_off_in: addr_or_splice_off_in_union,
    pub len: len_union,
    pub op_flags: op_flags_union,
    pub user_data: io_uring_user_data,
    pub buf: buf_union,
    pub personality: u16,
    pub splice_fd_in_or_file_index_or_addr_len: splice_fd_in_or_file_index_or_addr_len_union,
    pub addr3_or_cmd: addr3_or_cmd_union,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union ioprio_union {
    pub recv_flags: IoringRecvFlags,
    pub send_flags: IoringSendFlags,
    pub accept_flags: IoringAcceptFlags,
    pub ioprio: u16,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union len_union {
    pub poll_flags: IoringPollFlags,
    pub len: u32,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union addr3_or_cmd_union {
    pub addr3: addr3_struct,
    pub cmd: [u8; 0],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone, Default)]
#[non_exhaustive]
pub struct addr3_struct {
    pub addr3: u64,
    #[doc(hidden)]
    pub __pad2: [u64; 1],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union off_or_addr2_union {
    pub off: u64,
    pub addr2: io_uring_ptr,
    pub cmd_op: cmd_op_struct,
    pub user_data: io_uring_user_data,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
#[non_exhaustive]
pub struct cmd_op_struct {
    pub cmd_op: u32,
    #[doc(hidden)]
    pub __pad1: u32,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union addr_or_splice_off_in_union {
    pub addr: io_uring_ptr,
    pub splice_off_in: u64,
    pub msgring_cmd: IoringMsgringCmds,
    pub user_data: io_uring_user_data,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union op_flags_union {
    pub rw_flags: crate::io::ReadWriteFlags,
    pub fsync_flags: IoringFsyncFlags,
    pub poll_events: u16,
    pub poll32_events: u32,
    pub sync_range_flags: u32,
    /// `msg_flags` is split into `send_flags` and `recv_flags`.
    #[doc(alias = "msg_flags")]
    pub send_flags: SendFlags,
    /// `msg_flags` is split into `send_flags` and `recv_flags`.
    #[doc(alias = "msg_flags")]
    pub recv_flags: RecvFlags,
    pub timeout_flags: IoringTimeoutFlags,
    pub accept_flags: SocketFlags,
    pub cancel_flags: IoringAsyncCancelFlags,
    pub open_flags: OFlags,
    pub statx_flags: AtFlags,
    pub fadvise_advice: Advice,
    pub splice_flags: SpliceFlags,
    pub rename_flags: RenameFlags,
    pub unlink_flags: AtFlags,
    pub hardlink_flags: AtFlags,
    pub xattr_flags: XattrFlags,
    pub msg_ring_flags: IoringMsgringFlags,
    pub uring_cmd_flags: IoringUringCmdFlags,
    pub futex_flags: FutexWaitvFlags,
    pub install_fd_flags: IoringFixedFdFlags,
}

#[allow(missing_docs)]
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub union buf_union {
    pub buf_index: u16,
    pub buf_group: u16,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union splice_fd_in_or_file_index_or_addr_len_union {
    pub splice_fd_in: i32,
    pub file_index: u32,
    pub addr_len: addr_len_struct,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
#[non_exhaustive]
pub struct addr_len_struct {
    pub addr_len: u16,
    #[doc(hidden)]
    pub __pad3: [u16; 1],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
#[non_exhaustive]
pub struct io_uring_sync_cancel_reg {
    pub addr: io_uring_user_data,
    pub fd: i32,
    pub flags: IoringAsyncCancelFlags,
    pub timeout: Timespec,
    pub opcode: u8,
    #[doc(hidden)]
    pub pad: [u8; 7],
    #[doc(hidden)]
    pub pad2: [u64; 3],
}

impl Default for io_uring_sync_cancel_reg {
    #[inline]
    fn default() -> Self {
        Self {
            addr: Default::default(),
            fd: Default::default(),
            flags: Default::default(),
            timeout: Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            opcode: Default::default(),
            pad: Default::default(),
            pad2: Default::default(),
        }
    }
}

/// An io_uring Completion Queue Entry.
///
/// This does not derive `Copy` or `Clone` because the `big_cqe` field is not
/// automatically copyable.
#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct io_uring_cqe {
    pub user_data: io_uring_user_data,
    pub res: i32,
    pub flags: IoringCqeFlags,
    pub big_cqe: IncompleteArrayField<u64>,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_restriction {
    pub opcode: IoringRestrictionOp,
    pub register_or_sqe_op_or_sqe_flags: register_or_sqe_op_or_sqe_flags_union,
    #[doc(hidden)]
    pub resv: u8,
    #[doc(hidden)]
    pub resv2: [u32; 3],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Copy, Clone)]
pub union register_or_sqe_op_or_sqe_flags_union {
    pub register_op: IoringRegisterOp,
    pub sqe_op: IoringOp,
    pub sqe_flags: IoringSqeFlags,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_params {
    pub sq_entries: u32,
    pub cq_entries: u32,
    pub flags: IoringSetupFlags,
    pub sq_thread_cpu: u32,
    pub sq_thread_idle: u32,
    pub features: IoringFeatureFlags,
    pub wq_fd: RawFd,
    #[doc(hidden)]
    pub resv: [u32; 3],
    pub sq_off: io_sqring_offsets,
    pub cq_off: io_cqring_offsets,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_sqring_offsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub flags: u32,
    pub dropped: u32,
    pub array: u32,
    #[doc(hidden)]
    pub resv1: u32,
    pub user_addr: io_uring_ptr,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_cqring_offsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub overflow: u32,
    pub cqes: u32,
    pub flags: u32,
    #[doc(hidden)]
    pub resv1: u32,
    pub user_addr: io_uring_ptr,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct io_uring_probe {
    pub last_op: IoringOp,
    pub ops_len: u8,
    #[doc(hidden)]
    pub resv: u16,
    #[doc(hidden)]
    pub resv2: [u32; 3],
    pub ops: IncompleteArrayField<io_uring_probe_op>,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_probe_op {
    pub op: IoringOp,
    #[doc(hidden)]
    pub resv: u8,
    pub flags: IoringOpFlags,
    #[doc(hidden)]
    pub resv2: u32,
}

#[allow(missing_docs)]
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_files_update {
    pub offset: u32,
    #[doc(hidden)]
    pub resv: u32,
    pub fds: io_uring_ptr,
}

#[allow(missing_docs)]
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_rsrc_register {
    pub nr: u32,
    pub flags: IoringRsrcFlags,
    #[doc(hidden)]
    pub resv2: u64,
    pub data: io_uring_ptr,
    pub tags: io_uring_ptr,
}

#[allow(missing_docs)]
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_rsrc_update {
    pub offset: u32,
    #[doc(hidden)]
    pub resv: u32,
    pub data: io_uring_ptr,
}

#[allow(missing_docs)]
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_rsrc_update2 {
    pub offset: u32,
    #[doc(hidden)]
    pub resv: u32,
    pub data: io_uring_ptr,
    pub tags: io_uring_ptr,
    pub nr: u32,
    #[doc(hidden)]
    pub resv2: u32,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct io_uring_getevents_arg {
    pub sigmask: io_uring_ptr,
    pub sigmask_sz: u32,
    pub min_wait_usec: u32,
    pub ts: io_uring_ptr,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct io_uring_recvmsg_out {
    pub namelen: SocketAddrLen,
    pub controllen: u32,
    pub payloadlen: u32,
    pub flags: RecvmsgOutFlags,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct iovec {
    pub iov_base: *mut c_void,
    pub iov_len: usize,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct open_how {
    /// An [`OFlags`] value represented as a `u64`.
    pub flags: u64,

    /// A [`Mode`] value represented as a `u64`.
    pub mode: u64,

    pub resolve: ResolveFlags,
}

impl open_how {
    /// Create a zero-initialized `Self`.
    pub const fn zeroed() -> Self {
        Self {
            flags: 0,
            mode: 0,
            resolve: ResolveFlags::empty(),
        }
    }
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_buf_reg {
    pub ring_addr: io_uring_ptr,
    pub ring_entries: u32,
    pub bgid: u16,
    pub flags: u16,
    #[doc(hidden)]
    pub resv: [u64; 3_usize],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct io_uring_buf {
    pub addr: io_uring_ptr,
    pub len: u32,
    pub bid: u16,
    #[doc(hidden)]
    pub resv: u16,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
#[non_exhaustive]
pub struct buf_ring_tail_struct {
    #[doc(hidden)]
    pub resv1: u64,
    #[doc(hidden)]
    pub resv2: u32,
    #[doc(hidden)]
    pub resv3: u16,
    pub tail: u16,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct buf_ring_bufs_struct {
    pub bufs: IncompleteArrayField<io_uring_buf>,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct tail_or_bufs_struct {
    pub tail: UnionField<buf_ring_tail_struct>,
    pub bufs: UnionField<buf_ring_bufs_struct>,
    pub union_field: [u64; 2],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct io_uring_buf_ring {
    pub tail_or_bufs: tail_or_bufs_struct,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct io_uring_napi {
    pub busy_poll_to: u32,
    pub prefer_busy_poll: u8,
    pub opcode: u8,
    #[doc(hidden)]
    pub pad: [u8; 2],
    pub op_param: u32,
    #[doc(hidden)]
    pub resv: u32,
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct io_uring_clone_buffers {
    pub src_fd: u32,
    pub flags: u32,
    pub src_off: u32,
    pub dst_off: u32,
    pub nr: u32,
    #[doc(hidden)]
    pub pad: [u32; 3],
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct io_uring_reg_wait {
    pub ts: Timespec,
    pub min_wait_usec: u32,
    pub flags: u32,
    pub sigmask: io_uring_ptr,
    pub sigmask_sz: u32,
    #[doc(hidden)]
    pub pad: [u32; 3],
    #[doc(hidden)]
    pub pad2: [u64; 2],
}

impl Default for ioprio_union {
    #[inline]
    fn default() -> Self {
        default_union!(ioprio_union, ioprio)
    }
}

impl Default for len_union {
    #[inline]
    fn default() -> Self {
        default_union!(len_union, len)
    }
}

impl Default for off_or_addr2_union {
    #[inline]
    fn default() -> Self {
        default_union!(off_or_addr2_union, off)
    }
}

impl Default for addr_or_splice_off_in_union {
    #[inline]
    fn default() -> Self {
        default_union!(addr_or_splice_off_in_union, splice_off_in)
    }
}

impl Default for addr3_or_cmd_union {
    #[inline]
    fn default() -> Self {
        default_union!(addr3_or_cmd_union, addr3)
    }
}

impl Default for op_flags_union {
    #[inline]
    fn default() -> Self {
        default_union!(op_flags_union, sync_range_flags)
    }
}

impl Default for buf_union {
    #[inline]
    fn default() -> Self {
        default_union!(buf_union, buf_index)
    }
}

impl Default for splice_fd_in_or_file_index_or_addr_len_union {
    #[inline]
    fn default() -> Self {
        default_union!(splice_fd_in_or_file_index_or_addr_len_union, splice_fd_in)
    }
}

impl Default for register_or_sqe_op_or_sqe_flags_union {
    #[inline]
    fn default() -> Self {
        default_union!(register_or_sqe_op_or_sqe_flags_union, sqe_flags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd::AsRawFd as _;

    /// Check that our custom structs and unions have the same layout as the
    /// kernel's versions.
    #[test]
    fn io_uring_layouts() {
        use sys as c;

        // `io_uring_ptr` is a replacement for `u64`.
        assert_eq_size!(io_uring_ptr, u64);
        assert_eq_align!(io_uring_ptr, u64);

        // Test that pointers are stored in `io_uring_ptr` in the way that
        // io_uring stores them in a `u64`.
        unsafe {
            const MAGIC: u64 = !0x0123_4567_89ab_cdef;
            let ptr = io_uring_ptr::new(MAGIC as usize as *mut c_void);
            assert_eq!(ptr.ptr, MAGIC as usize as *mut c_void);
            #[cfg(target_pointer_width = "16")]
            assert_eq!(ptr.__pad16, 0);
            #[cfg(any(target_pointer_width = "16", target_pointer_width = "32"))]
            assert_eq!(ptr.__pad32, 0);
            let int = core::mem::transmute::<io_uring_ptr, u64>(ptr);
            assert_eq!(int, MAGIC as usize as u64);
        }

        // `io_uring_user_data` is a replacement for `u64`.
        assert_eq_size!(io_uring_user_data, u64);
        assert_eq_align!(io_uring_user_data, u64);

        // Test that `u64`s and pointers are properly stored in
        // `io_uring_user_data`.
        unsafe {
            const MAGIC: u64 = !0x0123_4567_89ab_cdef;
            let user_data = io_uring_user_data::from_u64(MAGIC);
            assert_eq!(user_data.u64_(), MAGIC);
            assert_eq!(
                core::mem::transmute::<io_uring_user_data, u64>(user_data),
                MAGIC
            );
            let user_data = io_uring_user_data::from_ptr(MAGIC as usize as *mut c_void);
            assert_eq!(user_data.ptr(), MAGIC as usize as *mut c_void);
            assert_eq!(
                core::mem::transmute::<io_uring_user_data, u64>(user_data),
                MAGIC as usize as u64
            );
        }

        check_renamed_type!(off_or_addr2_union, io_uring_sqe__bindgen_ty_1);
        check_renamed_type!(addr_or_splice_off_in_union, io_uring_sqe__bindgen_ty_2);
        check_renamed_type!(addr3_or_cmd_union, io_uring_sqe__bindgen_ty_6);
        check_renamed_type!(op_flags_union, io_uring_sqe__bindgen_ty_3);
        check_renamed_type!(buf_union, io_uring_sqe__bindgen_ty_4);
        check_renamed_type!(
            splice_fd_in_or_file_index_or_addr_len_union,
            io_uring_sqe__bindgen_ty_5
        );
        check_renamed_type!(addr_len_struct, io_uring_sqe__bindgen_ty_5__bindgen_ty_1);
        check_renamed_type!(
            register_or_sqe_op_or_sqe_flags_union,
            io_uring_restriction__bindgen_ty_1
        );

        check_renamed_type!(addr3_struct, io_uring_sqe__bindgen_ty_6__bindgen_ty_1);
        check_renamed_type!(cmd_op_struct, io_uring_sqe__bindgen_ty_1__bindgen_ty_1);

        check_type!(io_uring_sqe);
        check_struct_field!(io_uring_sqe, opcode);
        check_struct_field!(io_uring_sqe, flags);
        check_struct_field!(io_uring_sqe, ioprio);
        check_struct_field!(io_uring_sqe, fd);
        check_struct_renamed_field!(io_uring_sqe, off_or_addr2, __bindgen_anon_1);
        check_struct_renamed_field!(io_uring_sqe, addr_or_splice_off_in, __bindgen_anon_2);
        check_struct_field!(io_uring_sqe, len);
        check_struct_renamed_field!(io_uring_sqe, op_flags, __bindgen_anon_3);
        check_struct_field!(io_uring_sqe, user_data);
        check_struct_renamed_field!(io_uring_sqe, buf, __bindgen_anon_4);
        check_struct_field!(io_uring_sqe, personality);
        check_struct_renamed_field!(
            io_uring_sqe,
            splice_fd_in_or_file_index_or_addr_len,
            __bindgen_anon_5
        );
        check_struct_renamed_field!(io_uring_sqe, addr3_or_cmd, __bindgen_anon_6);

        check_type!(io_uring_restriction);
        check_struct_field!(io_uring_restriction, opcode);
        check_struct_renamed_field!(
            io_uring_restriction,
            register_or_sqe_op_or_sqe_flags,
            __bindgen_anon_1
        );
        check_struct_field!(io_uring_restriction, resv);
        check_struct_field!(io_uring_restriction, resv2);

        check_struct!(io_uring_cqe, user_data, res, flags, big_cqe);
        check_struct!(
            io_uring_params,
            sq_entries,
            cq_entries,
            flags,
            sq_thread_cpu,
            sq_thread_idle,
            features,
            wq_fd,
            resv,
            sq_off,
            cq_off
        );
        check_struct!(
            io_sqring_offsets,
            head,
            tail,
            ring_mask,
            ring_entries,
            flags,
            dropped,
            array,
            resv1,
            user_addr
        );
        check_struct!(
            io_cqring_offsets,
            head,
            tail,
            ring_mask,
            ring_entries,
            overflow,
            cqes,
            flags,
            resv1,
            user_addr
        );
        check_struct!(io_uring_recvmsg_out, namelen, controllen, payloadlen, flags);
        check_struct!(io_uring_probe, last_op, ops_len, resv, resv2, ops);
        check_struct!(io_uring_probe_op, op, resv, flags, resv2);
        check_struct!(io_uring_files_update, offset, resv, fds);
        check_struct!(io_uring_rsrc_register, nr, flags, resv2, data, tags);
        check_struct!(io_uring_rsrc_update, offset, resv, data);
        check_struct!(io_uring_rsrc_update2, offset, resv, data, tags, nr, resv2);
        check_struct!(
            io_uring_getevents_arg,
            sigmask,
            sigmask_sz,
            min_wait_usec,
            ts
        );
        check_struct!(iovec, iov_base, iov_len);
        check_struct!(open_how, flags, mode, resolve);
        check_struct!(io_uring_buf_reg, ring_addr, ring_entries, bgid, flags, resv);
        check_struct!(io_uring_buf, addr, len, bid, resv);
        check_struct!(
            io_uring_sync_cancel_reg,
            addr,
            fd,
            flags,
            timeout,
            opcode,
            pad,
            pad2
        );

        check_renamed_type!(tail_or_bufs_struct, io_uring_buf_ring__bindgen_ty_1);
        check_renamed_type!(
            buf_ring_tail_struct,
            io_uring_buf_ring__bindgen_ty_1__bindgen_ty_1
        );
        check_renamed_type!(
            buf_ring_bufs_struct,
            io_uring_buf_ring__bindgen_ty_1__bindgen_ty_2
        );
        check_struct_renamed_field!(io_uring_buf_ring, tail_or_bufs, __bindgen_anon_1);

        check_struct!(
            io_uring_napi,
            busy_poll_to,
            prefer_busy_poll,
            opcode,
            pad,
            op_param,
            resv
        );
        check_struct!(
            io_uring_clone_buffers,
            src_fd,
            flags,
            src_off,
            dst_off,
            nr,
            pad
        );
        check_struct!(
            io_uring_reg_wait,
            ts,
            min_wait_usec,
            flags,
            sigmask,
            sigmask_sz,
            pad,
            pad2
        );

        check_renamed_struct!(
            MsgHdr,
            msghdr,
            msg_name,
            msg_namelen,
            msg_iov,
            msg_iovlen,
            msg_control,
            msg_controllen,
            msg_flags
        );
    }

    #[test]
    fn test_io_uring_register_files_skip() {
        use crate::backend::c;
        assert!(IORING_REGISTER_FILES_SKIP.as_raw_fd() != -1);
        assert!(IORING_REGISTER_FILES_SKIP.as_raw_fd() != c::STDIN_FILENO);
        assert!(IORING_REGISTER_FILES_SKIP.as_raw_fd() != c::STDOUT_FILENO);
        assert!(IORING_REGISTER_FILES_SKIP.as_raw_fd() != c::STDERR_FILENO);
    }
}
