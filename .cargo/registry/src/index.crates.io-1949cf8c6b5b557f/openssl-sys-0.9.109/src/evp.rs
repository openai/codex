use super::*;
use libc::*;

pub const EVP_MAX_MD_SIZE: c_uint = 64;

pub const PKCS5_SALT_LEN: c_int = 8;
pub const PKCS12_DEFAULT_ITER: c_int = 2048;

pub const EVP_PKEY_RSA: c_int = NID_rsaEncryption;
#[cfg(any(ossl111, libressl310, boringssl, awslc))]
pub const EVP_PKEY_RSA_PSS: c_int = NID_rsassaPss;
pub const EVP_PKEY_DSA: c_int = NID_dsa;
pub const EVP_PKEY_DH: c_int = NID_dhKeyAgreement;
#[cfg(ossl110)]
pub const EVP_PKEY_DHX: c_int = NID_dhpublicnumber;
pub const EVP_PKEY_EC: c_int = NID_X9_62_id_ecPublicKey;
#[cfg(ossl111)]
pub const EVP_PKEY_SM2: c_int = NID_sm2;
#[cfg(any(ossl111, libressl370))]
pub const EVP_PKEY_X25519: c_int = NID_X25519;
#[cfg(any(ossl111, libressl370))]
pub const EVP_PKEY_ED25519: c_int = NID_ED25519;
#[cfg(ossl111)]
pub const EVP_PKEY_X448: c_int = NID_X448;
#[cfg(ossl111)]
pub const EVP_PKEY_ED448: c_int = NID_ED448;
pub const EVP_PKEY_HMAC: c_int = NID_hmac;
pub const EVP_PKEY_CMAC: c_int = NID_cmac;
#[cfg(ossl111)]
pub const EVP_PKEY_POLY1305: c_int = NID_poly1305;
#[cfg(any(ossl110, libressl360))]
pub const EVP_PKEY_HKDF: c_int = NID_hkdf;

#[cfg(ossl102)]
pub const EVP_CIPHER_CTX_FLAG_WRAP_ALLOW: c_int = 0x1;

pub const EVP_CTRL_GCM_SET_IVLEN: c_int = 0x9;
pub const EVP_CTRL_GCM_GET_TAG: c_int = 0x10;
pub const EVP_CTRL_GCM_SET_TAG: c_int = 0x11;

pub unsafe fn EVP_get_digestbynid(type_: c_int) -> *const EVP_MD {
    EVP_get_digestbyname(OBJ_nid2sn(type_))
}

