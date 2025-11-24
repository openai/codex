use libc::*;
use std::ptr;

use super::*;

#[cfg(not(ossl110))]
pub const SSL_MAX_KRB5_PRINCIPAL_LENGTH: c_int = 256;

#[cfg(not(ossl110))]
pub const SSL_MAX_SSL_SESSION_ID_LENGTH: c_int = 32;
#[cfg(not(ossl110))]
pub const SSL_MAX_SID_CTX_LENGTH: c_int = 32;

#[cfg(not(ossl110))]
pub const SSL_MAX_KEY_ARG_LENGTH: c_int = 8;
#[cfg(not(ossl110))]
pub const SSL_MAX_MASTER_KEY_LENGTH: c_int = 48;

pub const SSL_SENT_SHUTDOWN: c_int = 1;
pub const SSL_RECEIVED_SHUTDOWN: c_int = 2;

pub const SSL_FILETYPE_PEM: c_int = X509_FILETYPE_PEM;
pub const SSL_FILETYPE_ASN1: c_int = X509_FILETYPE_ASN1;

#[cfg(ossl111)]
pub const SSL_EXT_TLS_ONLY: c_uint = 0x0001;
/* This extension is only allowed in DTLS */
#[cfg(ossl111)]
pub const SSL_EXT_DTLS_ONLY: c_uint = 0x0002;
/* Some extensions may be allowed in DTLS but we don't implement them for it */
#[cfg(ossl111)]
pub const SSL_EXT_TLS_IMPLEMENTATION_ONLY: c_uint = 0x0004;
/* Most extensions are not defined for SSLv3 but EXT_TYPE_renegotiate is */
#[cfg(ossl111)]
pub const SSL_EXT_SSL3_ALLOWED: c_uint = 0x0008;
/* Extension is only defined for TLS1.2 and below */
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_2_AND_BELOW_ONLY: c_uint = 0x0010;
/* Extension is only defined for TLS1.3 and above */
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_ONLY: c_uint = 0x0020;
/* Ignore this extension during parsing if we are resuming */
#[cfg(ossl111)]
pub const SSL_EXT_IGNORE_ON_RESUMPTION: c_uint = 0x0040;
#[cfg(ossl111)]
pub const SSL_EXT_CLIENT_HELLO: c_uint = 0x0080;
/* Really means TLS1.2 or below */
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_2_SERVER_HELLO: c_uint = 0x0100;
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_SERVER_HELLO: c_uint = 0x0200;
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS: c_uint = 0x0400;
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_HELLO_RETRY_REQUEST: c_uint = 0x0800;
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_CERTIFICATE: c_uint = 0x1000;
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_NEW_SESSION_TICKET: c_uint = 0x2000;
#[cfg(ossl111)]
pub const SSL_EXT_TLS1_3_CERTIFICATE_REQUEST: c_uint = 0x4000;

cfg_if! {
    if #[cfg(ossl300)] {
        macro_rules! ssl_op_type {
            () => {u64};
        }
    } else {
        macro_rules! ssl_op_type {
            () => {c_ulong};
        }
    }
}

pub const SSL_OP_LEGACY_SERVER_CONNECT: ssl_op_type!() = 0x00000004;
cfg_if! {
    if #[cfg(libressl261)] {
        pub const SSL_OP_TLSEXT_PADDING: ssl_op_type!() = 0x0;
    } else if #[cfg(any(ossl102, libressl))] {
        pub const SSL_OP_TLSEXT_PADDING: ssl_op_type!() = 0x10;
    }
}
#[cfg(ossl101)]
pub const SSL_OP_SAFARI_ECDHE_ECDSA_BUG: ssl_op_type!() = 0x00000040;

pub const SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS: ssl_op_type!() = 0x00000800;

pub const SSL_OP_NO_QUERY_MTU: ssl_op_type!() = 0x00001000;
pub const SSL_OP_COOKIE_EXCHANGE: ssl_op_type!() = 0x00002000;
pub const SSL_OP_NO_TICKET: ssl_op_type!() = 0x00004000;
cfg_if! {
    if #[cfg(ossl101)] {
        pub const SSL_OP_CISCO_ANYCONNECT: ssl_op_type!() = 0x00008000;
    } else {
        pub const SSL_OP_CISCO_ANYCONNECT: ssl_op_type!() = 0x0;
    }
}

pub const SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION: ssl_op_type!() = 0x00010000;
cfg_if! {
    if #[cfg(ossl101)] {
        pub const SSL_OP_NO_COMPRESSION: ssl_op_type!() = 0x00020000;
        pub const SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION: ssl_op_type!() = 0x00040000;
    } else {
        pub const SSL_OP_NO_COMPRESSION: ssl_op_type!() = 0x0;
        pub const SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION: ssl_op_type!() = 0x0;
    }
}

