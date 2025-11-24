//! `Timespec` and related types, which are used by multiple public API
//! modules.

#[cfg(not(fix_y2038))]
use crate::backend::c;

/// `struct timespec`
#[cfg(not(fix_y2038))]
pub type Timespec = c::timespec;

/// `struct timespec`
#[cfg(fix_y2038)]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Timespec {
    /// Seconds.
    pub tv_sec: Secs,

    /// Nanoseconds. Must be less than 1_000_000_000.
    pub tv_nsec: Nsecs,
}

/// A type for the `tv_sec` field of [`Timespec`].
#[cfg(not(fix_y2038))]
#[allow(deprecated)]
pub type Secs = c::time_t;

/// A type for the `tv_sec` field of [`Timespec`].
#[cfg(fix_y2038)]
pub type Secs = i64;

/// A type for the `tv_sec` field of [`Timespec`].
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
pub type Nsecs = c::c_long;

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
