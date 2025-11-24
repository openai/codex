use foreign_types::ForeignTypeRef;
use libc::{c_char, c_void};
use std::convert::AsRef;
use std::ffi::CStr;
use std::fmt;
use std::ops::Deref;
use std::str;

use crate::stack::Stackable;

foreign_type_and_impl_send_sync! {
    type CType = c_char;
    fn drop = free;

    pub struct OpensslString;
    pub struct OpensslStringRef;
}

impl fmt::Display for OpensslString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl fmt::Debug for OpensslString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl Stackable for OpensslString {
    type StackType = ffi::stack_st_OPENSSL_STRING;
}

impl AsRef<str> for OpensslString {
    fn as_ref(&self) -> &str {
        self
    }
}

impl AsRef<[u8]> for OpensslString {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Deref for OpensslStringRef {
    type Target = str;

    fn deref(&self) -> &str {
        unsafe {
            let slice = CStr::from_ptr(self.as_ptr()).to_bytes();
            str::from_utf8_unchecked(slice)
        }
    }
}

impl AsRef<str> for OpensslStringRef {
    fn as_ref(&self) -> &str {
        self
    }
}

impl AsRef<[u8]> for OpensslStringRef {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Display for OpensslStringRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl fmt::Debug for OpensslStringRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[inline]
#[cfg(not(any(boringssl, awslc)))]
unsafe fn free(buf: *mut c_char) {
    ffi::OPENSSL_free(buf as *mut c_void);
}

#[inline]
#[cfg(any(boringssl, awslc))]
unsafe fn free(buf: *mut c_char) {
    ffi::CRYPTO_free(
        buf as *mut c_void,
        concat!(file!(), "\0").as_ptr() as *const c_char,
        line!() as ::libc::c_int,
    );
}
