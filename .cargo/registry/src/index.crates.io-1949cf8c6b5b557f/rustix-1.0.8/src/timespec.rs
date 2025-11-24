//! `Timespec` and related types, which are used by multiple public API
//! modules.

#![allow(dead_code)]

use core::num::TryFromIntError;
use core::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use core::time::Duration;

use crate::backend::c;
#[allow(unused)]
use crate::ffi;
#[cfg(not(fix_y2038))]
use core::ptr::null;

/// `struct timespec`—A quantity of time in seconds plus nanoseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Timespec {
    /// Seconds.
    pub tv_sec: Secs,

    /// Nanoseconds. Must be less than 1_000_000_000.
    ///
    /// When passed to [`rustix::fs::utimensat`], this field may instead be
    /// assigned the values [`UTIME_NOW`] or [`UTIME_OMIT`].
    ///
    /// [`UTIME_NOW`]: crate::fs::UTIME_NOW
    /// [`UTIME_OMIT`]: crate::fs::UTIME_OMIT
    /// [`rustix::fs::utimensat`]: crate::fs::utimensat
    pub tv_nsec: Nsecs,
}

/// A type for the `tv_sec` field of [`Timespec`].
pub type Secs = i64;

/// A type for the `tv_nsec` field of [`Timespec`].
#[cfg(any(
    fix_y2038,
    linux_raw,
    all(libc, target_arch = "x86_64", target_pointer_width = "32")
))]
pub type Nsecs = i64;

/// A type for the `tv_nsec` field of [`Timespec`].
#[cfg(all(
    not(fix_y2038),
    libc,
    not(all(target_arch = "x86_64", target_pointer_width = "32"))
))]
pub type Nsecs = ffi::c_long;

impl Timespec {
    /// Checked `Timespec` addition. Returns `None` if overflow occurred.
    ///
    /// # Panics
    ///
    /// If `0 <= .tv_nsec < 1_000_000_000` doesn't hold, this function may
    /// panic or return unexpected results.
    ///
    /// # Example
    ///
    /// ```
    /// use rustix::event::Timespec;
    ///
    /// assert_eq!(
    ///     Timespec {
    ///         tv_sec: 1,
    ///         tv_nsec: 2
    ///     }
    ///     .checked_add(Timespec {
    ///         tv_sec: 30,
    ///         tv_nsec: 40
    ///     }),
    ///     Some(Timespec {
    ///         tv_sec: 31,
    ///         tv_nsec: 42
    ///     })
    /// );
    /// assert_eq!(
    ///     Timespec {
    ///         tv_sec: 0,
    ///         tv_nsec: 999_999_999
    ///     }
    ///     .checked_add(Timespec {
    ///         tv_sec: 0,
    ///         tv_nsec: 2
    ///     }),
    ///     Some(Timespec {
    ///         tv_sec: 1,
    ///         tv_nsec: 1
    ///     })
    /// );
    /// assert_eq!(
    ///     Timespec {
    ///         tv_sec: i64::MAX,
    ///         tv_nsec: 999_999_999
    ///     }
    ///     .checked_add(Timespec {
    ///         tv_sec: 0,
    ///         tv_nsec: 1
    ///     }),
    ///     None
    /// );
    /// ```
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        if let Some(mut tv_sec) = self.tv_sec.checked_add(rhs.tv_sec) {
            let mut tv_nsec = self.tv_nsec + rhs.tv_nsec;
            if tv_nsec >= 1_000_000_000 {
                tv_nsec -= 1_000_000_000;
                if let Some(carried_sec) = tv_sec.checked_add(1) {
                    tv_sec = carried_sec;
                } else {
                    return None;
                }
            }
            Some(Self { tv_sec, tv_nsec })
        } else {
            None
        }
    }

    /// Checked `Timespec` subtraction. Returns `None` if overflow occurred.
    ///
    /// # Panics
    ///
    /// If `0 <= .tv_nsec < 1_000_000_000` doesn't hold, this function may
    /// panic or return unexpected results.
    ///
    /// # Example
    ///
    /// ```
    /// use rustix::event::Timespec;
    ///
    /// assert_eq!(
    ///     Timespec {
    ///         tv_sec: 31,
    ///         tv_nsec: 42
    ///     }
    ///     .checked_sub(Timespec {
    ///         tv_sec: 30,
    ///         tv_nsec: 40
    ///     }),
    ///     Some(Timespec {
    ///         tv_sec: 1,
    ///         tv_nsec: 2
    ///     })
    /// );
    /// assert_eq!(
    ///     Timespec {
    ///         tv_sec: 1,
    ///         tv_nsec: 1
    ///     }
    ///     .checked_sub(Timespec {
    ///         tv_sec: 0,
    ///         tv_nsec: 2
    ///     }),
    ///     Some(Timespec {
    ///         tv_sec: 0,
    ///         tv_nsec: 999_999_999
    ///     })
    /// );
    /// assert_eq!(
    ///     Timespec {
    ///         tv_sec: i64::MIN,
    ///         tv_nsec: 0
    ///     }
    ///     .checked_sub(Timespec {
    ///         tv_sec: 0,
    ///         tv_nsec: 1
    ///     }),
    ///     None
    /// );
    /// ```
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        if let Some(mut tv_sec) = self.tv_sec.checked_sub(rhs.tv_sec) {
            let mut tv_nsec = self.tv_nsec - rhs.tv_nsec;
            if tv_nsec < 0 {
                tv_nsec += 1_000_000_000;
                if let Some(borrowed_sec) = tv_sec.checked_sub(1) {
                    tv_sec = borrowed_sec;
                } else {
                    return None;
                }
            }
            Some(Self { tv_sec, tv_nsec })
        } else {
            None
        }
    }

    /// Convert from `Timespec` to `c::c_int` milliseconds, rounded up.
    pub(crate) fn as_c_int_millis(&self) -> Option<c::c_int> {
        let secs = self.tv_sec;
        if secs < 0 {
            return None;
        }
        secs.checked_mul(1000)
            .and_then(|millis| {
                // Add the nanoseconds, converted to milliseconds, rounding up.
                // With Rust 1.73.0 this can use `div_ceil`.
                millis.checked_add((i64::from(self.tv_nsec) + 999_999) / 1_000_000)
            })
            .and_then(|millis| c::c_int::try_from(millis).ok())
    }
}

