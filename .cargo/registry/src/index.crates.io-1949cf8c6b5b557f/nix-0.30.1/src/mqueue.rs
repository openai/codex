//! Posix Message Queue functions
//!
//! # Example
//!
// no_run because a kernel module may be required.
//! ```no_run
//! # use std::ffi::CString;
//! # use nix::mqueue::*;
//! use nix::sys::stat::Mode;
//!
//! const MSG_SIZE: mq_attr_member_t = 32;
//! let mq_name= "/a_nix_test_queue";
//!
//! let oflag0 = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
//! let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
//! let mqd0 = mq_open(mq_name, oflag0, mode, None).unwrap();
//! let msg_to_send = b"msg_1";
//! mq_send(&mqd0, msg_to_send, 1).unwrap();
//!
//! let oflag1 = MQ_OFlag::O_CREAT | MQ_OFlag::O_RDONLY;
//! let mqd1 = mq_open(mq_name, oflag1, mode, None).unwrap();
//! let mut buf = [0u8; 32];
//! let mut prio = 0u32;
//! let len = mq_receive(&mqd1, &mut buf, &mut prio).unwrap();
//! assert_eq!(prio, 1);
//! assert_eq!(msg_to_send, &buf[0..len]);
//!
//! mq_close(mqd1).unwrap();
//! mq_close(mqd0).unwrap();
//! ```
//! [Further reading and details on the C API](https://man7.org/linux/man-pages/man7/mq_overview.7.html)

use crate::errno::Errno;
use crate::NixPath;
use crate::Result;

use crate::sys::stat::Mode;
use libc::{self, mqd_t, size_t};
use std::mem;
#[cfg(any(
    target_os = "linux",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
use std::os::unix::io::{
    AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd,
};

libc_bitflags! {
    /// Used with [`mq_open`].
    pub struct MQ_OFlag: libc::c_int {
        /// Open the message queue for receiving messages.
        O_RDONLY;
        /// Open the queue for sending messages.
        O_WRONLY;
        /// Open the queue for both receiving and sending messages
        O_RDWR;
        /// Create a message queue.
        O_CREAT;
        /// If set along with `O_CREAT`, `mq_open` will fail if the message
        /// queue name exists.
        O_EXCL;
        /// `mq_send` and `mq_receive` should fail with `EAGAIN` rather than
        /// wait for resources that are not currently available.
        O_NONBLOCK;
        /// Set the close-on-exec flag for the message queue descriptor.
        O_CLOEXEC;
    }
}

/// A message-queue attribute, optionally used with [`mq_setattr`] and
/// [`mq_getattr`] and optionally [`mq_open`],
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MqAttr {
    mq_attr: libc::mq_attr,
}

/// Identifies an open POSIX Message Queue
// A safer wrapper around libc::mqd_t, which is a pointer on some platforms
// Deliberately is not Clone to prevent use-after-close scenarios
#[repr(transparent)]
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct MqdT(mqd_t);

// x32 compatibility
// See https://sourceware.org/bugzilla/show_bug.cgi?id=21279
/// Size of a message queue attribute member
#[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
pub type mq_attr_member_t = i64;
/// Size of a message queue attribute member
#[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
pub type mq_attr_member_t = libc::c_long;

impl MqAttr {
    /// Create a new message queue attribute
    ///
    /// # Arguments
    ///
    /// - `mq_flags`:   Either `0` or `O_NONBLOCK`.
    /// - `mq_maxmsg`:  Maximum number of messages on the queue.
    /// - `mq_msgsize`: Maximum message size in bytes.
    /// - `mq_curmsgs`: Number of messages currently in the queue.
    pub fn new(
        mq_flags: mq_attr_member_t,
        mq_maxmsg: mq_attr_member_t,
        mq_msgsize: mq_attr_member_t,
        mq_curmsgs: mq_attr_member_t,
    ) -> MqAttr {
        let mut attr = mem::MaybeUninit::<libc::mq_attr>::uninit();
        unsafe {
            let p = attr.as_mut_ptr();
            (*p).mq_flags = mq_flags;
            (*p).mq_maxmsg = mq_maxmsg;
            (*p).mq_msgsize = mq_msgsize;
            (*p).mq_curmsgs = mq_curmsgs;
            MqAttr {
                mq_attr: attr.assume_init(),
            }
        }
    }