#[cfg(ossl111)]
pub const SSL_OP_ENABLE_MIDDLEBOX_COMPAT: ssl_op_type!() = 0x00100000;
#[cfg(ossl111)]
pub const SSL_OP_PRIORITIZE_CHACHA: ssl_op_type!() = 0x00200000;

pub const SSL_OP_CIPHER_SERVER_PREFERENCE: ssl_op_type!() = 0x00400000;
cfg_if! {
    if #[cfg(libressl280)] {
        pub const SSL_OP_TLS_ROLLBACK_BUG: ssl_op_type!() = 0;
    } else {
        pub const SSL_OP_TLS_ROLLBACK_BUG: ssl_op_type!() = 0x00800000;
    }
}

cfg_if! {
    if #[cfg(ossl101)] {
        pub const SSL_OP_NO_SSLv3: ssl_op_type!() = 0x02000000;
    } else {
        pub const SSL_OP_NO_SSLv3: ssl_op_type!() = 0x0;
    }
}
pub const SSL_OP_NO_TLSv1_1: ssl_op_type!() = 0x10000000;
pub const SSL_OP_NO_TLSv1_2: ssl_op_type!() = 0x08000000;

pub const SSL_OP_NO_TLSv1: ssl_op_type!() = 0x04000000;
cfg_if! {
    if #[cfg(ossl102)] {
        pub const SSL_OP_NO_DTLSv1: ssl_op_type!() = 0x04000000;
        pub const SSL_OP_NO_DTLSv1_2: ssl_op_type!() = 0x08000000;
    } else if #[cfg(libressl332)] {
        pub const SSL_OP_NO_DTLSv1: ssl_op_type!() = 0x40000000;
        pub const SSL_OP_NO_DTLSv1_2: ssl_op_type!() = 0x80000000;
    }
}
#[cfg(any(ossl111, libressl340))]
pub const SSL_OP_NO_TLSv1_3: ssl_op_type!() = 0x20000000;

#[cfg(ossl110h)]
pub const SSL_OP_NO_RENEGOTIATION: ssl_op_type!() = 0x40000000;

cfg_if! {
    if #[cfg(ossl111)] {
        pub const SSL_OP_NO_SSL_MASK: ssl_op_type!() = SSL_OP_NO_SSLv2
            | SSL_OP_NO_SSLv3
            | SSL_OP_NO_TLSv1
            | SSL_OP_NO_TLSv1_1
            | SSL_OP_NO_TLSv1_2
            | SSL_OP_NO_TLSv1_3;
    } else if #[cfg(ossl102)] {
        pub const SSL_OP_NO_SSL_MASK: ssl_op_type!() =
            SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3 | SSL_OP_NO_TLSv1 | SSL_OP_NO_TLSv1_1 | SSL_OP_NO_TLSv1_2;
    }
}

cfg_if! {
    if #[cfg(libressl261)] {
        pub const SSL_OP_CRYPTOPRO_TLSEXT_BUG: ssl_op_type!() = 0x0;
    } else {
        pub const SSL_OP_CRYPTOPRO_TLSEXT_BUG: ssl_op_type!() = 0x80000000;
    }
}

cfg_if! {
    if #[cfg(ossl300)] {
        pub const SSL_OP_ALL: ssl_op_type!() = SSL_OP_CRYPTOPRO_TLSEXT_BUG
            | SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS
            | SSL_OP_TLSEXT_PADDING
            | SSL_OP_SAFARI_ECDHE_ECDSA_BUG;
    } else if #[cfg(ossl110f)] {
        pub const SSL_OP_ALL: ssl_op_type!() = SSL_OP_CRYPTOPRO_TLSEXT_BUG
            | SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS
            | SSL_OP_LEGACY_SERVER_CONNECT
            | SSL_OP_TLSEXT_PADDING
            | SSL_OP_SAFARI_ECDHE_ECDSA_BUG;
    } else if #[cfg(libressl261)] {
        pub const SSL_OP_ALL: ssl_op_type!() = 0x4;
    } else if #[cfg(libressl)] {
        pub const SSL_OP_ALL: ssl_op_type!() = 0x80000014;
    } else {
        pub const SSL_OP_ALL: ssl_op_type!() = 0x80000BFF;
    }
}

