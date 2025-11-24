use super::super::*;
use libc::*;

extern "C" {
    pub fn OSSL_set_max_threads(ctx: *mut OSSL_LIB_CTX, max_threads: u64) -> c_int;
    pub fn OSSL_get_max_threads(ctx: *mut OSSL_LIB_CTX) -> u64;
}
