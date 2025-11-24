use super::super::*;
use libc::*;

#[repr(C)]
#[derive(Copy, Clone)]
pub enum point_conversion_form_t {
    POINT_CONVERSION_COMPRESSED = 2,
    POINT_CONVERSION_UNCOMPRESSED = 4,
    POINT_CONVERSION_HYBRID = 6,
}

#[cfg(not(libressl410))]
pub enum EC_METHOD {}
pub enum EC_GROUP {}
pub enum EC_POINT {}

extern "C" {
    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    pub fn EC_GF2m_simple_method() -> *const EC_METHOD;

    #[cfg(not(libressl410))]
    pub fn EC_GROUP_new(meth: *const EC_METHOD) -> *mut EC_GROUP;

    pub fn EC_GROUP_free(group: *mut EC_GROUP);

    pub fn EC_GROUP_get_order(
        group: *const EC_GROUP,
        order: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_GROUP_get_cofactor(
        group: *const EC_GROUP,
        cofactor: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_GROUP_get0_generator(group: *const EC_GROUP) -> *const EC_POINT;

    pub fn EC_GROUP_set_generator(
        group: *mut EC_GROUP,
        generator: *const EC_POINT,
        order: *const BIGNUM,
        cofactor: *const BIGNUM,
    ) -> c_int;

    pub fn EC_GROUP_get_curve_name(group: *const EC_GROUP) -> c_int;

    pub fn EC_GROUP_set_asn1_flag(key: *mut EC_GROUP, flag: c_int);

    pub fn EC_GROUP_get_asn1_flag(group: *const EC_GROUP) -> c_int;

    pub fn EC_GROUP_get_curve_GFp(
        group: *const EC_GROUP,
        p: *mut BIGNUM,
        a: *mut BIGNUM,
        b: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    pub fn EC_GROUP_get_curve_GF2m(
        group: *const EC_GROUP,
        p: *mut BIGNUM,
        a: *mut BIGNUM,
        b: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_GROUP_get_degree(group: *const EC_GROUP) -> c_int;

    #[cfg(ossl110)]
    pub fn EC_GROUP_order_bits(group: *const EC_GROUP) -> c_int;

    pub fn EC_GROUP_new_curve_GFp(
        p: *const BIGNUM,
        a: *const BIGNUM,
        b: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> *mut EC_GROUP;

    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    pub fn EC_GROUP_new_curve_GF2m(
        p: *const BIGNUM,
        a: *const BIGNUM,
        b: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> *mut EC_GROUP;

    pub fn EC_GROUP_new_by_curve_name(nid: c_int) -> *mut EC_GROUP;

    pub fn EC_POINT_is_at_infinity(group: *const EC_GROUP, point: *const EC_POINT) -> c_int;

    pub fn EC_POINT_is_on_curve(
        group: *const EC_GROUP,
        point: *const EC_POINT,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_new(group: *const EC_GROUP) -> *mut EC_POINT;

    pub fn EC_POINT_free(point: *mut EC_POINT);

    pub fn EC_POINT_dup(p: *const EC_POINT, group: *const EC_GROUP) -> *mut EC_POINT;

    #[cfg(any(ossl111, boringssl, libressl350, awslc))]
    pub fn EC_POINT_get_affine_coordinates(
        group: *const EC_GROUP,
        p: *const EC_POINT,
        x: *mut BIGNUM,
        y: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_get_affine_coordinates_GFp(
        group: *const EC_GROUP,
        p: *const EC_POINT,
        x: *mut BIGNUM,
        y: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_set_affine_coordinates_GFp(
        group: *const EC_GROUP,
        p: *mut EC_POINT,
        x: *const BIGNUM,
        y: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    pub fn EC_POINT_get_affine_coordinates_GF2m(
        group: *const EC_GROUP,
        p: *const EC_POINT,
        x: *mut BIGNUM,
        y: *mut BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_point2oct(
        group: *const EC_GROUP,
        p: *const EC_POINT,
        form: point_conversion_form_t,
        buf: *mut c_uchar,
        len: size_t,
        ctx: *mut BN_CTX,
    ) -> size_t;

    pub fn EC_POINT_oct2point(
        group: *const EC_GROUP,
        p: *mut EC_POINT,
        buf: *const c_uchar,
        len: size_t,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_point2hex(
        group: *const EC_GROUP,
        p: *const EC_POINT,
        form: point_conversion_form_t,
        ctx: *mut BN_CTX,
    ) -> *mut c_char;

    pub fn EC_POINT_hex2point(
        group: *const EC_GROUP,
        s: *const c_char,
        p: *mut EC_POINT,
        ctx: *mut BN_CTX,
    ) -> *mut EC_POINT;

    pub fn EC_POINT_add(
        group: *const EC_GROUP,
        r: *mut EC_POINT,
        a: *const EC_POINT,
        b: *const EC_POINT,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_invert(group: *const EC_GROUP, r: *mut EC_POINT, ctx: *mut BN_CTX) -> c_int;

    pub fn EC_POINT_cmp(
        group: *const EC_GROUP,
        a: *const EC_POINT,
        b: *const EC_POINT,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_POINT_mul(
        group: *const EC_GROUP,
        r: *mut EC_POINT,
        n: *const BIGNUM,
        q: *const EC_POINT,
        m: *const BIGNUM,
        ctx: *mut BN_CTX,
    ) -> c_int;

    pub fn EC_KEY_new() -> *mut EC_KEY;

    pub fn EC_KEY_new_by_curve_name(nid: c_int) -> *mut EC_KEY;

    pub fn EC_KEY_free(key: *mut EC_KEY);

    pub fn EC_KEY_dup(key: *const EC_KEY) -> *mut EC_KEY;

    pub fn EC_KEY_up_ref(key: *mut EC_KEY) -> c_int;

    pub fn EC_KEY_get0_group(key: *const EC_KEY) -> *const EC_GROUP;

    pub fn EC_KEY_set_group(key: *mut EC_KEY, group: *const EC_GROUP) -> c_int;

    pub fn EC_KEY_get0_private_key(key: *const EC_KEY) -> *const BIGNUM;

    pub fn EC_KEY_set_private_key(key: *mut EC_KEY, key: *const BIGNUM) -> c_int;

    pub fn EC_KEY_get0_public_key(key: *const EC_KEY) -> *const EC_POINT;

    pub fn EC_KEY_set_public_key(key: *mut EC_KEY, key: *const EC_POINT) -> c_int;

    pub fn EC_KEY_generate_key(key: *mut EC_KEY) -> c_int;

    pub fn EC_KEY_check_key(key: *const EC_KEY) -> c_int;

    pub fn EC_KEY_set_public_key_affine_coordinates(
        key: *mut EC_KEY,
        x: *mut BIGNUM,
        y: *mut BIGNUM,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum ECDSA_SIG {}
    } else {
        #[repr(C)]
        pub struct ECDSA_SIG {
            pub r: *mut BIGNUM,
            pub s: *mut BIGNUM,
        }
    }
}

extern "C" {
    pub fn ECDSA_SIG_new() -> *mut ECDSA_SIG;

    pub fn ECDSA_SIG_free(sig: *mut ECDSA_SIG);

    #[cfg(any(ossl110, libressl273))]
    pub fn ECDSA_SIG_get0(sig: *const ECDSA_SIG, pr: *mut *const BIGNUM, ps: *mut *const BIGNUM);

    #[cfg(any(ossl110, libressl273))]
    pub fn ECDSA_SIG_set0(sig: *mut ECDSA_SIG, pr: *mut BIGNUM, ps: *mut BIGNUM) -> c_int;

    pub fn ECDSA_do_sign(
        dgst: *const c_uchar,
        dgst_len: c_int,
        eckey: *mut EC_KEY,
    ) -> *mut ECDSA_SIG;

    pub fn ECDSA_do_verify(
        dgst: *const c_uchar,
        dgst_len: c_int,
        sig: *const ECDSA_SIG,
        eckey: *mut EC_KEY,
    ) -> c_int;

    pub fn d2i_ECDSA_SIG(
        sig: *mut *mut ECDSA_SIG,
        inp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut ECDSA_SIG;

    pub fn i2d_ECDSA_SIG(sig: *const ECDSA_SIG, out: *mut *mut c_uchar) -> c_int;
}
