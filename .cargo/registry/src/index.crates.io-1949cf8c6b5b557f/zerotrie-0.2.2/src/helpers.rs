// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

pub(crate) trait MaybeSplitAt<T> {
    /// Like slice::split_at but debug-panics and returns an empty second slice
    /// if the index is out of range.
    fn debug_split_at(&self, mid: usize) -> (&Self, &Self);
}

impl<T> MaybeSplitAt<T> for [T] {
    #[inline]
    fn debug_split_at(&self, mid: usize) -> (&Self, &Self) {
        self.split_at_checked(mid).unwrap_or_else(|| {
            debug_assert!(false, "debug_split_at: {mid} expected to be in range");
            (self, &[])
        })
    }
}

pub(crate) trait DebugUnwrapOr<T> {
    /// Unwraps the option or panics in debug mode, returning the `gigo_value`
    fn debug_unwrap_or(self, gigo_value: T) -> T;
}

impl<T> DebugUnwrapOr<T> for Option<T> {
    #[inline]
    fn debug_unwrap_or(self, gigo_value: T) -> T {
        match self {
            Some(x) => x,
            None => {
                debug_assert!(false, "debug_unwrap_or called on a None value");
                gigo_value
            }
        }
    }
}

macro_rules! debug_unwrap {
    ($expr:expr, return $retval:expr, $($arg:tt)+) => {
        match $expr {
            Some(x) => x,
            None => {
                debug_assert!(false, $($arg)*);
                return $retval;
            }
        }
    };
    ($expr:expr, return $retval:expr) => {
        debug_unwrap!($expr, return $retval, "invalid trie")
    };
    ($expr:expr, break, $($arg:tt)+) => {
        match $expr {
            Some(x) => x,
            None => {
                debug_assert!(false, $($arg)*);
                break;
            }
        }
    };
    ($expr:expr, break) => {
        debug_unwrap!($expr, break, "invalid trie")
    };
    ($expr:expr, $($arg:tt)+) => {
        debug_unwrap!($expr, return (), $($arg)*)
    };
    ($expr:expr) => {
        debug_unwrap!($expr, return ())
    };
}

pub(crate) use debug_unwrap;

/// The maximum number of base-10 digits required for rendering a usize.
/// Note: 24/10 is an approximation of 8*log10(2)
pub(crate) const MAX_USIZE_LEN_AS_DIGITS: usize = core::mem::size_of::<usize>() * 24 / 10 + 1;

/// Formats a usize as a string of length N, padded with spaces,
/// with the given prefix.
///
/// If the string is too short, the function may panic. To prevent
/// this, N should be MAX_USIZE_LEN_AS_DIGITS larger than M.
pub(crate) const fn const_fmt_int<const M: usize, const N: usize>(
    prefix: [u8; M],
    value: usize,
) -> [u8; N] {
    let mut output = [b' '; N];
    let mut i = 0;
    while i < M {
        output[i] = prefix[i];
        i += 1;
    }
    let mut int_only = [b' '; MAX_USIZE_LEN_AS_DIGITS];
    let mut value = value;
    let mut i = MAX_USIZE_LEN_AS_DIGITS - 1;
    loop {
        let x = (value % 10) as u8;
        int_only[i] = x + b'0';
        value /= 10;
        if value == 0 {
            break;
        }
        i -= 1;
    }
    let mut j = M;
    while i < MAX_USIZE_LEN_AS_DIGITS {
        output[j] = int_only[i];
        j += 1;
        i += 1;
    }
    output
}

#[test]
fn test_const_fmt_int() {
    assert_eq!(*b"123", const_fmt_int::<0, 3>(*b"", 123));
    assert_eq!(*b"123   ", const_fmt_int::<0, 6>(*b"", 123));
    assert_eq!(*b"abc123", const_fmt_int::<3, 6>(*b"abc", 123));
}
