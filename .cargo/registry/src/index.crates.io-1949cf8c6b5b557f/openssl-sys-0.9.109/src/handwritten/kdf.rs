use super::super::*;
use libc::*;

cfg_if! {
    if #[cfg(ossl300)] {
        extern "C" {
            pub fn EVP_PKEY_CTX_set_hkdf_mode(ctx: *mut EVP_PKEY_CTX, mode: c_int) -> c_int;
            pub fn EVP_PKEY_CTX_set_hkdf_md(ctx: *mut EVP_PKEY_CTX, md: *const EVP_MD) -> c_int;
            pub fn EVP_PKEY_CTX_set1_hkdf_salt(
                ctx: *mut EVP_PKEY_CTX,
                salt: *const u8,
                saltlen: c_int,
            ) -> c_int;
            pub fn EVP_PKEY_CTX_set1_hkdf_key(
                ctx: *mut EVP_PKEY_CTX,
                key: *const u8,
                keylen: c_int,
            ) -> c_int;
            pub fn EVP_PKEY_CTX_add1_hkdf_info(
                ctx: *mut EVP_PKEY_CTX,
                info: *const u8,
                infolen: c_int,
            ) -> c_int;
            pub fn EVP_KDF_CTX_new(kdf: *mut EVP_KDF) -> *mut EVP_KDF_CTX;
            pub fn EVP_KDF_CTX_free(ctx: *mut EVP_KDF_CTX);
            pub fn EVP_KDF_CTX_reset(ctx: *mut EVP_KDF_CTX);
            pub fn EVP_KDF_CTX_get_kdf_size(ctx: *mut EVP_KDF_CTX) -> size_t;
            pub fn EVP_KDF_derive(ctx: *mut EVP_KDF_CTX, key: *mut u8, keylen: size_t, params: *const OSSL_PARAM) -> c_int;
            pub fn EVP_KDF_fetch(ctx: *mut OSSL_LIB_CTX, algorithm: *const c_char, properties: *const c_char) -> *mut EVP_KDF;
            pub fn EVP_KDF_free(kdf: *mut EVP_KDF);
        }

    }
}
