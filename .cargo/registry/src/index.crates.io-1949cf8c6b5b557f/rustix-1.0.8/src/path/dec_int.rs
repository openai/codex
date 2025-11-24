//! Efficient decimal integer formatting.
//!
//! # Safety
//!
//! This uses `CStr::from_bytes_with_nul_unchecked` and
//! `str::from_utf8_unchecked`on the buffer that it filled itself.
#![allow(unsafe_code)]

use crate::backend::fd::{AsFd, AsRawFd as _};
use crate::ffi::CStr;
use core::fmt;
use core::hint::unreachable_unchecked;
use core::mem::{self, MaybeUninit};
use core::num::{NonZeroU8, NonZeroUsize};
#[cfg(all(feature = "std", unix))]
use std::os::unix::ffi::OsStrExt;
#[cfg(all(
    feature = "std",
    target_os = "wasi",
    any(not(target_env = "p2"), wasip2)
))]
use std::os::wasi::ffi::OsStrExt;
#[cfg(feature = "std")]
use {std::ffi::OsStr, std::path::Path};

/// Format an integer into a decimal `Path` component, without constructing a
/// temporary `PathBuf` or `String`.
///
/// This is used for opening paths such as `/proc/self/fd/<fd>` on Linux.
///
/// # Examples
///
/// ```
/// # #[cfg(any(feature = "fs", feature = "net"))]
/// use rustix::path::DecInt;
///
/// # #[cfg(any(feature = "fs", feature = "net"))]
/// assert_eq!(
///     format!("hello {}", DecInt::new(9876).as_ref().display()),
///     "hello 9876"
/// );
/// ```
#[derive(Clone)]
pub struct DecInt {
    buf: [MaybeUninit<u8>; BUF_LEN],
    len: NonZeroU8,
}

/// Enough to hold an {u,i}64 and NUL terminator.
const BUF_LEN: usize = U64_MAX_STR_LEN + 1;

/// Maximum length of a formatted [`u64`].
const U64_MAX_STR_LEN: usize = "18446744073709551615".len();

/// Maximum length of a formatted [`i64`].
#[allow(dead_code)]
const I64_MAX_STR_LEN: usize = "-9223372036854775808".len();

const _: () = assert!(U64_MAX_STR_LEN == I64_MAX_STR_LEN);

mod private {
    pub trait Sealed: Copy {
        type Unsigned: super::Integer;

        fn as_unsigned(self) -> (bool, Self::Unsigned);
        fn eq_zero(self) -> bool;
        fn div_mod_10(&mut self) -> u8;
    }

    macro_rules! impl_unsigned {
        ($($ty:ty)+) => { $(
            impl Sealed for $ty {
                type Unsigned = $ty;

                #[inline]
                fn as_unsigned(self) -> (bool, $ty) {
                    (false, self)
                }

                #[inline]
                fn eq_zero(self) -> bool {
                    self == 0
                }

                #[inline]
                fn div_mod_10(&mut self) -> u8 {
                    let result = (*self % 10) as u8;
                    *self /= 10;
                    result
                }
            }
        )+ }
    }

    macro_rules! impl_signed {
        ($($signed:ty : $unsigned:ty)+) => { $(
            impl Sealed for $signed {
                type Unsigned = $unsigned;

                #[inline]
                fn as_unsigned(self) -> (bool, $unsigned) {
                    if self >= 0 {
                        (false, self as $unsigned)
                    } else {
                        (true, !(self as $unsigned) + 1)
                    }
                }

                #[inline]
                fn eq_zero(self) -> bool {
                    unimplemented!()
                }

                #[inline]
                fn div_mod_10(&mut self) -> u8 {
                    unimplemented!()
                }
            }
        )+ }
    }

    impl_unsigned!(u8 u16 u32 u64);
    impl_signed!(i8:u8 i16:u16 i32:u32 i64:u64);

