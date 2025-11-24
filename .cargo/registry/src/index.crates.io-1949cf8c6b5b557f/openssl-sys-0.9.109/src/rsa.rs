use libc::*;
use std::ptr;

use super::super::*;

pub const RSA_F4: c_long = 0x10001;

cfg_if! {
    if #[cfg(not(ossl300))] {
        pub unsafe fn EVP_PKEY_CTX_set_rsa_padding(ctx: *mut EVP_PKEY_CTX, pad: c_int) -> c_int {
            EVP_PKEY_CTX_ctrl(
                ctx,
                EVP_PKEY_RSA,
                -1,
                EVP_PKEY_CTRL_RSA_PADDING,
                pad,
                ptr::null_mut(),
            )
        }
        pub unsafe fn EVP_PKEY_CTX_get_rsa_padding(ctx: *mut EVP_PKEY_CTX, ppad: *mut c_int) -> c_int {
            EVP_PKEY_CTX_ctrl(
                ctx,
                EVP_PKEY_RSA,
                -1,
                EVP_PKEY_CTRL_GET_RSA_PADDING,
                0,
                ppad as *mut c_void,
            )
        }

        pub unsafe fn EVP_PKEY_CTX_set_rsa_pss_saltlen(ctx: *mut EVP_PKEY_CTX, len: c_int) -> c_int {
            EVP_PKEY_CTX_ctrl(
                ctx,
                EVP_PKEY_RSA,
                EVP_PKEY_OP_SIGN | EVP_PKEY_OP_VERIFY,
                EVP_PKEY_CTRL_RSA_PSS_SALTLEN,
                len,
                ptr::null_mut(),
            )
        }

        pub unsafe fn EVP_PKEY_CTX_set_rsa_mgf1_md(ctx: *mut EVP_PKEY_CTX, md: *mut EVP_MD) -> c_int {
            EVP_PKEY_CTX_ctrl(
                ctx,
                EVP_PKEY_RSA,
                EVP_PKEY_OP_TYPE_SIG | EVP_PKEY_OP_TYPE_CRYPT,
                EVP_PKEY_CTRL_RSA_MGF1_MD,
                0,
                md as *mut c_void,
            )
        }
    }
}

#[cfg(any(ossl102, libressl310))]
pub unsafe fn EVP_PKEY_CTX_set_rsa_oaep_md(ctx: *mut EVP_PKEY_CTX, md: *mut EVP_MD) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        EVP_PKEY_RSA,
        EVP_PKEY_OP_TYPE_CRYPT,
        EVP_PKEY_CTRL_RSA_OAEP_MD,
        0,
        md as *mut c_void,
    )
}

#[cfg(any(ossl102, libressl310))]
pub unsafe fn EVP_PKEY_CTX_set0_rsa_oaep_label(
    ctx: *mut EVP_PKEY_CTX,
    label: *mut c_void,
    len: c_int,
) -> c_int {
    EVP_PKEY_CTX_ctrl(
        ctx,
        EVP_PKEY_RSA,
        EVP_PKEY_OP_TYPE_CRYPT,
        EVP_PKEY_CTRL_RSA_OAEP_LABEL,
        len,
        label,
    )
}

pub const EVP_PKEY_CTRL_RSA_PADDING: c_int = EVP_PKEY_ALG_CTRL + 1;
pub const EVP_PKEY_CTRL_RSA_PSS_SALTLEN: c_int = EVP_PKEY_ALG_CTRL + 2;

pub const EVP_PKEY_CTRL_RSA_MGF1_MD: c_int = EVP_PKEY_ALG_CTRL + 5;

pub const EVP_PKEY_CTRL_GET_RSA_PADDING: c_int = EVP_PKEY_ALG_CTRL + 6;

#[cfg(any(ossl102, libressl310))]
pub const EVP_PKEY_CTRL_RSA_OAEP_MD: c_int = EVP_PKEY_ALG_CTRL + 9;
#[cfg(any(ossl102, libressl310))]
pub const EVP_PKEY_CTRL_RSA_OAEP_LABEL: c_int = EVP_PKEY_ALG_CTRL + 10;

pub const RSA_PKCS1_PADDING: c_int = 1;
#[cfg(not(ossl300))]
pub const RSA_SSLV23_PADDING: c_int = 2;
pub const RSA_NO_PADDING: c_int = 3;
pub const RSA_PKCS1_OAEP_PADDING: c_int = 4;
pub const RSA_X931_PADDING: c_int = 5;
pub const RSA_PKCS1_PSS_PADDING: c_int = 6;