cfg_if! {
    if #[cfg(ossl110)] {
        pub const SSL_OP_MICROSOFT_SESS_ID_BUG: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_NETSCAPE_CHALLENGE_BUG: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_NETSCAPE_REUSE_CIPHER_CHANGE_BUG: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_MICROSOFT_BIG_SSLV3_BUFFER: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_SSLEAY_080_CLIENT_DH_BUG: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_TLS_D5_BUG: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_TLS_BLOCK_PADDING_BUG: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_SINGLE_ECDH_USE: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_SINGLE_DH_USE: ssl_op_type!() = 0x00000000;
        pub const SSL_OP_NO_SSLv2: ssl_op_type!() = 0x00000000;
    } else if #[cfg(ossl101)] {
        pub const SSL_OP_MICROSOFT_SESS_ID_BUG: ssl_op_type!() = 0x00000001;
        pub const SSL_OP_NETSCAPE_CHALLENGE_BUG: ssl_op_type!() = 0x00000002;
        pub const SSL_OP_NETSCAPE_REUSE_CIPHER_CHANGE_BUG: ssl_op_type!() = 0x00000008;
        pub const SSL_OP_MICROSOFT_BIG_SSLV3_BUFFER: ssl_op_type!() = 0x00000020;
        pub const SSL_OP_SSLEAY_080_CLIENT_DH_BUG: ssl_op_type!() = 0x00000080;
        pub const SSL_OP_TLS_D5_BUG: ssl_op_type!() = 0x00000100;
        pub const SSL_OP_TLS_BLOCK_PADDING_BUG: ssl_op_type!() = 0x00000200;
        pub const SSL_OP_SINGLE_ECDH_USE: ssl_op_type!() = 0x00080000;
        pub const SSL_OP_SINGLE_DH_USE: ssl_op_type!() = 0x00100000;
        pub const SSL_OP_NO_SSLv2: ssl_op_type!() = 0x01000000;
    } else {
        pub const SSL_OP_MICROSOFT_SESS_ID_BUG: ssl_op_type!() = 0x0;
        pub const SSL_OP_NETSCAPE_CHALLENGE_BUG: ssl_op_type!() = 0x0;
        pub const SSL_OP_NETSCAPE_REUSE_CIPHER_CHANGE_BUG: ssl_op_type!() = 0x0;
        pub const SSL_OP_MICROSOFT_BIG_SSLV3_BUFFER: ssl_op_type!() = 0x0;
        pub const SSL_OP_SSLEAY_080_CLIENT_DH_BUG: ssl_op_type!() = 0x0;
        pub const SSL_OP_TLS_D5_BUG: ssl_op_type!() = 0x0;
        pub const SSL_OP_TLS_BLOCK_PADDING_BUG: ssl_op_type!() = 0x0;
        #[cfg(libressl261)]
        pub const SSL_OP_SINGLE_ECDH_USE: ssl_op_type!() = 0x0;
        #[cfg(not(libressl261))]
        pub const SSL_OP_SINGLE_ECDH_USE: ssl_op_type!() = 0x00080000;
        pub const SSL_OP_SINGLE_DH_USE: ssl_op_type!() = 0x00100000;
        pub const SSL_OP_NO_SSLv2: ssl_op_type!() = 0x0;
    }
}

pub const SSL_MODE_ENABLE_PARTIAL_WRITE: c_long = 0x1;
pub const SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER: c_long = 0x2;
pub const SSL_MODE_AUTO_RETRY: c_long = 0x4;
pub const SSL_MODE_NO_AUTO_CHAIN: c_long = 0x8;
pub const SSL_MODE_RELEASE_BUFFERS: c_long = 0x10;
#[cfg(ossl101)]
pub const SSL_MODE_SEND_CLIENTHELLO_TIME: c_long = 0x20;
#[cfg(ossl101)]
pub const SSL_MODE_SEND_SERVERHELLO_TIME: c_long = 0x40;
#[cfg(ossl101)]
pub const SSL_MODE_SEND_FALLBACK_SCSV: c_long = 0x80;

pub unsafe fn SSL_CTX_set_mode(ctx: *mut SSL_CTX, op: c_long) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_MODE, op, ptr::null_mut())
}

#[cfg(ossl111)]
pub const SSL_COOKIE_LENGTH: c_int = 4096;

cfg_if! {
    if #[cfg(not(ossl110))] {
        pub unsafe fn SSL_CTX_get_options(ctx: *const SSL_CTX) -> c_ulong {
            SSL_CTX_ctrl(ctx as *mut _, SSL_CTRL_OPTIONS, 0, ptr::null_mut()) as c_ulong
        }

        pub unsafe fn SSL_CTX_set_options(ctx: *const SSL_CTX, op: c_ulong) -> c_ulong {
            SSL_CTX_ctrl(
                ctx as *mut _,
                SSL_CTRL_OPTIONS,
                op as c_long,
                ptr::null_mut(),
            ) as c_ulong
        }

        pub unsafe fn SSL_CTX_clear_options(ctx: *const SSL_CTX, op: c_ulong) -> c_ulong {
            SSL_CTX_ctrl(
                ctx as *mut _,
                SSL_CTRL_CLEAR_OPTIONS,
                op as c_long,
                ptr::null_mut(),
            ) as c_ulong
        }
    }
}

pub unsafe fn SSL_set_mtu(ssl: *mut SSL, mtu: c_long) -> c_long {
    SSL_ctrl(ssl, SSL_CTRL_SET_MTU, mtu, ptr::null_mut())
}