    #[cfg(any(
        target_pointer_width = "16",
        target_pointer_width = "32",
        target_pointer_width = "64"
    ))]
    const _: () = {
        impl_unsigned!(usize);
        impl_signed!(isize:usize);
    };
}

/// An integer that can be used by [`DecInt::new`].
pub trait Integer: private::Sealed {}

impl Integer for i8 {}
impl Integer for i16 {}
impl Integer for i32 {}
impl Integer for i64 {}
impl Integer for u8 {}
impl Integer for u16 {}
impl Integer for u32 {}
impl Integer for u64 {}

#[cfg(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
))]
const _: () = {
    impl Integer for isize {}
    impl Integer for usize {}
};

impl DecInt {
    /// Construct a new path component from an integer.
    pub fn new<Int: Integer>(i: Int) -> Self {
        use private::Sealed as _;

        let (is_neg, mut i) = i.as_unsigned();
        let mut len = 1;
        let mut buf = [MaybeUninit::uninit(); BUF_LEN];
        buf[BUF_LEN - 1] = MaybeUninit::new(b'\0');

        // We use `loop { …; if cond { break } }` instead of
        // `while !cond { … }` so the loop is entered at least once. This way
        // `0` does not need a special handling.
        loop {
            len += 1;
            if len > BUF_LEN {
                // SAFETY: A stringified `i64`/`u64` cannot be longer than
                // `U64_MAX_STR_LEN` bytes.
                unsafe { unreachable_unchecked() };
            }
            buf[BUF_LEN - len] = MaybeUninit::new(b'0' + i.div_mod_10());
            if i.eq_zero() {
                break;
            }
        }

        if is_neg {
            len += 1;
            if len > BUF_LEN {
                // SAFETY: A stringified `i64`/`u64` cannot be longer than
                // `U64_MAX_STR_LEN` bytes.
                unsafe { unreachable_unchecked() };
            }
            buf[BUF_LEN - len] = MaybeUninit::new(b'-');
        }

        Self {
            buf,
            len: NonZeroU8::new(len as u8).unwrap(),
        }
    }

    /// Construct a new path component from a file descriptor.
    #[inline]
    pub fn from_fd<Fd: AsFd>(fd: Fd) -> Self {
        Self::new(fd.as_fd().as_raw_fd())
    }

    /// Return the raw byte buffer as a `&str`.
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: `DecInt` always holds a formatted decimal number, so it's
        // always valid UTF-8.
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// Return the raw byte buffer as a `&CStr`.
    #[inline]
    pub fn as_c_str(&self) -> &CStr {
        let bytes_with_nul = self.as_bytes_with_nul();
        debug_assert!(CStr::from_bytes_with_nul(bytes_with_nul).is_ok());

        // SAFETY: `self.buf` holds a single decimal ASCII representation and
        // at least one extra NUL byte.
        unsafe { CStr::from_bytes_with_nul_unchecked(bytes_with_nul) }
    }

    /// Return the raw byte buffer including the NUL byte.
    #[inline]
    pub fn as_bytes_with_nul(&self) -> &[u8] {
        let len = NonZeroUsize::from(self.len).get();
        if len > BUF_LEN {
            // SAFETY: A stringified `i64`/`u64` cannot be longer than
            // `U64_MAX_STR_LEN` bytes.
            unsafe { unreachable_unchecked() };
        }
        let init = &self.buf[(self.buf.len() - len)..];
        // SAFETY: We're guaranteed to have initialized `len + 1` bytes.
        unsafe { mem::transmute::<&[MaybeUninit<u8>], &[u8]>(init) }
    }

    /// Return the raw byte buffer.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        let bytes = self.as_bytes_with_nul();
        &bytes[..bytes.len() - 1]
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl AsRef<Path> for DecInt {
    #[inline]
    fn as_ref(&self) -> &Path {
        let as_os_str: &OsStr = OsStrExt::from_bytes(self.as_bytes());
        Path::new(as_os_str)
    }
}

impl fmt::Debug for DecInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}
