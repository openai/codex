use super::super::*;
use libc::*;

cfg_if! {
    if #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))] {
        #[repr(C)]
        #[derive(Clone)]
        pub struct SHA_CTX {
            pub h0: SHA_LONG,
            pub h1: SHA_LONG,
            pub h2: SHA_LONG,
            pub h3: SHA_LONG,
            pub h4: SHA_LONG,
            pub Nl: SHA_LONG,
            pub Nh: SHA_LONG,
            pub data: [SHA_LONG; SHA_LBLOCK as usize],
            pub num: c_uint,
        }

        extern "C" {
            pub fn SHA1_Init(c: *mut SHA_CTX) -> c_int;
            pub fn SHA1_Update(c: *mut SHA_CTX, data: *const c_void, len: size_t) -> c_int;
            pub fn SHA1_Final(md: *mut c_uchar, c: *mut SHA_CTX) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(ossl300))] {
        extern "C" {
            pub fn SHA1(d: *const c_uchar, n: size_t, md: *mut c_uchar) -> *mut c_uchar;
        }
    }
}

cfg_if! {
    if #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))] {
        #[repr(C)]
        #[derive(Clone)]
        pub struct SHA256_CTX {
            pub h: [SHA_LONG; 8],
            pub Nl: SHA_LONG,
            pub Nh: SHA_LONG,
            pub data: [SHA_LONG; SHA_LBLOCK as usize],
            pub num: c_uint,
            pub md_len: c_uint,
        }

        extern "C" {
            pub fn SHA224_Init(c: *mut SHA256_CTX) -> c_int;
            pub fn SHA224_Update(c: *mut SHA256_CTX, data: *const c_void, len: size_t) -> c_int;
            pub fn SHA224_Final(md: *mut c_uchar, c: *mut SHA256_CTX) -> c_int;
            pub fn SHA256_Init(c: *mut SHA256_CTX) -> c_int;
            pub fn SHA256_Update(c: *mut SHA256_CTX, data: *const c_void, len: size_t) -> c_int;
            pub fn SHA256_Final(md: *mut c_uchar, c: *mut SHA256_CTX) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(ossl300))] {
        extern "C" {
            pub fn SHA224(d: *const c_uchar, n: size_t, md: *mut c_uchar) -> *mut c_uchar;
            pub fn SHA256(d: *const c_uchar, n: size_t, md: *mut c_uchar) -> *mut c_uchar;
        }
    }
}

cfg_if! {
    if #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))] {
        #[repr(C)]
        #[derive(Clone)]
        pub struct SHA512_CTX {
            pub h: [SHA_LONG64; 8],
            pub Nl: SHA_LONG64,
            pub Nh: SHA_LONG64,
            // this is a union but we don't want to require 1.19
            u: [SHA_LONG64; SHA_LBLOCK as usize],
            pub num: c_uint,
            pub md_len: c_uint,
        }

        extern "C" {
            pub fn SHA384_Init(c: *mut SHA512_CTX) -> c_int;
            pub fn SHA384_Update(c: *mut SHA512_CTX, data: *const c_void, len: size_t) -> c_int;
            pub fn SHA384_Final(md: *mut c_uchar, c: *mut SHA512_CTX) -> c_int;
            pub fn SHA512_Init(c: *mut SHA512_CTX) -> c_int;
            pub fn SHA512_Update(c: *mut SHA512_CTX, data: *const c_void, len: size_t) -> c_int;
            pub fn SHA512_Final(md: *mut c_uchar, c: *mut SHA512_CTX) -> c_int;
        }
    }
}

cfg_if! {
    if #[cfg(not(ossl300))] {
        extern "C" {
            pub fn SHA384(d: *const c_uchar, n: size_t, md: *mut c_uchar) -> *mut c_uchar;
            pub fn SHA512(d: *const c_uchar, n: size_t, md: *mut c_uchar) -> *mut c_uchar;
        }
    }
}