cfg_if! {
    if #[cfg(ossl300)] {
        #[inline]
        pub unsafe fn EVP_MD_CTX_md(ctx: *const EVP_MD_CTX) -> *const EVP_MD {
            EVP_MD_CTX_get0_md(ctx)
        }

        #[inline]
        pub unsafe fn EVP_MD_CTX_get_size(ctx: *const EVP_MD_CTX) -> c_int {
            EVP_MD_get_size(EVP_MD_CTX_get0_md(ctx))
        }

        #[inline]
        pub unsafe fn EVP_MD_CTX_size(ctx: *const EVP_MD_CTX) -> c_int {
            EVP_MD_CTX_get_size(ctx)
        }

        #[inline]
        pub unsafe fn EVP_MD_block_size(md: *const EVP_MD) -> c_int {
            EVP_MD_get_block_size(md)
        }

        #[inline]
        pub unsafe fn EVP_MD_size(md: *const EVP_MD) -> c_int {
            EVP_MD_get_size(md)
        }

        #[inline]
        pub unsafe fn EVP_MD_type(md: *const EVP_MD) -> c_int {
            EVP_MD_get_type(md)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_key_length(cipher: *const EVP_CIPHER) -> c_int {
            EVP_CIPHER_get_key_length(cipher)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_block_size(cipher: *const EVP_CIPHER) -> c_int {
            EVP_CIPHER_get_block_size(cipher)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_iv_length(cipher: *const EVP_CIPHER) -> c_int {
            EVP_CIPHER_get_iv_length(cipher)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_nid(cipher: *const EVP_CIPHER) -> c_int {
            EVP_CIPHER_get_nid(cipher)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_CTX_block_size(ctx: *const EVP_CIPHER_CTX) -> c_int {
            EVP_CIPHER_CTX_get_block_size(ctx)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_CTX_key_length(ctx: *const EVP_CIPHER_CTX) -> c_int {
            EVP_CIPHER_CTX_get_key_length(ctx)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_CTX_iv_length(ctx: *const EVP_CIPHER_CTX) -> c_int {
            EVP_CIPHER_CTX_get_iv_length(ctx)
        }

        #[inline]
        pub unsafe fn EVP_CIPHER_CTX_num(ctx: *const EVP_CIPHER_CTX) -> c_int {
            EVP_CIPHER_CTX_get_num(ctx)
        }
    } else {
        pub unsafe fn EVP_MD_CTX_size(ctx: *const EVP_MD_CTX) -> c_int {
            EVP_MD_size(EVP_MD_CTX_md(ctx))
        }
    }
}
#[cfg(not(ossl300))]
#[inline]
pub unsafe fn EVP_DigestSignUpdate(
    ctx: *mut EVP_MD_CTX,
    data: *const c_void,
    dsize: size_t,
) -> c_int {
    EVP_DigestUpdate(ctx, data, dsize)
}
#[cfg(not(ossl300))]
#[inline]
pub unsafe fn EVP_DigestVerifyUpdate(
    ctx: *mut EVP_MD_CTX,
    data: *const c_void,
    dsize: size_t,
) -> c_int {
    EVP_DigestUpdate(ctx, data, dsize)
}
#[cfg(ossl300)]
#[inline]
pub unsafe fn EVP_PKEY_size(pkey: *const EVP_PKEY) -> c_int {
    EVP_PKEY_get_size(pkey)
}

cfg_if! {
    if #[cfg(ossl300)] {
        #[inline]
        pub unsafe fn EVP_PKEY_id(pkey: *const EVP_PKEY) -> c_int {
            EVP_PKEY_get_id(pkey)
        }

        #[inline]
        pub unsafe fn EVP_PKEY_bits(pkey: *const EVP_PKEY) -> c_int {
            EVP_PKEY_get_bits(pkey)
        }

        #[inline]
        pub unsafe fn EVP_PKEY_security_bits(pkey: *const EVP_PKEY) -> c_int {
            EVP_PKEY_get_security_bits(pkey)
        }
    }
}

pub const EVP_PKEY_OP_KEYGEN: c_int = 1 << 2;
cfg_if! {
    if #[cfg(ossl300)] {
        pub const EVP_PKEY_OP_SIGN: c_int = 1 << 4;
        pub const EVP_PKEY_OP_VERIFY: c_int = 1 << 5;
        pub const EVP_PKEY_OP_VERIFYRECOVER: c_int = 1 << 6;
        pub const EVP_PKEY_OP_SIGNCTX: c_int = 1 << 7;
        pub const EVP_PKEY_OP_VERIFYCTX: c_int = 1 << 8;
        pub const EVP_PKEY_OP_ENCRYPT: c_int = 1 << 9;
        pub const EVP_PKEY_OP_DECRYPT: c_int = 1 << 10;
        pub const EVP_PKEY_OP_DERIVE: c_int = 1 << 11;
    } else {
        pub const EVP_PKEY_OP_SIGN: c_int = 1 << 3;
        pub const EVP_PKEY_OP_VERIFY: c_int = 1 << 4;
        pub const EVP_PKEY_OP_VERIFYRECOVER: c_int = 1 << 5;
        pub const EVP_PKEY_OP_SIGNCTX: c_int = 1 << 6;
        pub const EVP_PKEY_OP_VERIFYCTX: c_int = 1 << 7;
        pub const EVP_PKEY_OP_ENCRYPT: c_int = 1 << 8;
        pub const EVP_PKEY_OP_DECRYPT: c_int = 1 << 9;
        pub const EVP_PKEY_OP_DERIVE: c_int = 1 << 10;
    }
}
#[cfg(ossl340)]
pub const EVP_PKEY_OP_SIGNMSG: c_int = 1 << 14;
#[cfg(ossl340)]
pub const EVP_PKEY_OP_VERIFYMSG: c_int = 1 << 15;

cfg_if! {
    if #[cfg(ossl340)] {
        pub const EVP_PKEY_OP_TYPE_SIG: c_int = EVP_PKEY_OP_SIGN
            | EVP_PKEY_OP_SIGNMSG
            | EVP_PKEY_OP_VERIFY
            | EVP_PKEY_OP_VERIFYMSG
            | EVP_PKEY_OP_VERIFYRECOVER
            | EVP_PKEY_OP_SIGNCTX
            | EVP_PKEY_OP_VERIFYCTX;
    } else {
        pub const EVP_PKEY_OP_TYPE_SIG: c_int = EVP_PKEY_OP_SIGN
            | EVP_PKEY_OP_VERIFY
            | EVP_PKEY_OP_VERIFYRECOVER
            | EVP_PKEY_OP_SIGNCTX
            | EVP_PKEY_OP_VERIFYCTX;
    }
}

pub const EVP_PKEY_OP_TYPE_CRYPT: c_int = EVP_PKEY_OP_ENCRYPT | EVP_PKEY_OP_DECRYPT;

pub const EVP_PKEY_CTRL_MD: c_int = 1;

pub const EVP_PKEY_CTRL_SET_MAC_KEY: c_int = 6;

pub const EVP_PKEY_CTRL_CIPHER: c_int = 12;

pub const EVP_PKEY_ALG_CTRL: c_int = 0x1000;

#[cfg(any(ossl111, libressl360))]
pub const EVP_PKEY_HKDEF_MODE_EXTRACT_AND_EXPAND: c_int = 0;

#[cfg(any(ossl111, libressl360))]
pub const EVP_PKEY_HKDEF_MODE_EXTRACT_ONLY: c_int = 1;

#[cfg(any(ossl111, libressl360))]
pub const EVP_PKEY_HKDEF_MODE_EXPAND_ONLY: c_int = 2;

#[cfg(any(ossl110, libressl360))]
pub const EVP_PKEY_CTRL_HKDF_MD: c_int = EVP_PKEY_ALG_CTRL + 3;

#[cfg(any(ossl110, libressl360))]
pub const EVP_PKEY_CTRL_HKDF_SALT: c_int = EVP_PKEY_ALG_CTRL + 4;

#[cfg(any(ossl110, libressl360))]
pub const EVP_PKEY_CTRL_HKDF_KEY: c_int = EVP_PKEY_ALG_CTRL + 5;

#[cfg(any(ossl110, libressl360))]
pub const EVP_PKEY_CTRL_HKDF_INFO: c_int = EVP_PKEY_ALG_CTRL + 6;

#[cfg(any(ossl111, libressl360))]
pub const EVP_PKEY_CTRL_HKDF_MODE: c_int = EVP_PKEY_ALG_CTRL + 7;

#[cfg(any(all(ossl111, not(ossl300)), libressl360))]
pub unsafe fn EVP_PKEY_CTX_set_hkdf_mode(ctx: *mut EVP_PKEY_CTX, mode: c_int) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        -1,
        EVP_PKEY_OP_DERIVE,
        EVP_PKEY_CTRL_HKDF_MODE,
        mode,
        std::ptr::null_mut(),
    )
}

#[cfg(any(all(ossl110, not(ossl300)), libressl360))]
pub unsafe fn EVP_PKEY_CTX_set_hkdf_md(ctx: *mut EVP_PKEY_CTX, md: *const EVP_MD) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        -1,
        EVP_PKEY_OP_DERIVE,
        EVP_PKEY_CTRL_HKDF_MD,
        0,
        md as *mut c_void,
    )
}