#[cfg(ossl110)]
pub unsafe fn SSL_get_extms_support(ssl: *mut SSL) -> c_long {
    SSL_ctrl(ssl, SSL_CTRL_GET_EXTMS_SUPPORT, 0, ptr::null_mut())
}

pub const SSL_SESS_CACHE_OFF: c_long = 0x0;
pub const SSL_SESS_CACHE_CLIENT: c_long = 0x1;
pub const SSL_SESS_CACHE_SERVER: c_long = 0x2;
pub const SSL_SESS_CACHE_BOTH: c_long = SSL_SESS_CACHE_CLIENT | SSL_SESS_CACHE_SERVER;
pub const SSL_SESS_CACHE_NO_AUTO_CLEAR: c_long = 0x80;
pub const SSL_SESS_CACHE_NO_INTERNAL_LOOKUP: c_long = 0x100;
pub const SSL_SESS_CACHE_NO_INTERNAL_STORE: c_long = 0x200;
pub const SSL_SESS_CACHE_NO_INTERNAL: c_long =
    SSL_SESS_CACHE_NO_INTERNAL_LOOKUP | SSL_SESS_CACHE_NO_INTERNAL_STORE;

pub const OPENSSL_NPN_UNSUPPORTED: c_int = 0;
pub const OPENSSL_NPN_NEGOTIATED: c_int = 1;
pub const OPENSSL_NPN_NO_OVERLAP: c_int = 2;

pub const SSL_AD_ILLEGAL_PARAMETER: c_int = SSL3_AD_ILLEGAL_PARAMETER;
pub const SSL_AD_DECODE_ERROR: c_int = TLS1_AD_DECODE_ERROR;
pub const SSL_AD_UNRECOGNIZED_NAME: c_int = TLS1_AD_UNRECOGNIZED_NAME;
pub const SSL_ERROR_NONE: c_int = 0;
pub const SSL_ERROR_SSL: c_int = 1;
pub const SSL_ERROR_SYSCALL: c_int = 5;
pub const SSL_ERROR_WANT_ACCEPT: c_int = 8;
pub const SSL_ERROR_WANT_CONNECT: c_int = 7;
pub const SSL_ERROR_WANT_READ: c_int = 2;
pub const SSL_ERROR_WANT_WRITE: c_int = 3;
pub const SSL_ERROR_WANT_X509_LOOKUP: c_int = 4;
pub const SSL_ERROR_ZERO_RETURN: c_int = 6;
#[cfg(ossl111)]
pub const SSL_ERROR_WANT_CLIENT_HELLO_CB: c_int = 11;
pub const SSL_VERIFY_NONE: c_int = 0;
pub const SSL_VERIFY_PEER: c_int = 1;
pub const SSL_VERIFY_FAIL_IF_NO_PEER_CERT: c_int = 2;
pub const SSL_CTRL_SET_TMP_DH: c_int = 3;
pub const SSL_CTRL_SET_TMP_ECDH: c_int = 4;
#[cfg(any(libressl, all(ossl101, not(ossl110))))]
pub const SSL_CTRL_GET_SESSION_REUSED: c_int = 8;
pub const SSL_CTRL_EXTRA_CHAIN_CERT: c_int = 14;
pub const SSL_CTRL_SET_MTU: c_int = 17;
#[cfg(any(libressl, all(ossl101, not(ossl110))))]
pub const SSL_CTRL_OPTIONS: c_int = 32;
pub const SSL_CTRL_MODE: c_int = 33;
pub const SSL_CTRL_SET_READ_AHEAD: c_int = 41;
pub const SSL_CTRL_SET_SESS_CACHE_SIZE: c_int = 42;
pub const SSL_CTRL_GET_SESS_CACHE_SIZE: c_int = 43;
pub const SSL_CTRL_SET_SESS_CACHE_MODE: c_int = 44;
pub const SSL_CTRL_SET_TLSEXT_SERVERNAME_CB: c_int = 53;
pub const SSL_CTRL_SET_TLSEXT_SERVERNAME_ARG: c_int = 54;
pub const SSL_CTRL_SET_TLSEXT_HOSTNAME: c_int = 55;
pub const SSL_CTRL_SET_TLSEXT_STATUS_REQ_CB: c_int = 63;
pub const SSL_CTRL_SET_TLSEXT_STATUS_REQ_CB_ARG: c_int = 64;
pub const SSL_CTRL_SET_TLSEXT_STATUS_REQ_TYPE: c_int = 65;
pub const SSL_CTRL_GET_TLSEXT_STATUS_REQ_OCSP_RESP: c_int = 70;
pub const SSL_CTRL_SET_TLSEXT_STATUS_REQ_OCSP_RESP: c_int = 71;
#[cfg(any(libressl, all(ossl101, not(ossl110))))]
pub const SSL_CTRL_CLEAR_OPTIONS: c_int = 77;
pub const SSL_CTRL_GET_EXTRA_CHAIN_CERTS: c_int = 82;
#[cfg(ossl102)]
pub const SSL_CTRL_CHAIN_CERT: c_int = 89;
#[cfg(any(ossl111, libressl252))]
pub const SSL_CTRL_SET_GROUPS_LIST: c_int = 92;
#[cfg(any(libressl, all(ossl102, not(ossl110))))]
pub const SSL_CTRL_SET_ECDH_AUTO: c_int = 94;
#[cfg(ossl102)]
pub const SSL_CTRL_SET_SIGALGS_LIST: c_int = 98;
#[cfg(ossl102)]
pub const SSL_CTRL_SET_VERIFY_CERT_STORE: c_int = 106;
#[cfg(ossl300)]
pub const SSL_CTRL_GET_PEER_TMP_KEY: c_int = 109;
#[cfg(ossl110)]
pub const SSL_CTRL_GET_EXTMS_SUPPORT: c_int = 122;
#[cfg(any(ossl110, libressl261))]
pub const SSL_CTRL_SET_MIN_PROTO_VERSION: c_int = 123;
#[cfg(any(ossl110, libressl261))]
pub const SSL_CTRL_SET_MAX_PROTO_VERSION: c_int = 124;
#[cfg(any(ossl110g, libressl270))]
pub const SSL_CTRL_GET_MIN_PROTO_VERSION: c_int = 130;
#[cfg(any(ossl110g, libressl270))]
pub const SSL_CTRL_GET_MAX_PROTO_VERSION: c_int = 131;
#[cfg(ossl300)]
pub const SSL_CTRL_GET_TMP_KEY: c_int = 133;

