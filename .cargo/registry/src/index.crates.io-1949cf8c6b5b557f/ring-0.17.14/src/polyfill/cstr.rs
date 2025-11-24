// Copyright 2024 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#![cfg(all(
    target_vendor = "apple",
    any(
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos"
    )
))]

//! Work around lack of `core::ffi::CStr` prior to Rust 1.64, and the lack of
//! `const fn` support for `CStr` in later versions.

#![cfg(all(
    all(target_arch = "aarch64", target_endian = "little"),
    target_vendor = "apple"
))]

use core::mem::{align_of, size_of};

// TODO(MSRV 1.64): Use `core::ffi::c_char`.
use libc::c_char;

// TODO(MSRV 1.64): Replace with `&core::ffi::CStr`.
pub struct Ref(&'static [u8]);

impl Ref {
    #[inline(always)]
    pub fn as_ptr(&self) -> *const c_char {
        const _SAME_ALIGNMENT: () = assert!(align_of::<u8>() == align_of::<c_char>());
        const _SAME_SIZE: () = assert!(size_of::<u8>() == size_of::<c_char>());

        // It is safe to cast a `*const u8` to a `const c_char` as they are the
        // same size and alignment.
        self.0.as_ptr().cast()
    }

    // SAFETY: Same as `CStr::from_bytes_with_nul_unchecked`.
    const unsafe fn from_bytes_with_nul_unchecked(value: &'static [u8]) -> Self {
        Self(value)
    }
}

pub const fn unwrap_const_from_bytes_with_nul(value: &'static [u8]) -> Ref {
    // XXX: We cannot use `unwrap_const` since `Ref`/`CStr` is not `Copy`.
    match const_from_bytes_with_nul(value) {
        Some(r) => r,
        None => panic!("const_from_bytes_with_nul failed"),
    }
}

// TODO(MSRV 1.72): Replace with `CStr::from_bytes_with_nul`.
#[inline(always)]
const fn const_from_bytes_with_nul(value: &'static [u8]) -> Option<Ref> {
    const fn const_contains(mut value: &[u8], needle: &u8) -> bool {
        while let [head, tail @ ..] = value {
            if *head == *needle {
                return true;
            }
            value = tail;
        }
        false
    }

    // TODO(MSRV 1.69): Use `core::ffi::CStr::from_bytes_until_nul`
    match value {
        [before_nul @ .., 0] if !const_contains(before_nul, &0) => {
            // SAFETY:
            //   * `value` is nul-terminated according to the slice pattern.
            //   * `value` doesn't contain any interior null, by the guard.
            // TODO(MSRV 1.64): Use `CStr::from_bytes_with_nul_unchecked`
            Some(unsafe { Ref::from_bytes_with_nul_unchecked(value) })
        }
        _ => None,
    }
}

mod tests {
    use super::const_from_bytes_with_nul;

    // Bad.
    const _EMPTY_UNTERMINATED: () = assert!(const_from_bytes_with_nul(b"").is_none());
    const _EMPTY_DOUBLE_TERMINATED: () = assert!(const_from_bytes_with_nul(b"\0\0").is_none());
    const _DOUBLE_NUL: () = assert!(const_from_bytes_with_nul(b"\0\0").is_none());
    const _LEADINGL_NUL: () = assert!(const_from_bytes_with_nul(b"\0a\0").is_none());
    const _INTERNAL_NUL_UNTERMINATED: () = assert!(const_from_bytes_with_nul(b"\0a").is_none());

    // Good.
    const _EMPTY_TERMINATED: () = assert!(const_from_bytes_with_nul(b"\0").is_some());
    const _NONEMPTY: () = assert!(const_from_bytes_with_nul(b"asdf\0").is_some());
    const _1_CHAR: () = assert!(const_from_bytes_with_nul(b"a\0").is_some());
}