#[cfg(any(all(ossl110, not(ossl300)), libressl360))]
pub unsafe fn EVP_PKEY_CTX_set1_hkdf_salt(
    ctx: *mut EVP_PKEY_CTX,
    salt: *const u8,
    saltlen: c_int,
) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        -1,
        EVP_PKEY_OP_DERIVE,
        EVP_PKEY_CTRL_HKDF_SALT,
        saltlen,
        salt as *mut c_void,
    )
}

#[cfg(any(all(ossl110, not(ossl300)), libressl360))]
pub unsafe fn EVP_PKEY_CTX_set1_hkdf_key(
    ctx: *mut EVP_PKEY_CTX,
    key: *const u8,
    keylen: c_int,
) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        -1,
        EVP_PKEY_OP_DERIVE,
        EVP_PKEY_CTRL_HKDF_KEY,
        keylen,
        key as *mut c_void,
    )
}

#[cfg(any(all(ossl110, not(ossl300)), libressl360))]
pub unsafe fn EVP_PKEY_CTX_add1_hkdf_info(
    ctx: *mut EVP_PKEY_CTX,
    info: *const u8,
    infolen: c_int,
) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        -1,
        EVP_PKEY_OP_DERIVE,
        EVP_PKEY_CTRL_HKDF_INFO,
        infolen,
        info as *mut c_void,
    )
}

#[cfg(not(any(ossl300, boringssl, awslc)))]
pub unsafe fn EVP_PKEY_CTX_set_signature_md(cxt: *mut EVP_PKEY_CTX, md: *mut EVP_MD) -> c_int {
    EVP_PKEY_CTX_ctrl(
        cxt,
        -1,
        EVP_PKEY_OP_TYPE_SIG,
        EVP_PKEY_CTRL_MD,
        0,
        md as *mut c_void,
    )
}

pub unsafe fn EVP_PKEY_assign_RSA(pkey: *mut EVP_PKEY, rsa: *mut RSA) -> c_int {
    EVP_PKEY_assign(pkey, EVP_PKEY_RSA, rsa as *mut c_void)
}

pub unsafe fn EVP_PKEY_assign_DSA(pkey: *mut EVP_PKEY, dsa: *mut DSA) -> c_int {
    EVP_PKEY_assign(pkey, EVP_PKEY_DSA, dsa as *mut c_void)
}

pub unsafe fn EVP_PKEY_assign_DH(pkey: *mut EVP_PKEY, dh: *mut DH) -> c_int {
    EVP_PKEY_assign(pkey, EVP_PKEY_DH, dh as *mut c_void)
}

pub unsafe fn EVP_PKEY_assign_EC_KEY(pkey: *mut EVP_PKEY, ec_key: *mut EC_KEY) -> c_int {
    EVP_PKEY_assign(pkey, EVP_PKEY_EC, ec_key as *mut c_void)
}
