use super::super::*;
use libc::*;

extern "C" {
    #[cfg(ossl300)]
    pub fn OSSL_PROVIDER_load(ctx: *mut OSSL_LIB_CTX, name: *const c_char) -> *mut OSSL_PROVIDER;
    #[cfg(ossl300)]
    pub fn OSSL_PROVIDER_try_load(
        ctx: *mut OSSL_LIB_CTX,
        name: *const c_char,
        retain_fallbacks: c_int,
    ) -> *mut OSSL_PROVIDER;
    #[cfg(ossl300)]
    pub fn OSSL_PROVIDER_unload(prov: *mut OSSL_PROVIDER) -> c_int;
    #[cfg(ossl300)]
    pub fn OSSL_PROVIDER_set_default_search_path(
        ctx: *mut OSSL_LIB_CTX,
        path: *const c_char,
    ) -> c_int;
}
