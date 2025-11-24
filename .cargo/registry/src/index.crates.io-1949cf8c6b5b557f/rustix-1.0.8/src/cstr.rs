/// A macro for [`CStr`] literals.
///
/// This can make passing string literals to rustix APIs more efficient, since
/// most underlying system calls with string arguments expect NUL-terminated
/// strings, and passing strings to rustix as `CStr`s means that rustix doesn't
/// need to copy them into a separate buffer to NUL-terminate them.
///
/// In Rust ≥ 1.77, users can use [C-string literals] instead of this macro.
///
/// [`CStr`]: crate::ffi::CStr
/// [C-string literals]: https://blog.rust-lang.org/2024/03/21/Rust-1.77.0.html#c-string-literals
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "fs")]
/// # fn main() -> rustix::io::Result<()> {
/// use rustix::cstr;
/// use rustix::fs::{statat, AtFlags, CWD};
///
/// let metadata = statat(CWD, cstr!("Cargo.toml"), AtFlags::empty())?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "fs"))]
/// # fn main() {}
/// ```
#[allow(unused_macros)]
#[macro_export]
macro_rules! cstr {
    ($str:literal) => {{
        // Check for NUL manually, to ensure safety.
        //
        // In release builds, with strings that don't contain NULs, this
        // constant-folds away.
        //
        // We don't use std's `CStr::from_bytes_with_nul`; as of this writing,
        // that function isn't defined as `#[inline]` in std and doesn't
        // constant-fold away.
        assert!(
            !$str.bytes().any(|b| b == b'\0'),
            "cstr argument contains embedded NUL bytes",
        );

        #[allow(unsafe_code, unused_unsafe)]
        {
            // Now that we know the string doesn't have embedded NULs, we can
            // call `from_bytes_with_nul_unchecked`, which as of this writing
            // is defined as `#[inline]` and completely optimizes away.
            //
            // SAFETY: We have manually checked that the string does not
            // contain embedded NULs above, and we append or own NUL terminator
            // here.
            unsafe {
                $crate::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes())
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_cstr() {
        use crate::ffi::CString;
        use alloc::borrow::ToOwned as _;
        assert_eq!(cstr!(""), &*CString::new("").unwrap());
        assert_eq!(cstr!("").to_owned(), CString::new("").unwrap());
        assert_eq!(cstr!("hello"), &*CString::new("hello").unwrap());
        assert_eq!(cstr!("hello").to_owned(), CString::new("hello").unwrap());
    }

    #[test]
    #[should_panic]
    fn test_invalid_cstr() {
        let _ = cstr!("hello\0world");
    }

    #[test]
    #[should_panic]
    fn test_invalid_empty_cstr() {
        let _ = cstr!("\0");
    }
}
