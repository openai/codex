use libc::*;
use std::ptr;

use super::*;

pub const OPENSSL_EC_NAMED_CURVE: c_int = 1;

#[cfg(ossl300)]
pub unsafe fn EVP_EC_gen(curve: *const c_char) -> *mut EVP_PKEY {
    EVP_PKEY_Q_keygen(
        ptr::null_mut(),
        ptr::null_mut(),
        "EC\0".as_ptr().cast(),
        curve,
    )
}