impl TryFrom<Timespec> for Duration {
    type Error = TryFromIntError;

    fn try_from(ts: Timespec) -> Result<Self, Self::Error> {
        Ok(Self::new(ts.tv_sec.try_into()?, ts.tv_nsec as _))
    }
}

impl TryFrom<Duration> for Timespec {
    type Error = TryFromIntError;

    fn try_from(dur: Duration) -> Result<Self, Self::Error> {
        Ok(Self {
            tv_sec: dur.as_secs().try_into()?,
            tv_nsec: dur.subsec_nanos() as _,
        })
    }
}

impl Add for Timespec {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.checked_add(rhs)
            .expect("overflow when adding timespecs")
    }
}

impl AddAssign for Timespec {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Timespec {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self.checked_sub(rhs)
            .expect("overflow when subtracting timespecs")
    }
}

impl SubAssign for Timespec {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for Timespec {
    type Output = Self;

    fn neg(self) -> Self {
        Self::default() - self
    }
}

/// On 32-bit glibc platforms, `timespec` has anonymous padding fields, which
/// Rust doesn't support yet (see `unnamed_fields`), so we define our own
/// struct with explicit padding, with bidirectional `From` impls.
#[cfg(fix_y2038)]
#[repr(C)]
#[derive(Debug, Clone)]
pub(crate) struct LibcTimespec {
    pub(crate) tv_sec: Secs,

    #[cfg(target_endian = "big")]
    padding: core::mem::MaybeUninit<u32>,

    pub(crate) tv_nsec: i32,

