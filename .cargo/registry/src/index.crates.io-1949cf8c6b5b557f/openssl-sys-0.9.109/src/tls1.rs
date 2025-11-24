use libc::*;
use std::mem;
use std::ptr;

use super::*;

pub const TLS1_VERSION: c_int = 0x301;
pub const TLS1_1_VERSION: c_int = 0x302;
pub const TLS1_2_VERSION: c_int = 0x303;
#[cfg(any(ossl111, libressl340))]
pub const TLS1_3_VERSION: c_int = 0x304;

pub const DTLS1_VERSION: c_int = 0xFEFF;
#[cfg(any(ossl102, libressl332))]
pub const DTLS1_2_VERSION: c_int = 0xFEFD;

pub const TLS1_AD_DECODE_ERROR: c_int = 50;
pub const TLS1_AD_UNRECOGNIZED_NAME: c_int = 112;

pub const TLSEXT_NAMETYPE_host_name: c_int = 0;
pub const TLSEXT_STATUSTYPE_ocsp: c_int = 1;

pub unsafe fn SSL_set_tlsext_host_name(s: *mut SSL, name: *mut c_char) -> c_long {
    SSL_ctrl(
        s,
        SSL_CTRL_SET_TLSEXT_HOSTNAME,
        TLSEXT_NAMETYPE_host_name as c_long,
        name as *mut c_void,
    )
}

pub unsafe fn SSL_set_tlsext_status_type(s: *mut SSL, type_: c_int) -> c_long {
    SSL_ctrl(
        s,
        SSL_CTRL_SET_TLSEXT_STATUS_REQ_TYPE,
        type_ as c_long,
        ptr::null_mut(),
    )
}

pub unsafe fn SSL_get_tlsext_status_ocsp_resp(ssl: *mut SSL, resp: *mut *mut c_uchar) -> c_long {
    SSL_ctrl(
        ssl,
        SSL_CTRL_GET_TLSEXT_STATUS_REQ_OCSP_RESP,
        0,
        resp as *mut c_void,
    )
}

pub unsafe fn SSL_set_tlsext_status_ocsp_resp(
    ssl: *mut SSL,
    resp: *mut c_uchar,
    len: c_long,
) -> c_long {
    SSL_ctrl(
        ssl,
        SSL_CTRL_SET_TLSEXT_STATUS_REQ_OCSP_RESP,
        len,
        resp as *mut c_void,
    )
}

#[deprecated(note = "use SSL_CTX_set_tlsext_servername_callback__fixed_rust instead")]
#[allow(deprecated)]
pub unsafe fn SSL_CTX_set_tlsext_servername_callback(
    ctx: *mut SSL_CTX,
    // FIXME should have the right signature
    cb: Option<extern "C" fn()>,
) -> c_long {
    SSL_CTX_callback_ctrl(ctx, SSL_CTRL_SET_TLSEXT_SERVERNAME_CB, cb)
}

pub unsafe fn SSL_CTX_set_tlsext_servername_callback__fixed_rust(
    ctx: *mut SSL_CTX,
    cb: Option<unsafe extern "C" fn(*mut SSL, *mut c_int, *mut c_void) -> c_int>,
) -> c_long {
    SSL_CTX_callback_ctrl__fixed_rust(
        ctx,
        SSL_CTRL_SET_TLSEXT_SERVERNAME_CB,
        mem::transmute::<
            std::option::Option<
                unsafe extern "C" fn(*mut SSL, *mut c_int, *mut libc::c_void) -> i32,
            >,
            std::option::Option<unsafe extern "C" fn()>,
        >(cb),
    )
}

pub const SSL_TLSEXT_ERR_OK: c_int = 0;
pub const SSL_TLSEXT_ERR_ALERT_WARNING: c_int = 1;
pub const SSL_TLSEXT_ERR_ALERT_FATAL: c_int = 2;
pub const SSL_TLSEXT_ERR_NOACK: c_int = 3;

pub unsafe fn SSL_CTX_set_tlsext_servername_arg(ctx: *mut SSL_CTX, arg: *mut c_void) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_TLSEXT_SERVERNAME_ARG, 0, arg)
}

pub unsafe fn SSL_CTX_set_tlsext_status_cb(
    ctx: *mut SSL_CTX,
    cb: Option<unsafe extern "C" fn(*mut SSL, *mut c_void) -> c_int>,
) -> c_long {
    SSL_CTX_callback_ctrl__fixed_rust(
        ctx,
        SSL_CTRL_SET_TLSEXT_STATUS_REQ_CB,
        mem::transmute::<
            std::option::Option<unsafe extern "C" fn(*mut SSL, *mut c_void) -> i32>,
            std::option::Option<unsafe extern "C" fn()>,
        >(cb),
    )
}

pub unsafe fn SSL_CTX_set_tlsext_status_arg(ctx: *mut SSL_CTX, arg: *mut c_void) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_TLSEXT_STATUS_REQ_CB_ARG, 0, arg)
}
