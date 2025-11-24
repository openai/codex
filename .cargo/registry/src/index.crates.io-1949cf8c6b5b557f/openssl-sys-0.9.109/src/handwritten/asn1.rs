use super::super::*;
use libc::*;

#[repr(C)]
pub struct ASN1_ENCODING {
    pub enc: *mut c_uchar,
    pub len: c_long,
    pub modified: c_int,
}

extern "C" {
    pub fn ASN1_OBJECT_free(x: *mut ASN1_OBJECT);
    pub fn OBJ_dup(x: *const ASN1_OBJECT) -> *mut ASN1_OBJECT;
}

stack!(stack_st_ASN1_OBJECT);

#[repr(C)]
pub struct ASN1_TYPE {
    pub type_: c_int,
    pub value: ASN1_TYPE_value,
}
#[repr(C)]
pub union ASN1_TYPE_value {
    pub ptr: *mut c_char,
    pub boolean: ASN1_BOOLEAN,
    pub asn1_string: *mut ASN1_STRING,
    pub object: *mut ASN1_OBJECT,
    pub integer: *mut ASN1_INTEGER,
    pub enumerated: *mut ASN1_ENUMERATED,
    pub bit_string: *mut ASN1_BIT_STRING,
    pub octet_string: *mut ASN1_OCTET_STRING,
    pub printablestring: *mut ASN1_PRINTABLESTRING,
    pub t61string: *mut ASN1_T61STRING,
    pub ia5string: *mut ASN1_IA5STRING,
    pub generalstring: *mut ASN1_GENERALSTRING,
    pub bmpstring: *mut ASN1_BMPSTRING,
    pub universalstring: *mut ASN1_UNIVERSALSTRING,
    pub utctime: *mut ASN1_UTCTIME,
    pub generalizedtime: *mut ASN1_GENERALIZEDTIME,
    pub visiblestring: *mut ASN1_VISIBLESTRING,
    pub utf8string: *mut ASN1_UTF8STRING,
    pub set: *mut ASN1_STRING,
    pub sequence: *mut ASN1_STRING,
    pub asn1_value: *mut ASN1_VALUE,
}

extern "C" {
    pub fn ASN1_STRING_type_new(ty: c_int) -> *mut ASN1_STRING;
    #[cfg(any(ossl110, libressl273))]
    pub fn ASN1_STRING_get0_data(x: *const ASN1_STRING) -> *const c_uchar;
    #[cfg(any(all(ossl101, not(ossl110)), libressl))]
    pub fn ASN1_STRING_data(x: *mut ASN1_STRING) -> *mut c_uchar;
    pub fn ASN1_STRING_new() -> *mut ASN1_STRING;
    pub fn ASN1_OCTET_STRING_new() -> *mut ASN1_OCTET_STRING;
    pub fn ASN1_STRING_free(x: *mut ASN1_STRING);
    pub fn ASN1_STRING_length(x: *const ASN1_STRING) -> c_int;
    pub fn ASN1_STRING_set(x: *mut ASN1_STRING, data: *const c_void, len_in: c_int) -> c_int;
    pub fn ASN1_OCTET_STRING_set(
        x: *mut ASN1_OCTET_STRING,
        data: *const c_uchar,
        len_in: c_int,
    ) -> c_int;

    pub fn ASN1_BIT_STRING_free(x: *mut ASN1_BIT_STRING);
    pub fn ASN1_OCTET_STRING_free(x: *mut ASN1_OCTET_STRING);

    pub fn ASN1_GENERALIZEDTIME_free(tm: *mut ASN1_GENERALIZEDTIME);
    pub fn ASN1_GENERALIZEDTIME_print(b: *mut BIO, tm: *const ASN1_GENERALIZEDTIME) -> c_int;
    pub fn ASN1_TIME_new() -> *mut ASN1_TIME;
    #[cfg(ossl102)]
    pub fn ASN1_TIME_diff(
        pday: *mut c_int,
        psec: *mut c_int,
        from: *const ASN1_TIME,
        to: *const ASN1_TIME,
    ) -> c_int;
    pub fn ASN1_TIME_free(tm: *mut ASN1_TIME);
    pub fn ASN1_TIME_print(b: *mut BIO, tm: *const ASN1_TIME) -> c_int;
    pub fn ASN1_TIME_set(from: *mut ASN1_TIME, to: time_t) -> *mut ASN1_TIME;

    pub fn ASN1_INTEGER_free(x: *mut ASN1_INTEGER);
    pub fn ASN1_INTEGER_dup(a: *const ASN1_INTEGER) -> *mut ASN1_INTEGER;
    pub fn ASN1_INTEGER_get(dest: *const ASN1_INTEGER) -> c_long;
    pub fn ASN1_INTEGER_set(dest: *mut ASN1_INTEGER, value: c_long) -> c_int;
    pub fn ASN1_INTEGER_cmp(a: *const ASN1_INTEGER, b: *const ASN1_INTEGER) -> c_int;
    pub fn BN_to_ASN1_INTEGER(bn: *const BIGNUM, ai: *mut ASN1_INTEGER) -> *mut ASN1_INTEGER;
    pub fn ASN1_INTEGER_to_BN(ai: *const ASN1_INTEGER, bn: *mut BIGNUM) -> *mut BIGNUM;

    pub fn ASN1_TIME_set_string(s: *mut ASN1_TIME, str: *const c_char) -> c_int;
    #[cfg(ossl111)]
    pub fn ASN1_TIME_set_string_X509(s: *mut ASN1_TIME, str: *const c_char) -> c_int;

    pub fn ASN1_ENUMERATED_free(a: *mut ASN1_ENUMERATED);
    #[cfg(ossl110)]
    pub fn ASN1_ENUMERATED_get_int64(pr: *mut i64, a: *const ASN1_ENUMERATED) -> c_int;

    pub fn ASN1_TYPE_new() -> *mut ASN1_TYPE;
    pub fn ASN1_TYPE_set(a: *mut ASN1_TYPE, type_: c_int, value: *mut c_void);
    pub fn ASN1_TYPE_free(x: *mut ASN1_TYPE);
    pub fn d2i_ASN1_TYPE(
        k: *mut *mut ASN1_TYPE,
        buf: *mut *const u8,
        len: c_long,
    ) -> *mut ASN1_TYPE;
}

const_ptr_api! {
    extern "C" {
        pub fn ASN1_STRING_to_UTF8(out: *mut *mut c_uchar, s: #[const_ptr_if(any(ossl110, libressl280))] ASN1_STRING) -> c_int;
        pub fn ASN1_STRING_type(x: #[const_ptr_if(any(ossl110, libressl280))]  ASN1_STRING) -> c_int;
        pub fn ASN1_generate_v3(str: #[const_ptr_if(any(ossl110, libressl280))] c_char, cnf: *mut X509V3_CTX) -> *mut ASN1_TYPE;
        pub fn i2d_ASN1_TYPE(a: #[const_ptr_if(ossl300)] ASN1_TYPE, pp: *mut *mut c_uchar) -> c_int;
    }
}