    #[cfg(target_endian = "little")]
    padding: core::mem::MaybeUninit<u32>,
}

#[cfg(fix_y2038)]
impl From<LibcTimespec> for Timespec {
    #[inline]
    fn from(t: LibcTimespec) -> Self {
        Self {
            tv_sec: t.tv_sec,
            tv_nsec: t.tv_nsec as _,
        }
    }
}

#[cfg(fix_y2038)]
impl From<Timespec> for LibcTimespec {
    #[inline]
    fn from(t: Timespec) -> Self {
        Self {
            tv_sec: t.tv_sec,
            tv_nsec: t.tv_nsec as _,
            padding: core::mem::MaybeUninit::uninit(),
        }
    }
}

#[cfg(not(fix_y2038))]
pub(crate) fn as_libc_timespec_ptr(timespec: &Timespec) -> *const c::timespec {
    #[cfg(test)]
    {
        assert_eq_size!(Timespec, c::timespec);
    }
    crate::utils::as_ptr(timespec).cast::<c::timespec>()
}

#[cfg(not(fix_y2038))]
pub(crate) fn as_libc_timespec_mut_ptr(
    timespec: &mut core::mem::MaybeUninit<Timespec>,
) -> *mut c::timespec {
    #[cfg(test)]
    {
        assert_eq_size!(Timespec, c::timespec);
    }
    timespec.as_mut_ptr().cast::<c::timespec>()
}

#[cfg(not(fix_y2038))]
pub(crate) fn option_as_libc_timespec_ptr(timespec: Option<&Timespec>) -> *const c::timespec {
    match timespec {
        None => null(),
        Some(timespec) => as_libc_timespec_ptr(timespec),
    }
}

/// As described [here], Apple platforms may return a negative nanoseconds
/// value in some cases; adjust it so that nanoseconds is always in
/// `0..1_000_000_000`.
///
/// [here]: https://github.com/rust-lang/rust/issues/108277#issuecomment-1787057158
#[cfg(apple)]
#[inline]
pub(crate) fn fix_negative_nsecs(
    mut secs: c::time_t,
    mut nsecs: c::c_long,
) -> (c::time_t, c::c_long) {
    #[cold]
    fn adjust(secs: &mut c::time_t, nsecs: c::c_long) -> c::c_long {
        assert!(nsecs >= -1_000_000_000);
        assert!(*secs < 0);
        assert!(*secs > c::time_t::MIN);
        *secs -= 1;
        nsecs + 1_000_000_000
    }

    if nsecs < 0 {
        nsecs = adjust(&mut secs, nsecs);
    }
    (secs, nsecs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(apple)]
    #[test]
    fn test_negative_timestamps() {
        let mut secs = -59;
        let mut nsecs = -900_000_000;
        (secs, nsecs) = fix_negative_nsecs(secs, nsecs);
        assert_eq!(secs, -60);
        assert_eq!(nsecs, 100_000_000);
        (secs, nsecs) = fix_negative_nsecs(secs, nsecs);
        assert_eq!(secs, -60);
        assert_eq!(nsecs, 100_000_000);
    }

    #[test]
    fn test_sizes() {
        assert_eq_size!(Secs, u64);
        const_assert!(core::mem::size_of::<Timespec>() >= core::mem::size_of::<(u64, u32)>());
        const_assert!(core::mem::size_of::<Nsecs>() >= 4);

        let mut t = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        // `tv_nsec` needs to be able to hold nanoseconds up to a second.
        t.tv_nsec = 999_999_999_u32 as _;
        assert_eq!(t.tv_nsec as u64, 999_999_999_u64);

        // `tv_sec` needs to be able to hold more than 32-bits of seconds.
        t.tv_sec = 0x1_0000_0000_u64 as _;
        assert_eq!(t.tv_sec as u64, 0x1_0000_0000_u64);
    }

    // Test that our workarounds are needed.
    #[cfg(fix_y2038)]
    #[test]
    #[allow(deprecated)]
    fn test_fix_y2038() {
        assert_eq_size!(libc::time_t, u32);
    }

    // Test that our workarounds are not needed.
    #[cfg(not(fix_y2038))]
    #[test]
    fn timespec_layouts() {
        use crate::backend::c;
        check_renamed_struct!(Timespec, timespec, tv_sec, tv_nsec);
    }

    // Test that `Timespec` matches Linux's `__kernel_timespec`.
    #[cfg(linux_kernel)]
    #[test]
    fn test_against_kernel_timespec() {
        assert_eq_size!(Timespec, linux_raw_sys::general::__kernel_timespec);
        assert_eq_align!(Timespec, linux_raw_sys::general::__kernel_timespec);
        assert_eq!(
            memoffset::span_of!(Timespec, tv_sec),
            memoffset::span_of!(linux_raw_sys::general::__kernel_timespec, tv_sec)
        );
        assert_eq!(
            memoffset::span_of!(Timespec, tv_nsec),
            memoffset::span_of!(linux_raw_sys::general::__kernel_timespec, tv_nsec)
        );
    }
}
