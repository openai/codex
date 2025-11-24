use libc::*;

use super::super::*;

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum DSA_SIG {}
    } else {
      #[repr(C)]
      pub struct DSA_SIG {
          pub r: *mut BIGNUM,
          pub s: *mut BIGNUM,
      }
    }
}

extern "C" {
    pub fn DSA_new() -> *mut DSA;
    pub fn DSA_free(dsa: *mut DSA);
    pub fn DSA_up_ref(dsa: *mut DSA) -> c_int;
    pub fn DSA_size(dsa: *const DSA) -> c_int;
    pub fn DSA_sign(
        dummy: c_int,
        dgst: *const c_uchar,
        len: c_int,
        sigret: *mut c_uchar,
        siglen: *mut c_uint,
        dsa: *mut DSA,
    ) -> c_int;
    pub fn DSA_verify(
        dummy: c_int,
        dgst: *const c_uchar,
        len: c_int,
        sigbuf: *const c_uchar,
        siglen: c_int,
        dsa: *mut DSA,
    ) -> c_int;

    pub fn d2i_DSAPublicKey(a: *mut *mut DSA, pp: *mut *const c_uchar, length: c_long) -> *mut DSA;
    pub fn d2i_DSAPrivateKey(a: *mut *mut DSA, pp: *mut *const c_uchar, length: c_long)
        -> *mut DSA;

    pub fn DSA_generate_parameters_ex(
        dsa: *mut DSA,
        bits: c_int,
        seed: *const c_uchar,
        seed_len: c_int,
        counter_ref: *mut c_int,
        h_ret: *mut c_ulong,
        cb: *mut BN_GENCB,
    ) -> c_int;

    pub fn DSA_generate_key(dsa: *mut DSA) -> c_int;
    pub fn i2d_DSAPublicKey(a: *const DSA, pp: *mut *mut c_uchar) -> c_int;
    pub fn i2d_DSAPrivateKey(a: *const DSA, pp: *mut *mut c_uchar) -> c_int;

    #[cfg(any(ossl110, libressl273))]
    pub fn DSA_get0_pqg(
        d: *const DSA,
        p: *mut *const BIGNUM,
        q: *mut *const BIGNUM,
        q: *mut *const BIGNUM,
    );
    #[cfg(any(ossl110, libressl273))]
    pub fn DSA_set0_pqg(d: *mut DSA, p: *mut BIGNUM, q: *mut BIGNUM, q: *mut BIGNUM) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    pub fn DSA_get0_key(d: *const DSA, pub_key: *mut *const BIGNUM, priv_key: *mut *const BIGNUM);
    #[cfg(any(ossl110, libressl273))]
    pub fn DSA_set0_key(d: *mut DSA, pub_key: *mut BIGNUM, priv_key: *mut BIGNUM) -> c_int;
    pub fn d2i_DSA_SIG(
        sig: *mut *mut DSA_SIG,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut DSA_SIG;
    pub fn i2d_DSA_SIG(a: *const DSA_SIG, pp: *mut *mut c_uchar) -> c_int;

    pub fn DSA_SIG_new() -> *mut DSA_SIG;
    pub fn DSA_SIG_free(sig: *mut DSA_SIG);

    #[cfg(any(ossl110, libressl273))]
    pub fn DSA_SIG_get0(sig: *const DSA_SIG, pr: *mut *const BIGNUM, ps: *mut *const BIGNUM);

    #[cfg(any(ossl110, libressl273))]
    pub fn DSA_SIG_set0(sig: *mut DSA_SIG, pr: *mut BIGNUM, ps: *mut BIGNUM) -> c_int;
}
