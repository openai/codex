use super::super::*;
use libc::*;

#[repr(C)]
pub struct ERR_STRING_DATA {
    pub error: c_ulong,
    pub string: *const c_char,
}

cfg_if! {
    if #[cfg(ossl300)] {
        extern "C" {
            pub fn ERR_new();
            pub fn ERR_set_debug(file: *const c_char, line: c_int, func: *const c_char);
            pub fn ERR_set_error(lib: c_int, reason: c_int, fmt: *const c_char, ...);
        }
    } else {
        extern "C" {
            pub fn ERR_put_error(lib: c_int, func: c_int, reason: c_int, file: *const c_char, line: c_int);
        }
    }
}

extern "C" {
    pub fn ERR_set_error_data(data: *mut c_char, flags: c_int);

    pub fn ERR_get_error() -> c_ulong;
    #[cfg(ossl300)]
    pub fn ERR_get_error_all(
        file: *mut *const c_char,
        line: *mut c_int,
        func: *mut *const c_char,
        data: *mut *const c_char,
        flags: *mut c_int,
    ) -> c_ulong;
    pub fn ERR_get_error_line_data(
        file: *mut *const c_char,
        line: *mut c_int,
        data: *mut *const c_char,
        flags: *mut c_int,
    ) -> c_ulong;
    pub fn ERR_peek_last_error() -> c_ulong;
    pub fn ERR_clear_error();
    pub fn ERR_lib_error_string(err: c_ulong) -> *const c_char;
    pub fn ERR_func_error_string(err: c_ulong) -> *const c_char;
    pub fn ERR_reason_error_string(err: c_ulong) -> *const c_char;
    #[cfg(ossl110)]
    pub fn ERR_load_strings(lib: c_int, str: *mut ERR_STRING_DATA) -> c_int;
    #[cfg(not(ossl110))]
    pub fn ERR_load_strings(lib: c_int, str: *mut ERR_STRING_DATA);
    #[cfg(not(ossl110))]
    pub fn ERR_load_crypto_strings();

    pub fn ERR_get_next_error_library() -> c_int;
}