pub unsafe fn SSL_CTX_set_tmp_dh(ctx: *mut SSL_CTX, dh: *mut DH) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_TMP_DH, 0, dh as *mut c_void)
}

pub unsafe fn SSL_CTX_set_tmp_ecdh(ctx: *mut SSL_CTX, key: *mut EC_KEY) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_TMP_ECDH, 0, key as *mut c_void)
}

pub unsafe fn SSL_set_tmp_dh(ssl: *mut SSL, dh: *mut DH) -> c_long {
    SSL_ctrl(ssl, SSL_CTRL_SET_TMP_DH, 0, dh as *mut c_void)
}

pub unsafe fn SSL_set_tmp_ecdh(ssl: *mut SSL, key: *mut EC_KEY) -> c_long {
    SSL_ctrl(ssl, SSL_CTRL_SET_TMP_ECDH, 0, key as *mut c_void)
}

pub unsafe fn SSL_CTX_add_extra_chain_cert(ctx: *mut SSL_CTX, x509: *mut X509) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_EXTRA_CHAIN_CERT, 0, x509 as *mut c_void)
}

pub unsafe fn SSL_CTX_get_extra_chain_certs(
    ctx: *mut SSL_CTX,
    chain: *mut *mut stack_st_X509,
) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_GET_EXTRA_CHAIN_CERTS, 0, chain as *mut c_void)
}

#[cfg(ossl102)]
pub unsafe fn SSL_CTX_set0_verify_cert_store(ctx: *mut SSL_CTX, st: *mut X509_STORE) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_VERIFY_CERT_STORE, 0, st as *mut c_void)
}

#[cfg(ossl102)]
pub unsafe fn SSL_set0_verify_cert_store(ssl: *mut SSL, st: *mut X509_STORE) -> c_long {
    SSL_ctrl(ssl, SSL_CTRL_SET_VERIFY_CERT_STORE, 0, st as *mut c_void)
}

cfg_if! {
    if #[cfg(ossl111)] {
        pub unsafe fn SSL_CTX_set1_groups_list(ctx: *mut SSL_CTX, s: *const c_char) -> c_long {
            SSL_CTX_ctrl(
                ctx,
                SSL_CTRL_SET_GROUPS_LIST,
                0,
                s as *const c_void as *mut c_void,
            )
        }
    } else if #[cfg(libressl251)] {
        extern "C" {
            pub fn SSL_CTX_set1_groups_list(ctx: *mut SSL_CTX, s: *const c_char) -> c_int;
        }
    }
}

#[cfg(ossl102)]
pub unsafe fn SSL_add0_chain_cert(ssl: *mut SSL, ptr: *mut X509) -> c_long {
    SSL_ctrl(ssl, SSL_CTRL_CHAIN_CERT, 0, ptr as *mut c_void)
}

#[cfg(ossl102)]
pub unsafe fn SSL_CTX_set1_sigalgs_list(ctx: *mut SSL_CTX, s: *const c_char) -> c_long {
    SSL_CTX_ctrl(
        ctx,
        SSL_CTRL_SET_SIGALGS_LIST,
        0,
        s as *const c_void as *mut c_void,
    )
}

