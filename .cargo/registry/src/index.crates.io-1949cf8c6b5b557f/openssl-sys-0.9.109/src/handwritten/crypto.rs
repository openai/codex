use super::super::*;
use libc::*;

stack!(stack_st_void);

cfg_if! {
    if #[cfg(any(ossl110, libressl271))] {
        extern "C" {
            pub fn OpenSSL_version_num() -> c_ulong;
            pub fn OpenSSL_version(key: c_int) -> *const c_char;
        }
    } else {
        extern "C" {
            pub fn SSLeay() -> c_ulong;
            pub fn SSLeay_version(key: c_int) -> *const c_char;
        }
    }
}

extern "C" {
    #[cfg(any(ossl110, libressl))]
    pub fn CRYPTO_get_ex_new_index(
        class_index: c_int,
        argl: c_long,
        argp: *mut c_void,
        new_func: Option<CRYPTO_EX_new>,
        dup_func: Option<CRYPTO_EX_dup>,
        free_func: Option<CRYPTO_EX_free>,
    ) -> c_int;

    #[cfg(not(ossl110))]
    pub fn CRYPTO_num_locks() -> c_int;
}

#[allow(clashing_extern_declarations)]
extern "C" {
    #[cfg(not(ossl110))]
    #[link_name = "CRYPTO_set_locking_callback"]
    pub fn CRYPTO_set_locking_callback__fixed_rust(
        func: Option<unsafe extern "C" fn(mode: c_int, n: c_int, file: *const c_char, line: c_int)>,
    );

    #[cfg(not(ossl110))]
    #[link_name = "CRYPTO_set_id_callback"]
    pub fn CRYPTO_set_id_callback__fixed_rust(func: Option<unsafe extern "C" fn() -> c_ulong>);
}

extern "C" {
    #[cfg(not(ossl110))]
    pub fn CRYPTO_add_lock(
        pointer: *mut c_int,
        amount: c_int,
        type_: c_int,
        file: *const c_char,
        line: c_int,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(any(ossl110, libressl390))] {
        extern "C" {
            pub fn CRYPTO_malloc(num: size_t, file: *const c_char, line: c_int) -> *mut c_void;
            pub fn CRYPTO_free(buf: *mut c_void, file: *const c_char, line: c_int);
        }
    } else {
        extern "C" {
            pub fn CRYPTO_malloc(num: c_int, file: *const c_char, line: c_int) -> *mut c_void;
            pub fn CRYPTO_free(buf: *mut c_void);
        }
    }
}

extern "C" {
    #[cfg(all(ossl101, not(ossl300)))]
    pub fn FIPS_mode() -> c_int;
    #[cfg(all(ossl101, not(ossl300)))]
    pub fn FIPS_mode_set(onoff: c_int) -> c_int;

    pub fn CRYPTO_memcmp(a: *const c_void, b: *const c_void, len: size_t) -> c_int;

    #[cfg(ossl300)]
    pub fn OSSL_LIB_CTX_new() -> *mut OSSL_LIB_CTX;
    #[cfg(ossl300)]
    pub fn OSSL_LIB_CTX_free(libcts: *mut OSSL_LIB_CTX);
}