    /// The current flags, either `0` or `O_NONBLOCK`.
    pub const fn flags(&self) -> mq_attr_member_t {
        self.mq_attr.mq_flags
    }

    /// The max number of messages that can be held by the queue
    pub const fn maxmsg(&self) -> mq_attr_member_t {
        self.mq_attr.mq_maxmsg
    }

    /// The maximum size of each message (in bytes)
    pub const fn msgsize(&self) -> mq_attr_member_t {
        self.mq_attr.mq_msgsize
    }

    /// The number of messages currently held in the queue
    pub const fn curmsgs(&self) -> mq_attr_member_t {
        self.mq_attr.mq_curmsgs
    }
}

/// Open a message queue
///
/// See also [`mq_open(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_open.html)
// The mode.bits() cast is only lossless on some OSes
#[allow(clippy::cast_lossless)]
pub fn mq_open<P>(
    name: &P,
    oflag: MQ_OFlag,
    mode: Mode,
    attr: Option<&MqAttr>,
) -> Result<MqdT>
where
    P: ?Sized + NixPath,
{
    let res = name.with_nix_path(|cstr| match attr {
        Some(mq_attr) => unsafe {
            libc::mq_open(
                cstr.as_ptr(),
                oflag.bits(),
                mode.bits() as libc::c_int,
                &mq_attr.mq_attr as *const libc::mq_attr,
            )
        },
        None => unsafe { libc::mq_open(cstr.as_ptr(), oflag.bits()) },
    })?;

    Errno::result(res).map(MqdT)
}

/// Remove a message queue
///
/// See also [`mq_unlink(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_unlink.html)
pub fn mq_unlink<P>(name: &P) -> Result<()>
where
    P: ?Sized + NixPath,
{
    let res =
        name.with_nix_path(|cstr| unsafe { libc::mq_unlink(cstr.as_ptr()) })?;
    Errno::result(res).map(drop)
}

/// Close a message queue
///
/// See also [`mq_close(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_close.html)
pub fn mq_close(mqdes: MqdT) -> Result<()> {
    let res = unsafe { libc::mq_close(mqdes.0) };
    Errno::result(res).map(drop)
}

/// Receive a message from a message queue
///
/// See also [`mq_receive(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_receive.html)
pub fn mq_receive(
    mqdes: &MqdT,
    message: &mut [u8],
    msg_prio: &mut u32,
) -> Result<usize> {
    let len = message.len() as size_t;
    let res = unsafe {
        libc::mq_receive(
            mqdes.0,
            message.as_mut_ptr().cast(),
            len,
            msg_prio as *mut u32,
        )
    };
    Errno::result(res).map(|r| r as usize)
}

feature! {
    #![feature = "time"]
    use crate::sys::time::TimeSpec;
    /// Receive a message from a message queue with a timeout
    ///
    /// See also ['mq_timedreceive(2)'](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_receive.html)
    pub fn mq_timedreceive(
        mqdes: &MqdT,
        message: &mut [u8],
        msg_prio: &mut u32,
        abstime: &TimeSpec,
    ) -> Result<usize> {
        let len = message.len() as size_t;
        let res = unsafe {
            libc::mq_timedreceive(
                mqdes.0,
                message.as_mut_ptr().cast(),
                len,
                msg_prio as *mut u32,
                abstime.as_ref(),
            )
        };
        Errno::result(res).map(|r| r as usize)
    }
}