#[cfg(any(libressl, all(ossl102, not(ossl110))))]
pub unsafe fn SSL_CTX_set_ecdh_auto(ctx: *mut SSL_CTX, onoff: c_int) -> c_int {
    SSL_CTX_ctrl(
        ctx,
        SSL_CTRL_SET_ECDH_AUTO,
        onoff as c_long,
        ptr::null_mut(),
    ) as c_int
}

#[cfg(any(libressl, all(ossl102, not(ossl110))))]
pub unsafe fn SSL_set_ecdh_auto(ssl: *mut SSL, onoff: c_int) -> c_int {
    SSL_ctrl(
        ssl,
        SSL_CTRL_SET_ECDH_AUTO,
        onoff as c_long,
        ptr::null_mut(),
    ) as c_int
}

cfg_if! {
    if #[cfg(ossl110)] {
        pub unsafe fn SSL_CTX_set_min_proto_version(ctx: *mut SSL_CTX, version: c_int) -> c_int {
            SSL_CTX_ctrl(
                ctx,
                SSL_CTRL_SET_MIN_PROTO_VERSION,
                version as c_long,
                ptr::null_mut(),
            ) as c_int
        }

        pub unsafe fn SSL_CTX_set_max_proto_version(ctx: *mut SSL_CTX, version: c_int) -> c_int {
            SSL_CTX_ctrl(
                ctx,
                SSL_CTRL_SET_MAX_PROTO_VERSION,
                version as c_long,
                ptr::null_mut(),
            ) as c_int
        }

        pub unsafe fn SSL_set_min_proto_version(s: *mut SSL, version: c_int) -> c_int {
            SSL_ctrl(
                s,
                SSL_CTRL_SET_MIN_PROTO_VERSION,
                version as c_long,
                ptr::null_mut(),
            ) as c_int
        }

        pub unsafe fn SSL_set_max_proto_version(s: *mut SSL, version: c_int) -> c_int {
            SSL_ctrl(
                s,
                SSL_CTRL_SET_MAX_PROTO_VERSION,
                version as c_long,
                ptr::null_mut(),
            ) as c_int
        }
    }
}

cfg_if! {
    if #[cfg(ossl110g)] {
        pub unsafe fn SSL_CTX_get_min_proto_version(ctx: *mut SSL_CTX) -> c_int {
            SSL_CTX_ctrl(ctx, SSL_CTRL_GET_MIN_PROTO_VERSION, 0, ptr::null_mut()) as c_int
        }

        pub unsafe fn SSL_CTX_get_max_proto_version(ctx: *mut SSL_CTX) -> c_int {
            SSL_CTX_ctrl(ctx, SSL_CTRL_GET_MAX_PROTO_VERSION, 0, ptr::null_mut()) as c_int
        }
        pub unsafe fn SSL_get_min_proto_version(s: *mut SSL) -> c_int {
            SSL_ctrl(s, SSL_CTRL_GET_MIN_PROTO_VERSION, 0, ptr::null_mut()) as c_int
        }
        pub unsafe fn SSL_get_max_proto_version(s: *mut SSL) -> c_int {
            SSL_ctrl(s, SSL_CTRL_GET_MAX_PROTO_VERSION, 0, ptr::null_mut()) as c_int
        }
    }
}
cfg_if! {
    if #[cfg(ossl300)] {
        pub unsafe fn SSL_get_peer_tmp_key(ssl: *mut SSL, key: *mut *mut EVP_PKEY) -> c_long {
            SSL_ctrl(ssl, SSL_CTRL_GET_PEER_TMP_KEY, 0, key as *mut c_void)
        }

        pub unsafe fn SSL_get_tmp_key(ssl: *mut SSL, key: *mut *mut EVP_PKEY) -> c_long {
            SSL_ctrl(ssl, SSL_CTRL_GET_TMP_KEY, 0, key as *mut c_void)
        }
    }
}

#[cfg(ossl111)]
pub const SSL_CLIENT_HELLO_SUCCESS: c_int = 1;
#[cfg(ossl111)]
pub const SSL_CLIENT_HELLO_ERROR: c_int = 0;
#[cfg(ossl111)]
pub const SSL_CLIENT_HELLO_RETRY: c_int = -1;

#[cfg(any(ossl111, libressl340))]
pub const SSL_READ_EARLY_DATA_ERROR: c_int = 0;
#[cfg(any(ossl111, libressl340))]
pub const SSL_READ_EARLY_DATA_SUCCESS: c_int = 1;
#[cfg(any(ossl111, libressl340))]
pub const SSL_READ_EARLY_DATA_FINISH: c_int = 2;

