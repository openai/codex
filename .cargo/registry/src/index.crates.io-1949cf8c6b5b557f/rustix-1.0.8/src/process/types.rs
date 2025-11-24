//! Types for use with [`rustix::process`] functions.
//!
//! [`rustix::process`]: crate::process

#![allow(unsafe_code)]

use crate::backend::c;
use crate::pid::Pid;
use core::mem::transmute;

/// File lock data structure used in [`fcntl_getlk`].
///
/// [`fcntl_getlk`]: crate::process::fcntl_getlk()
#[cfg(not(target_os = "horizon"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flock {
    /// Starting offset for lock
    pub start: u64,
    /// Number of bytes to lock
    pub length: u64,
    /// PID of process blocking our lock. If set to `None`, it refers to the
    /// current process
    pub pid: Option<Pid>,
    /// Type of lock
    pub typ: FlockType,
    /// Offset type of lock
    pub offset_type: FlockOffsetType,
}

#[cfg(not(target_os = "horizon"))]
impl Flock {
    pub(crate) const unsafe fn from_raw_unchecked(raw_fl: c::flock) -> Self {
        Self {
            start: raw_fl.l_start as _,
            length: raw_fl.l_len as _,
            pid: Pid::from_raw(raw_fl.l_pid),
            typ: transmute::<i16, FlockType>(raw_fl.l_type),
            offset_type: transmute::<i16, FlockOffsetType>(raw_fl.l_whence),
        }
    }

    pub(crate) fn as_raw(&self) -> c::flock {
        let mut f: c::flock = unsafe { core::mem::zeroed() };
        f.l_start = self.start as _;
        f.l_len = self.length as _;
        f.l_pid = Pid::as_raw(self.pid);
        f.l_type = self.typ as _;
        f.l_whence = self.offset_type as _;
        f
    }
}

#[cfg(not(target_os = "horizon"))]
impl From<FlockType> for Flock {
    fn from(value: FlockType) -> Self {
        Self {
            start: 0,
            length: 0,
            pid: None,
            typ: value,
            offset_type: FlockOffsetType::Set,
        }
    }
}

/// `F_*LCK` constants for use with [`fcntl_getlk`].
///
/// [`fcntl_getlk`]: crate::process::fcntl_getlk()
#[cfg(not(target_os = "horizon"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i16)]
pub enum FlockType {
    /// `F_RDLCK`
    ReadLock = c::F_RDLCK as _,
    /// `F_WRLCK`
    WriteLock = c::F_WRLCK as _,
    /// `F_UNLCK`
    Unlocked = c::F_UNLCK as _,
}

/// `F_SEEK*` constants for use with [`fcntl_getlk`].
///
/// [`fcntl_getlk`]: crate::process::fcntl_getlk()
#[cfg(not(target_os = "horizon"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i16)]
pub enum FlockOffsetType {
    /// `F_SEEK_SET`
    Set = c::SEEK_SET as _,
    /// `F_SEEK_CUR`
    Current = c::SEEK_CUR as _,
    /// `F_SEEK_END`
    End = c::SEEK_END as _,
}