/// Send a message to a message queue
///
/// See also [`mq_send(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_send.html)
pub fn mq_send(mqdes: &MqdT, message: &[u8], msq_prio: u32) -> Result<()> {
    let res = unsafe {
        libc::mq_send(mqdes.0, message.as_ptr().cast(), message.len(), msq_prio)
    };
    Errno::result(res).map(drop)
}

/// Get message queue attributes
///
/// See also [`mq_getattr(2)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_getattr.html)
pub fn mq_getattr(mqd: &MqdT) -> Result<MqAttr> {
    let mut attr = mem::MaybeUninit::<libc::mq_attr>::uninit();
    let res = unsafe { libc::mq_getattr(mqd.0, attr.as_mut_ptr()) };
    Errno::result(res).map(|_| unsafe {
        MqAttr {
            mq_attr: attr.assume_init(),
        }
    })
}

/// Set the attributes of the message queue. Only `O_NONBLOCK` can be set,
/// everything else will be ignored. Returns the old attributes.
///
/// It is recommend to use the `mq_set_nonblock()` and `mq_remove_nonblock()`
/// convenience functions as they are easier to use.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mq_setattr.html)
pub fn mq_setattr(mqd: &MqdT, newattr: &MqAttr) -> Result<MqAttr> {
    let mut attr = mem::MaybeUninit::<libc::mq_attr>::uninit();
    let res = unsafe {
        libc::mq_setattr(
            mqd.0,
            &newattr.mq_attr as *const libc::mq_attr,
            attr.as_mut_ptr(),
        )
    };
    Errno::result(res).map(|_| unsafe {
        MqAttr {
            mq_attr: attr.assume_init(),
        }
    })
}

/// Convenience function.
/// Sets the `O_NONBLOCK` attribute for a given message queue descriptor
/// Returns the old attributes
#[allow(clippy::useless_conversion)] // Not useless on all OSes
pub fn mq_set_nonblock(mqd: &MqdT) -> Result<MqAttr> {
    let oldattr = mq_getattr(mqd)?;
    let newattr = MqAttr::new(
        mq_attr_member_t::from(MQ_OFlag::O_NONBLOCK.bits()),
        oldattr.mq_attr.mq_maxmsg,
        oldattr.mq_attr.mq_msgsize,
        oldattr.mq_attr.mq_curmsgs,
    );
    mq_setattr(mqd, &newattr)
}

/// Convenience function.
/// Removes `O_NONBLOCK` attribute for a given message queue descriptor
/// Returns the old attributes
pub fn mq_remove_nonblock(mqd: &MqdT) -> Result<MqAttr> {
    let oldattr = mq_getattr(mqd)?;
    let newattr = MqAttr::new(
        0,
        oldattr.mq_attr.mq_maxmsg,
        oldattr.mq_attr.mq_msgsize,
        oldattr.mq_attr.mq_curmsgs,
    );
    mq_setattr(mqd, &newattr)
}

#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
impl AsFd for MqdT {
    /// Borrow the underlying message queue descriptor.
    fn as_fd(&self) -> BorrowedFd {
        // SAFETY: [MqdT] will only contain a valid fd by construction.
        unsafe { BorrowedFd::borrow_raw(self.0) }
    }
}

#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
impl AsRawFd for MqdT {
    /// Return the underlying message queue descriptor.
    ///
    /// Returned descriptor is a "shallow copy" of the descriptor, so it refers
    ///  to the same underlying kernel object as `self`.
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
impl FromRawFd for MqdT {
    /// Construct an [MqdT] from [RawFd].
    ///
    /// # Safety
    /// The `fd` given must be a valid and open file descriptor for a message
    ///  queue.
    unsafe fn from_raw_fd(fd: RawFd) -> MqdT {
        MqdT(fd)
    }
}

#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
impl IntoRawFd for MqdT {
    /// Consume this [MqdT] and return a [RawFd].
    fn into_raw_fd(self) -> RawFd {
        self.0
    }
}