cfg_if! {
    if #[cfg(ossl110)] {
        pub unsafe fn SSL_get_ex_new_index(
            l: c_long,
            p: *mut c_void,
            newf: Option<CRYPTO_EX_new>,
            dupf: Option<CRYPTO_EX_dup>,
            freef: Option<CRYPTO_EX_free>,
        ) -> c_int {
            CRYPTO_get_ex_new_index(CRYPTO_EX_INDEX_SSL, l, p, newf, dupf, freef)
        }
    }
}
cfg_if! {
    if #[cfg(ossl110)] {
        pub unsafe fn SSL_CTX_get_ex_new_index(
            l: c_long,
            p: *mut c_void,
            newf: Option<CRYPTO_EX_new>,
            dupf: Option<CRYPTO_EX_dup>,
            freef: Option<CRYPTO_EX_free>,
        ) -> c_int {
            CRYPTO_get_ex_new_index(CRYPTO_EX_INDEX_SSL_CTX, l, p, newf, dupf, freef)
        }
    }
}

pub unsafe fn SSL_CTX_sess_set_cache_size(ctx: *mut SSL_CTX, t: c_long) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_SESS_CACHE_SIZE, t, ptr::null_mut())
}

pub unsafe fn SSL_CTX_sess_get_cache_size(ctx: *mut SSL_CTX) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_GET_SESS_CACHE_SIZE, 0, ptr::null_mut())
}

pub unsafe fn SSL_CTX_set_session_cache_mode(ctx: *mut SSL_CTX, m: c_long) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_SESS_CACHE_MODE, m, ptr::null_mut())
}

pub unsafe fn SSL_CTX_set_read_ahead(ctx: *mut SSL_CTX, m: c_long) -> c_long {
    SSL_CTX_ctrl(ctx, SSL_CTRL_SET_READ_AHEAD, m, ptr::null_mut())
}

#[allow(clashing_extern_declarations)]
extern "C" {
    #[deprecated(note = "use SSL_CTX_set_tmp_dh_callback__fixed_rust instead")]
    pub fn SSL_CTX_set_tmp_dh_callback(
        ctx: *mut SSL_CTX,
        dh: unsafe extern "C" fn(ssl: *mut SSL, is_export: c_int, keylength: c_int) -> *mut DH,
    );
    #[deprecated(note = "use SSL_set_tmp_dh_callback__fixed_rust instead")]
    pub fn SSL_set_tmp_dh_callback(
        ctx: *mut SSL,
        dh: unsafe extern "C" fn(ssl: *mut SSL, is_export: c_int, keylength: c_int) -> *mut DH,
    );
    #[deprecated(note = "use SSL_CTX_set_tmp_ecdh_callback__fixed_rust instead")]
    #[cfg(not(ossl110))]
    pub fn SSL_CTX_set_tmp_ecdh_callback(
        ctx: *mut SSL_CTX,
        ecdh: unsafe extern "C" fn(
            ssl: *mut SSL,
            is_export: c_int,
            keylength: c_int,
        ) -> *mut EC_KEY,
    );
    #[deprecated(note = "use SSL_set_tmp_ecdh_callback__fixed_rust instead")]
    #[cfg(not(ossl110))]
    pub fn SSL_set_tmp_ecdh_callback(
        ssl: *mut SSL,
        ecdh: unsafe extern "C" fn(
            ssl: *mut SSL,
            is_export: c_int,
            keylength: c_int,
        ) -> *mut EC_KEY,
    );

    #[deprecated(note = "use SSL_CTX_callback_ctrl__fixed_rust instead")]
    pub fn SSL_CTX_callback_ctrl(
        ctx: *mut SSL_CTX,
        cmd: c_int,
        fp: Option<extern "C" fn()>,
    ) -> c_long;

    #[deprecated(note = "use SSL_CTX_set_alpn_select_cb__fixed_rust instead")]
    #[cfg(any(ossl102, libressl261))]
    pub fn SSL_CTX_set_alpn_select_cb(
        ssl: *mut SSL_CTX,
        cb: extern "C" fn(
            ssl: *mut SSL,
            out: *mut *const c_uchar,
            outlen: *mut c_uchar,
            inbuf: *const c_uchar,
            inlen: c_uint,
            arg: *mut c_void,
        ) -> c_int,
        arg: *mut c_void,
    );
}

#[cfg(not(ossl110))]
pub unsafe fn SSL_session_reused(ssl: *mut SSL) -> c_int {
    SSL_ctrl(ssl, SSL_CTRL_GET_SESSION_REUSED, 0, ptr::null_mut()) as c_int
}

#[cfg(ossl110)]
pub const OPENSSL_INIT_LOAD_SSL_STRINGS: u64 = 0x00200000;
#[cfg(ossl111b)]
pub const OPENSSL_INIT_NO_ATEXIT: u64 = 0x00080000;

