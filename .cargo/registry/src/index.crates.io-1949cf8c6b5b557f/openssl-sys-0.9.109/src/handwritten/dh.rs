use super::super::*;

extern "C" {
    pub fn DH_new() -> *mut DH;
    pub fn DH_free(dh: *mut DH);
    pub fn DH_check(dh: *const DH, codes: *mut c_int) -> c_int;

    #[cfg(not(libressl382))]
    pub fn DH_generate_parameters(
        prime_len: c_int,
        generator: c_int,
        callback: Option<extern "C" fn(c_int, c_int, *mut c_void)>,
        cb_arg: *mut c_void,
    ) -> *mut DH;

    pub fn DH_generate_parameters_ex(
        dh: *mut DH,
        prime_len: c_int,
        generator: c_int,
        cb: *mut BN_GENCB,
    ) -> c_int;

    pub fn DH_generate_key(dh: *mut DH) -> c_int;
    pub fn DH_compute_key(key: *mut c_uchar, pub_key: *const BIGNUM, dh: *mut DH) -> c_int;
    pub fn DH_size(dh: *const DH) -> c_int;

    pub fn d2i_DHparams(k: *mut *mut DH, pp: *mut *const c_uchar, length: c_long) -> *mut DH;
    pub fn i2d_DHparams(dh: *const DH, pp: *mut *mut c_uchar) -> c_int;

    #[cfg(ossl102)]
    pub fn DH_get_1024_160() -> *mut DH;
    #[cfg(ossl102)]
    pub fn DH_get_2048_224() -> *mut DH;
    #[cfg(ossl102)]
    pub fn DH_get_2048_256() -> *mut DH;

    #[cfg(any(ossl110, libressl270))]
    pub fn DH_set0_pqg(dh: *mut DH, p: *mut BIGNUM, q: *mut BIGNUM, g: *mut BIGNUM) -> c_int;
    #[cfg(any(ossl110, libressl270))]
    pub fn DH_get0_pqg(
        dh: *const DH,
        p: *mut *const BIGNUM,
        q: *mut *const BIGNUM,
        g: *mut *const BIGNUM,
    );

    #[cfg(any(ossl110, libressl270))]
    pub fn DH_set0_key(dh: *mut DH, pub_key: *mut BIGNUM, priv_key: *mut BIGNUM) -> c_int;

    #[cfg(any(ossl110, libressl270))]
    pub fn DH_get0_key(dh: *const DH, pub_key: *mut *const BIGNUM, priv_key: *mut *const BIGNUM);
}
