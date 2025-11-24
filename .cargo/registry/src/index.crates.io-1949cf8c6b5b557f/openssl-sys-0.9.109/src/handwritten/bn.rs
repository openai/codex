use super::super::*;
use libc::*;

extern "C" {
    pub fn BN_CTX_new() -> *mut BN_CTX;
    #[cfg(ossl110)]
    pub fn BN_CTX_secure_new() -> *mut BN_CTX;
    pub fn BN_CTX_free(ctx: *mut BN_CTX);
    pub fn BN_rand(r: *mut BIGNUM, bits: c_int, top: c_int, bottom: c_int) -> c_int;
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    pub fn BN_pseudo_rand(r: *mut BIGNUM, bits: c_int, top: c_int, bottom: c_int) -> c_int;
    pub fn BN_rand_range(r: *mut BIGNUM, range: *const BIGNUM) -> c_int;
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    pub fn BN_pseudo_rand_range(r: *mut BIGNUM, range: *const BIGNUM) -> c_int;
    pub fn BN_new() -> *mut BIGNUM;
    #[cfg(ossl110)]
    pub fn BN_secure_new() -> *mut BIGNUM;
    #[cfg(ossl110)]
    pub fn BN_set_flags(b: *mut BIGNUM, n: c_int);
    #[cfg(ossl110)]
    pub fn BN_get_flags(b: *const BIGNUM, n: c_int) -> c_int;
    pub fn BN_num_bits(bn: *const BIGNUM) -> c_int;
    pub fn BN_clear_free(bn: *mut BIGNUM);
    pub fn BN_bin2bn(s: *const u8, size: c_int, ret: *mut BIGNUM) -> *mut BIGNUM;
    pub fn BN_bn2bin(a: *const BIGNUM, to: *mut u8) -> c_int;
    #[cfg(any(ossl110, libressl340))]
    pub fn BN_bn2binpad(a: *const BIGNUM, to: *mut u8, tolen: c_int) -> c_int;
    pub fn BN_sub(r: *mut BIGNUM, a: *const BIGNUM, b: *const BIGNUM) -> c_int;
    pub fn BN_add(r: *mut BIGNUM, a: *const BIGNUM, b: *const BIGNUM) -> c_int;
    pub fn BN_mul(r: *mut BIGNUM, a: *const BIGNUM, b: *const BIGNUM, ctx: *mut BN_CTX) -> c_int;
    pub fn BN_sqr(r: *mut BIGNUM, a: *const BIGNUM, ctx: *mut BN_CTX) -> c_int;
    pub fn BN_set_negative(bn: *mut BIGNUM, n: c_int);
    #[cfg(any(ossl110, libressl350))]
    pub fn BN_is_negative(b: *const BIGNUM) -> c_int;
    #[cfg(any(ossl110, libressl350))]
    pub fn BN_is_odd(b: *const BIGNUM) -> c_int;

    pub fn BN_div(
        dv: *mut BIGNUM,
        rem: *mut BIGNUM,
        a: *const BIGNUM,
        b: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;
    pub fn BN_nnmod(
        rem: *mut BIGNUM,
        a: *const BIGNUM,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;
    pub fn BN_mod_add(
        r: *mut BIGNUM,
        a: *const BIGNUM,
        b: *const BIGNUM,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;
    pub fn BN_mod_sub(
        r: *mut BIGNUM,
        a: *const BIGNUM,
        b: *const BIGNUM,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;
    pub fn BN_mod_mul(
        r: *mut BIGNUM,
        a: *const BIGNUM,
        b: *const BIGNUM,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;
    pub fn BN_mod_sqr(
        r: *mut BIGNUM,
        a: *const BIGNUM,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;
    pub fn BN_mod_sqrt(
        ret: *mut BIGNUM,
        a: *const BIGNUM,
        p: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> *mut BIGNUM;

    pub fn BN_mod_word(r: *const BIGNUM, w: BN_ULONG) -> BN_ULONG;
    pub fn BN_div_word(r: *mut BIGNUM, w: BN_ULONG) -> BN_ULONG;
    pub fn BN_mul_word(r: *mut BIGNUM, w: BN_ULONG) -> c_int;
    pub fn BN_add_word(r: *mut BIGNUM, w: BN_ULONG) -> c_int;
    pub fn BN_sub_word(r: *mut BIGNUM, w: BN_ULONG) -> c_int;
    pub fn BN_set_word(bn: *mut BIGNUM, n: BN_ULONG) -> c_int;

    pub fn BN_cmp(a: *const BIGNUM, b: *const BIGNUM) -> c_int;
    pub fn BN_free(bn: *mut BIGNUM);
    pub fn BN_is_bit_set(a: *const BIGNUM, n: c_int) -> c_int;
    pub fn BN_lshift(r: *mut BIGNUM, a: *const BIGNUM, n: c_int) -> c_int;
    pub fn BN_lshift1(r: *mut BIGNUM, a: *const BIGNUM) -> c_int;
    pub fn BN_exp(r: *mut BIGNUM, a: *const BIGNUM, p: *const BIGNUM, ctx: *mut BN_CTX) -> c_int;

    pub fn BN_mod_exp(
        r: *mut BIGNUM,
        a: *const BIGNUM,
        p: *const BIGNUM,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn BN_mask_bits(a: *mut BIGNUM, n: c_int) -> c_int;
    pub fn BN_rshift(r: *mut BIGNUM, a: *const BIGNUM, n: c_int) -> c_int;
    pub fn BN_rshift1(r: *mut BIGNUM, a: *const BIGNUM) -> c_int;
    pub fn BN_bn2hex(a: *const BIGNUM) -> *mut c_char;
    pub fn BN_bn2dec(a: *const BIGNUM) -> *mut c_char;
    pub fn BN_hex2bn(a: *mut *mut BIGNUM, s: *const c_char) -> c_int;
    pub fn BN_dec2bn(a: *mut *mut BIGNUM, s: *const c_char) -> c_int;
    pub fn BN_gcd(r: *mut BIGNUM, a: *const BIGNUM, b: *const BIGNUM, ctx: *mut BN_CTX) -> c_int;
    pub fn BN_mod_inverse(
        r: *mut BIGNUM,
        a: *const BIGNUM,
        n: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> *mut BIGNUM;
    pub fn BN_clear(bn: *mut BIGNUM);
    pub fn BN_dup(n: *const BIGNUM) -> *mut BIGNUM;
    pub fn BN_ucmp(a: *const BIGNUM, b: *const BIGNUM) -> c_int;
    pub fn BN_set_bit(a: *mut BIGNUM, n: c_int) -> c_int;
    pub fn BN_clear_bit(a: *mut BIGNUM, n: c_int) -> c_int;

    pub fn BN_generate_prime_ex(
        r: *mut BIGNUM,
        bits: c_int,
        safe: c_int,
        add: *const BIGNUM,
        rem: *const BIGNUM,
        cb: *mut BN_GENCB,
    ) -> c_int;
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    pub fn BN_is_prime_ex(
        p: *const BIGNUM,
        checks: c_int,
        ctx: *mut BN_CTX,
        cb: *mut BN_GENCB,
    ) -> c_int;
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    pub fn BN_is_prime_fasttest_ex(
        p: *const BIGNUM,
        checks: c_int,
        ctx: *mut BN_CTX,
        do_trial_division: c_int,
        cb: *mut BN_GENCB,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        extern "C" {
            pub fn BN_get_rfc2409_prime_768(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc2409_prime_1024(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc3526_prime_1536(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc3526_prime_2048(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc3526_prime_3072(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc3526_prime_4096(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc3526_prime_6144(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn BN_get_rfc3526_prime_8192(bn: *mut BIGNUM) -> *mut BIGNUM;
        }
    } else {
        extern "C" {
            pub fn get_rfc2409_prime_768(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc2409_prime_1024(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc3526_prime_1536(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc3526_prime_2048(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc3526_prime_3072(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc3526_prime_4096(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc3526_prime_6144(bn: *mut BIGNUM) -> *mut BIGNUM;
            pub fn get_rfc3526_prime_8192(bn: *mut BIGNUM) -> *mut BIGNUM;
        }
    }
}
