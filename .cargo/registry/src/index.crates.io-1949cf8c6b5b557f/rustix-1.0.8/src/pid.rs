//! The `Pid` type.

#![allow(unsafe_code)]

use core::num::NonZeroI32;

/// A process identifier as a raw integer.
pub type RawPid = i32;

/// `pid_t`—A non-zero Unix process ID.
///
/// This is a pid, and not a pidfd. It is not a file descriptor, and the
/// process it refers to could disappear at any time and be replaced by
/// another, unrelated, process.
///
/// On Linux, `Pid` values are also used to identify threads.
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
    /// Returns `Some` for positive values, and `None` for zero values.
    ///
    /// This is safe because a `Pid` is a number without any guarantees for the
    /// kernel. Non-child `Pid`s are always racy for any syscalls, but can only
    /// cause logic errors. If you want race-free access to or control of
    /// non-child processes, please consider other mechanisms like [pidfd] on
    /// Linux.
    ///
    /// Passing a negative number doesn't invoke undefined behavior, but it
    /// may cause unexpected behavior.
    ///
    /// [pidfd]: https://man7.org/linux/man-pages/man2/pidfd_open.2.html
    #[inline]
    pub const fn from_raw(raw: RawPid) -> Option<Self> {
        debug_assert!(raw >= 0);
        match NonZeroI32::new(raw) {
            Some(non_zero) => Some(Self(non_zero)),
            None => None,
        }
    }

    /// Converts a known positive `RawPid` into a `Pid`.
    ///
    /// Passing a negative number doesn't invoke undefined behavior, but it
    /// may cause unexpected behavior.
    ///
    /// # Safety
    ///
    /// The caller must guarantee `raw` is non-zero.
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
    pub const fn as_raw(pid: Option<Self>) -> RawPid {
        match pid {
            Some(pid) => pid.0.get(),
            None => 0,
        }
    }

    /// Test whether this pid represents the init process ([`Pid::INIT`]).
    #[inline]
    pub const fn is_init(self) -> bool {
        self.0.get() == Self::INIT.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_ctors() {
        use std::num::NonZeroI32;
        assert!(Pid::from_raw(0).is_none());
        assert_eq!(
            Pid::from_raw(77).unwrap().as_raw_nonzero(),
            NonZeroI32::new(77).unwrap()
        );
        assert_eq!(Pid::as_raw(Pid::from_raw(77)), 77);
    }

    #[test]
    fn test_specials() {
        assert!(Pid::from_raw(1).unwrap().is_init());
        assert_eq!(Pid::from_raw(1).unwrap(), Pid::INIT);
    }
}
