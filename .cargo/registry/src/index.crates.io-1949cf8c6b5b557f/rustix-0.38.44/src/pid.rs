//! The `Pid` type.

#![allow(unsafe_code)]

use crate::backend::c;
use core::num::NonZeroI32;

/// A process identifier as a raw integer.
pub type RawPid = c::pid_t;

/// `pid_t`—A non-zero Unix process ID.
///
/// This is a pid, and not a pidfd. It is not a file descriptor, and the
/// process it refers to could disappear at any time and be replaced by
/// another, unrelated, process.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct Pid(NonZeroI32);

impl Pid {
    /// A `Pid` corresponding to the init process (pid 1).
    pub const INIT: Self = Self(match NonZeroI32::new(1) {
        Some(n) => n,
        None => panic!("unreachable"),
    });

    /// Converts a `RawPid` into a `Pid`.
    ///
    /// Returns `Some` for positive `RawPid`s. Otherwise, returns `None`.
    ///
    /// This is safe because a `Pid` is a number without any guarantees for the
    /// kernel. Non-child `Pid`s are always racy for any syscalls, but can only
    /// cause logic errors. If you want race-free access to or control of
    /// non-child processes, please consider other mechanisms like [pidfd] on
    /// Linux.
    ///
    /// [pidfd]: https://man7.org/linux/man-pages/man2/pidfd_open.2.html
    #[inline]
    pub const fn from_raw(raw: RawPid) -> Option<Self> {
        if raw > 0 {
            // SAFETY: We just checked that `raw > 0`.
            unsafe { Some(Self::from_raw_unchecked(raw)) }
        } else {
            None
        }
    }

    /// Converts a known positive `RawPid` into a `Pid`.
    ///
    /// # Safety
    ///
    /// The caller must guarantee `raw` is positive.
    #[inline]
    pub const unsafe fn from_raw_unchecked(raw: RawPid) -> Self {
        debug_assert!(raw > 0);
        Self(NonZeroI32::new_unchecked(raw))
    }

    /// Creates a `Pid` holding the ID of the given child process.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_child(child: &std::process::Child) -> Self {
        let id = child.id();
        // SAFETY: We know the returned ID is valid because it came directly
        // from an OS API.
        unsafe { Self::from_raw_unchecked(id as i32) }
    }

    /// Converts a `Pid` into a `NonZeroI32`.
    #[inline]
    pub const fn as_raw_nonzero(self) -> NonZeroI32 {
        self.0
    }

    /// Converts an `Option<Pid>` into a `RawPid`.
    #[inline]
    pub fn as_raw(pid: Option<Self>) -> RawPid {
        pid.map_or(0, |pid| pid.0.get())
    }

    /// Test whether this pid represents the init process (pid 1).
    #[inline]
    pub const fn is_init(self) -> bool {
        self.0.get() == Self::INIT.0.get()
    }
}

#[test]
fn test_sizes() {
    use core::mem::transmute;

    assert_eq_size!(RawPid, NonZeroI32);
    assert_eq_size!(RawPid, Pid);
    assert_eq_size!(RawPid, Option<Pid>);

    // Rustix doesn't depend on `Option<Pid>` matching the ABI of a raw integer
    // for correctness, but it should work nonetheless.
    const_assert_eq!(0 as RawPid, unsafe {
        transmute::<Option<Pid>, RawPid>(None)
    });
    const_assert_eq!(4567 as RawPid, unsafe {
        transmute::<Option<Pid>, RawPid>(Some(Pid::from_raw_unchecked(4567)))
    });
}
