use super::super::*;
use libc::*;

cfg_if! {
    if #[cfg(ossl300)] {
        extern "C" {
            pub fn EVP_PKEY_CTX_set_rsa_padding(ctx: *mut EVP_PKEY_CTX, pad_mode: c_int) -> c_int;
            pub fn EVP_PKEY_CTX_get_rsa_padding(ctx: *mut EVP_PKEY_CTX, pad_mode: *mut c_int) -> c_int;

            pub fn EVP_PKEY_CTX_set_rsa_pss_saltlen(ctx: *mut EVP_PKEY_CTX, len: c_int) -> c_int;
            pub fn EVP_PKEY_CTX_set_rsa_mgf1_md(ctx: *mut EVP_PKEY_CTX, md: *const EVP_MD) -> c_int;
        }
    }
}

extern "C" {
    pub fn RSA_new() -> *mut RSA;
    pub fn RSA_size(k: *const RSA) -> c_int;

    #[cfg(any(ossl110, libressl273))]
    pub fn RSA_set0_key(r: *mut RSA, n: *mut BIGNUM, e: *mut BIGNUM, d: *mut BIGNUM) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    pub fn RSA_set0_factors(r: *mut RSA, p: *mut BIGNUM, q: *mut BIGNUM) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    pub fn RSA_set0_crt_params(
        r: *mut RSA,
        dmp1: *mut BIGNUM,
        dmq1: *mut BIGNUM,
        iqmp: *mut BIGNUM,
    ) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    pub fn RSA_get0_key(
        r: *const RSA,
        n: *mut *const BIGNUM,
        e: *mut *const BIGNUM,
        d: *mut *const BIGNUM,
    );
    #[cfg(any(ossl110, libressl273))]
    pub fn RSA_get0_factors(r: *const RSA, p: *mut *const BIGNUM, q: *mut *const BIGNUM);
    #[cfg(any(ossl110, libressl273))]
    pub fn RSA_get0_crt_params(
        r: *const RSA,
        dmp1: *mut *const BIGNUM,
        dmq1: *mut *const BIGNUM,
        iqmp: *mut *const BIGNUM,
    );

    #[cfg(not(ossl110))]
    pub fn RSA_generate_key(
        modsz: c_int,
        e: c_ulong,
        cb: Option<extern "C" fn(c_int, c_int, *mut c_void)>,
        cbarg: *mut c_void,
    ) -> *mut RSA;

    pub fn RSA_generate_key_ex(
        rsa: *mut RSA,
        bits: c_int,
        e: *mut BIGNUM,
        cb: *mut BN_GENCB,
    ) -> c_int;

    pub fn RSA_public_encrypt(
        flen: c_int,
        from: *const u8,
        to: *mut u8,
        k: *mut RSA,
        pad: c_int,
    ) -> c_int;
    pub fn RSA_private_encrypt(
        flen: c_int,
        from: *const u8,
        to: *mut u8,
        k: *mut RSA,
        pad: c_int,
    ) -> c_int;
    pub fn RSA_public_decrypt(
        flen: c_int,
        from: *const u8,
        to: *mut u8,
        k: *mut RSA,
        pad: c_int,
    ) -> c_int;
    pub fn RSA_private_decrypt(
        flen: c_int,
        from: *const u8,
        to: *mut u8,
        k: *mut RSA,
        pad: c_int,
    ) -> c_int;
    pub fn RSA_check_key(r: *const RSA) -> c_int;
    pub fn RSA_free(rsa: *mut RSA);
    pub fn RSA_up_ref(rsa: *mut RSA) -> c_int;

    pub fn i2d_RSAPublicKey(k: *const RSA, buf: *mut *mut u8) -> c_int;
    pub fn d2i_RSAPublicKey(k: *mut *mut RSA, buf: *mut *const u8, len: c_long) -> *mut RSA;
    pub fn i2d_RSAPrivateKey(k: *const RSA, buf: *mut *mut u8) -> c_int;
    pub fn d2i_RSAPrivateKey(k: *mut *mut RSA, buf: *mut *const u8, len: c_long) -> *mut RSA;

    pub fn RSA_sign(
        t: c_int,
        m: *const u8,
        mlen: c_uint,
        sig: *mut u8,
        siglen: *mut c_uint,
        k: *mut RSA,
    ) -> c_int;
    pub fn RSA_verify(
        t: c_int,
        m: *const u8,
        mlen: c_uint,
        sig: *const u8,
        siglen: c_uint,
        k: *mut RSA,
    ) -> c_int;

    pub fn RSA_padding_check_PKCS1_type_2(
        to: *mut c_uchar,
        tlen: c_int,
        f: *const c_uchar,
        fl: c_int,
        rsa_len: c_int,
    ) -> c_int;
}