cfg_if! {
    if #[cfg(ossl330)] {
        pub const SSL_VALUE_CLASS_GENERIC: c_uint = 0;
        pub const SSL_VALUE_CLASS_FEATURE_REQUEST: c_uint = 1;
        pub const SSL_VALUE_CLASS_FEATURE_PEER_REQUEST: c_uint = 2;
        pub const SSL_VALUE_CLASS_FEATURE_NEGOTIATED: c_uint = 3;

        pub const SSL_VALUE_NONE: c_uint = 0;
        pub const SSL_VALUE_QUIC_STREAM_BIDI_LOCAL_AVAIL: c_uint = 1;
        pub const SSL_VALUE_QUIC_STREAM_BIDI_REMOTE_AVAIL: c_uint = 2;
        pub const SSL_VALUE_QUIC_STREAM_UNI_LOCAL_AVAIL: c_uint = 3;
        pub const SSL_VALUE_QUIC_STREAM_UNI_REMOTE_AVAIL: c_uint = 4;
        pub const SSL_VALUE_QUIC_IDLE_TIMEOUT: c_uint = 5;
        pub const SSL_VALUE_EVENT_HANDLING_MODE: c_uint = 6;
        pub const SSL_VALUE_STREAM_WRITE_BUF_SIZE: c_uint = 7;
        pub const SSL_VALUE_STREAM_WRITE_BUF_USED: c_uint = 8;
        pub const SSL_VALUE_STREAM_WRITE_BUF_AVAIL: c_uint = 9;

        pub const SSL_VALUE_EVENT_HANDLING_MODE_INHERIT: c_uint = 0;
        pub const SSL_VALUE_EVENT_HANDLING_MODE_IMPLICIT: c_uint = 1;
        pub const SSL_VALUE_EVENT_HANDLING_MODE_EXPLICIT: c_uint = 2;

        pub unsafe fn SSL_get_generic_value_uint(ssl: *mut SSL, id: u32, value: *mut u64) -> c_int {
            SSL_get_value_uint(ssl, SSL_VALUE_CLASS_GENERIC, id, value)
        }
        pub unsafe fn SSL_set_generic_value_uint(ssl: *mut SSL, id: u32, value: u64) -> c_int {
            SSL_set_value_uint(ssl, SSL_VALUE_CLASS_GENERIC, id, value)
        }
        pub unsafe fn SSL_get_feature_request_uint(ssl: *mut SSL, id: u32, value: *mut u64) -> c_int {
            SSL_get_value_uint(ssl, SSL_VALUE_CLASS_FEATURE_REQUEST, id, value)
        }
        pub unsafe fn SSL_set_feature_request_uint(ssl: *mut SSL, id: u32, value: u64) -> c_int {
            SSL_set_value_uint(ssl, SSL_VALUE_CLASS_FEATURE_REQUEST, id, value)
        }
        pub unsafe fn SSL_get_feature_peer_request_uint(ssl: *mut SSL, id: u32, value: *mut u64) -> c_int {
            SSL_get_value_uint(ssl, SSL_VALUE_CLASS_FEATURE_PEER_REQUEST, id, value)
        }
        pub unsafe fn SSL_get_feature_negotiated_uint(ssl: *mut SSL, id: u32, value: *mut u64) -> c_int {
            SSL_get_value_uint(ssl, SSL_VALUE_CLASS_FEATURE_NEGOTIATED, id, value)
        }
        pub unsafe fn SSL_get_quic_stream_bidi_local_avail(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_QUIC_STREAM_BIDI_LOCAL_AVAIL, value)
        }
        pub unsafe fn SSL_get_quic_stream_bidi_remote_avail(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_QUIC_STREAM_BIDI_REMOTE_AVAIL, value)
        }
        pub unsafe fn SSL_get_quic_stream_uni_local_avail(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_QUIC_STREAM_UNI_LOCAL_AVAIL, value)
        }
        pub unsafe fn SSL_get_quic_stream_uni_remote_avail(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_QUIC_STREAM_UNI_REMOTE_AVAIL, value)
        }
        pub unsafe fn SSL_get_event_handling_mode(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_EVENT_HANDLING_MODE, value)
        }
        pub unsafe fn SSL_set_event_handling_mode(ssl: *mut SSL, value: u64) -> c_int {
            SSL_set_generic_value_uint(ssl, SSL_VALUE_EVENT_HANDLING_MODE, value)
        }
        pub unsafe fn SSL_get_stream_write_buf_size(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_STREAM_WRITE_BUF_SIZE, value)
        }
        pub unsafe fn SSL_get_stream_write_buf_avail(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_STREAM_WRITE_BUF_AVAIL, value)
        }
        pub unsafe fn SSL_get_stream_write_buf_used(ssl: *mut SSL, value: *mut u64) -> c_int {
            SSL_get_generic_value_uint(ssl, SSL_VALUE_STREAM_WRITE_BUF_USED, value)
        }
    }
}
