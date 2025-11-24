use libc::*;

#[allow(unused_imports)]
use super::super::*;

pub enum ASN1_OBJECT {}
pub enum ASN1_VALUE {}

pub type ASN1_BOOLEAN = c_int;
pub enum ASN1_INTEGER {}
pub enum ASN1_ENUMERATED {}
pub enum ASN1_GENERALIZEDTIME {}
pub enum ASN1_STRING {}
pub enum ASN1_BIT_STRING {}
pub enum ASN1_TIME {}
pub enum ASN1_OCTET_STRING {}
pub enum ASN1_NULL {}
pub enum ASN1_PRINTABLESTRING {}
pub enum ASN1_T61STRING {}
pub enum ASN1_IA5STRING {}
pub enum ASN1_GENERALSTRING {}
pub enum ASN1_BMPSTRING {}
pub enum ASN1_UNIVERSALSTRING {}
pub enum ASN1_UTCTIME {}
pub enum ASN1_VISIBLESTRING {}
pub enum ASN1_UTF8STRING {}

pub enum bio_st {} // FIXME remove
cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum BIO {}
    } else {
        #[repr(C)]
        pub struct BIO {
            pub method: *mut BIO_METHOD,
            pub callback: Option<
                unsafe extern "C" fn(*mut BIO, c_int, *const c_char, c_int, c_long, c_long) -> c_long,
            >,
            pub cb_arg: *mut c_char,
            pub init: c_int,
            pub shutdown: c_int,
            pub flags: c_int,
            pub retry_reason: c_int,
            pub num: c_int,
            pub ptr: *mut c_void,
            pub next_bio: *mut BIO,
            pub prev_bio: *mut BIO,
            pub references: c_int,
            pub num_read: c_ulong,
            pub num_write: c_ulong,
            pub ex_data: CRYPTO_EX_DATA,
        }
    }
}
cfg_if! {
    if #[cfg(ossl320)] {
        pub enum BIO_ADDR {}
        pub enum BIO_POLL_DESCRIPTOR {}
        #[repr(C)]
        pub struct BIO_MSG {
            pub data: *mut c_void,
            pub data_len: usize,
            pub peer: *mut BIO_ADDR,
            pub local: *mut BIO_ADDR,
            pub flags: u64,
        }
    }
}
cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum BIGNUM {}
    } else {
        #[repr(C)]
        pub struct BIGNUM {
            pub d: *mut BN_ULONG,
            pub top: c_int,
            pub dmax: c_int,
            pub neg: c_int,
            pub flags: c_int,
        }
    }
}
pub enum BN_BLINDING {}
pub enum BN_MONT_CTX {}

pub enum BN_CTX {}
pub enum BN_GENCB {}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum EVP_CIPHER {}
    } else {
        #[repr(C)]
        pub struct EVP_CIPHER {
            pub nid: c_int,
            pub block_size: c_int,
            pub key_len: c_int,
            pub iv_len: c_int,
            pub flags: c_ulong,
            pub init: Option<
                unsafe extern "C" fn(*mut EVP_CIPHER_CTX, *const c_uchar, *const c_uchar, c_int) -> c_int,
            >,
            pub do_cipher: Option<
                unsafe extern "C" fn(*mut EVP_CIPHER_CTX, *mut c_uchar, *const c_uchar, size_t) -> c_int,
            >,
            pub cleanup: Option<unsafe extern "C" fn(*mut EVP_CIPHER_CTX) -> c_int>,
            pub ctx_size: c_int,
            pub set_asn1_parameters:
                Option<unsafe extern "C" fn(*mut EVP_CIPHER_CTX, *mut ASN1_TYPE) -> c_int>,
            pub get_asn1_parameters:
                Option<unsafe extern "C" fn(*mut EVP_CIPHER_CTX, *mut ASN1_TYPE) -> c_int>,
            pub ctrl:
                Option<unsafe extern "C" fn(*mut EVP_CIPHER_CTX, c_int, c_int, *mut c_void) -> c_int>,
            pub app_data: *mut c_void,
        }
    }
}
pub enum EVP_CIPHER_CTX {}
pub enum EVP_MD {}
cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum EVP_MD_CTX {}
    } else {
        #[repr(C)]
        pub struct EVP_MD_CTX {
            digest: *mut EVP_MD,
            engine: *mut ENGINE,
            flags: c_ulong,
            md_data: *mut c_void,
            pctx: *mut EVP_PKEY_CTX,
            update: *mut c_void,
        }
    }
}

pub enum PKCS8_PRIV_KEY_INFO {}

pub enum EVP_PKEY_ASN1_METHOD {}

pub enum EVP_PKEY_CTX {}

pub enum CMAC_CTX {}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum HMAC_CTX {}
    } else {
        #[repr(C)]
        pub struct HMAC_CTX {
            md: *mut EVP_MD,
            md_ctx: EVP_MD_CTX,
            i_ctx: EVP_MD_CTX,
            o_ctx: EVP_MD_CTX,
            key_length: c_uint,
            key: [c_uchar; 128],
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum DH {}
    } else {
        #[repr(C)]
        pub struct DH {
            pub pad: c_int,
            pub version: c_int,
            pub p: *mut BIGNUM,
            pub g: *mut BIGNUM,
            pub length: c_long,
            pub pub_key: *mut BIGNUM,
            pub priv_key: *mut BIGNUM,
            pub flags: c_int,
            pub method_mont_p: *mut BN_MONT_CTX,
            pub q: *mut BIGNUM,
            pub j: *mut BIGNUM,
            pub seed: *mut c_uchar,
            pub seedlen: c_int,
            pub counter: *mut BIGNUM,
            pub references: c_int,
            pub ex_data: CRYPTO_EX_DATA,
            pub meth: *const DH_METHOD,
            pub engine: *mut ENGINE,
        }
    }
}
pub enum DH_METHOD {}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum DSA {}
    } else {
        #[repr(C)]
        pub struct DSA {
            pub pad: c_int,
            pub version: c_long,
            pub write_params: c_int,

            pub p: *mut BIGNUM,
            pub q: *mut BIGNUM,
            pub g: *mut BIGNUM,
            pub pub_key: *mut BIGNUM,
            pub priv_key: *mut BIGNUM,
            pub kinv: *mut BIGNUM,
            pub r: *mut BIGNUM,

            pub flags: c_int,
            pub method_mont_p: *mut BN_MONT_CTX,
            pub references: c_int,
            pub ex_data: CRYPTO_EX_DATA,
            pub meth: *const DSA_METHOD,
            pub engine: *mut ENGINE,
        }
    }
}
pub enum DSA_METHOD {}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum RSA {}
    } else if #[cfg(libressl)] {
        #[repr(C)]
        pub struct RSA {
            pub pad: c_int,
            pub version: c_long,
            pub meth: *const RSA_METHOD,

            pub engine: *mut ENGINE,
            pub n: *mut BIGNUM,
            pub e: *mut BIGNUM,
            pub d: *mut BIGNUM,
            pub p: *mut BIGNUM,
            pub q: *mut BIGNUM,
            pub dmp1: *mut BIGNUM,
            pub dmq1: *mut BIGNUM,
            pub iqmp: *mut BIGNUM,

            pub ex_data: CRYPTO_EX_DATA,
            pub references: c_int,
            pub flags: c_int,

            pub _method_mod_n: *mut BN_MONT_CTX,
            pub _method_mod_p: *mut BN_MONT_CTX,
            pub _method_mod_q: *mut BN_MONT_CTX,

            pub blinding: *mut BN_BLINDING,
            pub mt_blinding: *mut BN_BLINDING,
        }
    } else {
        #[repr(C)]
        pub struct RSA {
            pub pad: c_int,
            pub version: c_long,
            pub meth: *const RSA_METHOD,

            pub engine: *mut ENGINE,
            pub n: *mut BIGNUM,
            pub e: *mut BIGNUM,
            pub d: *mut BIGNUM,
            pub p: *mut BIGNUM,
            pub q: *mut BIGNUM,
            pub dmp1: *mut BIGNUM,
            pub dmq1: *mut BIGNUM,
            pub iqmp: *mut BIGNUM,

            pub ex_data: CRYPTO_EX_DATA,
            pub references: c_int,
            pub flags: c_int,

            pub _method_mod_n: *mut BN_MONT_CTX,
            pub _method_mod_p: *mut BN_MONT_CTX,
            pub _method_mod_q: *mut BN_MONT_CTX,

            pub bignum_data: *mut c_char,
            pub blinding: *mut BN_BLINDING,
            pub mt_blinding: *mut BN_BLINDING,
        }
    }
}
pub enum RSA_METHOD {}

pub enum EC_KEY {}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum X509 {}
    } else if #[cfg(libressl)] {
        #[repr(C)]
        pub struct X509 {
            pub cert_info: *mut X509_CINF,
            pub sig_alg: *mut X509_ALGOR,
            pub signature: *mut ASN1_BIT_STRING,
            pub valid: c_int,
            pub references: c_int,
            pub name: *mut c_char,
            pub ex_data: CRYPTO_EX_DATA,
            pub ex_pathlen: c_long,
            pub ex_pcpathlen: c_long,
            pub ex_flags: c_ulong,
            pub ex_kusage: c_ulong,
            pub ex_xkusage: c_ulong,
            pub ex_nscert: c_ulong,
            skid: *mut c_void,
            akid: *mut c_void,
            policy_cache: *mut c_void,
            crldp: *mut c_void,
            altname: *mut c_void,
            nc: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_SHA"))]
            sha1_hash: [c_uchar; 20],
            aux: *mut c_void,
        }
    } else {
        #[repr(C)]
        pub struct X509 {
            pub cert_info: *mut X509_CINF,
            pub sig_alg: *mut X509_ALGOR,
            pub signature: *mut ASN1_BIT_STRING,
            pub valid: c_int,
            pub references: c_int,
            pub name: *mut c_char,
            pub ex_data: CRYPTO_EX_DATA,
            pub ex_pathlen: c_long,
            pub ex_pcpathlen: c_long,
            pub ex_flags: c_ulong,
            pub ex_kusage: c_ulong,
            pub ex_xkusage: c_ulong,
            pub ex_nscert: c_ulong,
            skid: *mut c_void,
            akid: *mut c_void,
            policy_cache: *mut c_void,
            crldp: *mut c_void,
            altname: *mut c_void,
            nc: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_RFC3779"))]
            rfc3779_addr: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_RFC3779"))]
            rfc3779_asid: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_SHA"))]
            sha1_hash: [c_uchar; 20],
            aux: *mut c_void,
        }
    }
}
cfg_if! {
    if #[cfg(any(ossl110, libressl382))] {
        pub enum X509_ALGOR {}
    } else {
        #[repr(C)]
        pub struct X509_ALGOR {
            pub algorithm: *mut ASN1_OBJECT,
            parameter: *mut c_void,
        }
    }
}

stack!(stack_st_X509_ALGOR);

pub enum X509_LOOKUP_METHOD {}

pub enum X509_NAME {}

cfg_if! {
    if #[cfg(any(ossl110, libressl270))] {
        pub enum X509_STORE {}
    } else {
        #[repr(C)]
        pub struct X509_STORE {
            cache: c_int,
            pub objs: *mut stack_st_X509_OBJECT,
            get_cert_methods: *mut stack_st_X509_LOOKUP,
            param: *mut X509_VERIFY_PARAM,
            verify: Option<extern "C" fn(ctx: *mut X509_STORE_CTX) -> c_int>,
            verify_cb: Option<extern "C" fn(ok: c_int, ctx: *mut X509_STORE_CTX) -> c_int>,
            get_issuer: Option<
                extern "C" fn(issuer: *mut *mut X509, ctx: *mut X509_STORE_CTX, x: *mut X509) -> c_int,
            >,
            check_issued:
                Option<extern "C" fn(ctx: *mut X509_STORE_CTX, x: *mut X509, issuer: *mut X509) -> c_int>,
            check_revocation: Option<extern "C" fn(ctx: *mut X509_STORE_CTX) -> c_int>,
            get_crl: Option<
                extern "C" fn(ctx: *mut X509_STORE_CTX, crl: *mut *mut X509_CRL, x: *mut X509) -> c_int,
            >,
            check_crl: Option<extern "C" fn(ctx: *mut X509_STORE_CTX, crl: *mut X509_CRL) -> c_int>,
            cert_crl:
                Option<extern "C" fn(ctx: *mut X509_STORE_CTX, crl: *mut X509_CRL, x: *mut X509) -> c_int>,
            lookup_certs:
                Option<extern "C" fn(ctx: *mut X509_STORE_CTX, nm: *const X509_NAME) -> *mut stack_st_X509>,
            lookup_crls: Option<
                extern "C" fn(ctx: *const X509_STORE_CTX, nm: *const X509_NAME) -> *mut stack_st_X509_CRL,
            >,
            cleanup: Option<extern "C" fn(ctx: *mut X509_STORE_CTX) -> c_int>,
            ex_data: CRYPTO_EX_DATA,
            references: c_int,
        }
    }
}

pub enum X509_STORE_CTX {}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum X509_VERIFY_PARAM {}
    } else if #[cfg(libressl251)] {
        #[repr(C)]
        pub struct X509_VERIFY_PARAM {
            pub name: *mut c_char,
            pub check_time: time_t,
            pub inh_flags: c_ulong,
            pub flags: c_ulong,
            pub purpose: c_int,
            pub trust: c_int,
            pub depth: c_int,
            pub policies: *mut stack_st_ASN1_OBJECT,
            id: *mut c_void,
        }
    } else if #[cfg(libressl)] {
        #[repr(C)]
        pub struct X509_VERIFY_PARAM {
            pub name: *mut c_char,
            pub check_time: time_t,
            pub inh_flags: c_ulong,
            pub flags: c_ulong,
            pub purpose: c_int,
            pub trust: c_int,
            pub depth: c_int,
            pub policies: *mut stack_st_ASN1_OBJECT,
            //pub id: *mut X509_VERIFY_PARAM_ID,
        }
    } else {
        #[repr(C)]
        pub struct X509_VERIFY_PARAM {
            pub name: *mut c_char,
            pub check_time: time_t,
            pub inh_flags: c_ulong,
            pub flags: c_ulong,
            pub purpose: c_int,
            pub trust: c_int,
            pub depth: c_int,
            pub policies: *mut stack_st_ASN1_OBJECT,
            #[cfg(ossl102)]
            pub id: *mut X509_VERIFY_PARAM_ID,
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl270))] {
        pub enum X509_OBJECT {}
    } else {
        #[repr(C)]
        pub struct X509_OBJECT {
            pub type_: c_int,
            pub data: X509_OBJECT_data,
        }
        #[repr(C)]
        pub union X509_OBJECT_data {
            pub ptr: *mut c_char,
            pub x509: *mut X509,
            pub crl: *mut X509_CRL,
            pub pkey: *mut EVP_PKEY,
        }
    }
}

pub enum X509_LOOKUP {}

#[repr(C)]
pub struct X509V3_CTX {
    flags: c_int,
    issuer_cert: *mut c_void,
    subject_cert: *mut c_void,
    subject_req: *mut c_void,
    crl: *mut c_void,
    #[cfg(not(libressl400))]
    db_meth: *mut c_void,
    db: *mut c_void,
    #[cfg(ossl300)]
    issuer_pkey: *mut c_void,
    // I like the last comment line, it is copied from OpenSSL sources:
    // Maybe more here
}
pub enum CONF {}
#[cfg(ossl110)]
pub enum OPENSSL_INIT_SETTINGS {}

pub enum ENGINE {}
cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum SSL {}
    } else if #[cfg(libressl251)] {
        #[repr(C)]
        pub struct SSL {
            version: c_int,
            method: *const SSL_METHOD,
            rbio: *mut BIO,
            wbio: *mut BIO,
            bbio: *mut BIO,
            pub server: c_int,
            s3: *mut c_void,
            d1: *mut c_void,
            param: *mut c_void,
            cipher_list: *mut stack_st_SSL_CIPHER,
            cert: *mut c_void,
            sid_ctx_length: c_uint,
            sid_ctx: [c_uchar; SSL_MAX_SID_CTX_LENGTH as usize],
            session: *mut SSL_SESSION,
            verify_mode: c_int,
            error: c_int,
            error_code: c_int,
            ctx: *mut SSL_CTX,
            verify_result: c_long,
            references: c_int,
            client_version: c_int,
            max_send_fragment: c_uint,
            tlsext_hostname: *mut c_char,
            tlsext_status_type: c_int,
            initial_ctx: *mut SSL_CTX,
            enc_read_ctx: *mut EVP_CIPHER_CTX,
            read_hash: *mut EVP_MD_CTX,
            internal: *mut c_void,
        }
    } else if #[cfg(libressl)] {
        #[repr(C)]
        pub struct SSL {
            version: c_int,
            type_: c_int,
            method: *const SSL_METHOD,
            rbio: *mut c_void,
            wbio: *mut c_void,
            bbio: *mut c_void,
            rwstate: c_int,
            in_handshake: c_int,
            handshake_func: Option<unsafe extern "C" fn(*mut SSL) -> c_int>,
            pub server: c_int,
            new_session: c_int,
            quiet_shutdown: c_int,
            shutdown: c_int,
            state: c_int,
            rstate: c_int,
            init_buf: *mut c_void,
            init_msg: *mut c_void,
            init_num: c_int,
            init_off: c_int,
            packet: *mut c_uchar,
            packet_length: c_uint,
            s3: *mut c_void,
            d1: *mut c_void,
            read_ahead: c_int,
            msg_callback: Option<
                unsafe extern "C" fn(c_int,
                                    c_int,
                                    c_int,
                                    *const c_void,
                                    size_t,
                                    *mut SSL,
                                    *mut c_void),
            >,
            msg_callback_arg: *mut c_void,
            hit: c_int,
            param: *mut c_void,
            cipher_list: *mut stack_st_SSL_CIPHER,
            cipher_list_by_id: *mut stack_st_SSL_CIPHER,
            mac_flags: c_int,
            aead_read_ctx: *mut c_void,
            enc_read_ctx: *mut EVP_CIPHER_CTX,
            read_hash: *mut EVP_MD_CTX,
            aead_write_ctx: *mut c_void,
            enc_write_ctx: *mut EVP_CIPHER_CTX,
            write_hash: *mut EVP_MD_CTX,
            cert: *mut c_void,
            sid_ctx_length: c_uint,
            sid_ctx: [c_uchar; SSL_MAX_SID_CTX_LENGTH as usize],
            session: *mut SSL_SESSION,
            generate_session_id: GEN_SESSION_CB,
            verify_mode: c_int,
            verify_callback: Option<unsafe extern "C" fn(c_int, *mut X509_STORE_CTX) -> c_int>,
            info_callback: Option<unsafe extern "C" fn(*mut SSL, c_int, c_int)>,
            error: c_int,
            error_code: c_int,
            ctx: *mut SSL_CTX,
            debug: c_int,
            verify_result: c_long,
            ex_data: CRYPTO_EX_DATA,
            client_CA: *mut stack_st_X509_NAME,
            references: c_int,
            options: c_ulong,
            mode: c_ulong,
            max_cert_list: c_long,
            first_packet: c_int,
            client_version: c_int,
            max_send_fragment: c_uint,
            tlsext_debug_cb:
                Option<unsafe extern "C" fn(*mut SSL, c_int, c_int, *mut c_uchar, c_int, *mut c_void)>,
            tlsext_debug_arg: *mut c_void,
            tlsext_hostname: *mut c_char,
            servername_done: c_int,
            tlsext_status_type: c_int,
            tlsext_status_expected: c_int,
            tlsext_ocsp_ids: *mut c_void,
            tlsext_ocsp_exts: *mut c_void,
            tlsext_ocsp_resp: *mut c_uchar,
            tlsext_ocsp_resplen: c_int,
            tlsext_ticket_expected: c_int,
            tlsext_ecpointformatlist_length: size_t,
            tlsext_ecpointformatlist: *mut c_uchar,
            tlsext_ellipticcurvelist_length: size_t,
            tlsext_ellipticcurvelist: *mut c_uchar,
            tlsext_session_ticket: *mut c_void,
            tlsext_session_ticket_ext_cb: tls_session_ticket_ext_cb_fn,
            tls_session_ticket_ext_cb_arg: *mut c_void,
            tls_session_secret_cb: tls_session_secret_cb_fn,
            tls_session_secret_cb_arg: *mut c_void,
            initial_ctx: *mut SSL_CTX,
            next_proto_negotiated: *mut c_uchar,
            next_proto_negotiated_len: c_uchar,
            srtp_profiles: *mut c_void,
            srtp_profile: *mut c_void,
            tlsext_heartbeat: c_uint,
            tlsext_hb_pending: c_uint,
            tlsext_hb_seq: c_uint,
            alpn_client_proto_list: *mut c_uchar,
            alpn_client_proto_list_len: c_uint,
            renegotiate: c_int,
        }
    } else {
        #[repr(C)]
        pub struct SSL {
            version: c_int,
            type_: c_int,
            method: *const SSL_METHOD,
            rbio: *mut c_void,
            wbio: *mut c_void,
            bbio: *mut c_void,
            rwstate: c_int,
            in_handshake: c_int,
            handshake_func: Option<unsafe extern "C" fn(*mut SSL) -> c_int>,
            pub server: c_int,
            new_session: c_int,
            quiet_session: c_int,
            shutdown: c_int,
            state: c_int,
            rstate: c_int,
            init_buf: *mut c_void,
            init_msg: *mut c_void,
            init_num: c_int,
            init_off: c_int,
            packet: *mut c_uchar,
            packet_length: c_uint,
            s2: *mut c_void,
            s3: *mut c_void,
            d1: *mut c_void,
            read_ahead: c_int,
            msg_callback: Option<
                unsafe extern "C" fn(c_int, c_int, c_int, *const c_void, size_t, *mut SSL, *mut c_void),
            >,
            msg_callback_arg: *mut c_void,
            hit: c_int,
            param: *mut c_void,
            cipher_list: *mut stack_st_SSL_CIPHER,
            cipher_list_by_id: *mut stack_st_SSL_CIPHER,
            mac_flags: c_int,
            enc_read_ctx: *mut EVP_CIPHER_CTX,
            read_hash: *mut EVP_MD_CTX,
            expand: *mut c_void,
            enc_write_ctx: *mut EVP_CIPHER_CTX,
            write_hash: *mut EVP_MD_CTX,
            compress: *mut c_void,
            cert: *mut c_void,
            sid_ctx_length: c_uint,
            sid_ctx: [c_uchar; SSL_MAX_SID_CTX_LENGTH as usize],
            session: *mut SSL_SESSION,
            generate_session_id: GEN_SESSION_CB,
            verify_mode: c_int,
            verify_callback: Option<unsafe extern "C" fn(c_int, *mut X509_STORE_CTX) -> c_int>,
            info_callback: Option<unsafe extern "C" fn(*mut SSL, c_int, c_int)>,
            error: c_int,
            error_code: c_int,
            #[cfg(not(osslconf = "OPENSSL_NO_KRB5"))]
            kssl_ctx: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
            psk_client_callback: Option<
                unsafe extern "C" fn(*mut SSL, *const c_char, *mut c_char, c_uint, *mut c_uchar, c_uint)
                    -> c_uint,
            >,
            #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
            psk_server_callback:
                Option<unsafe extern "C" fn(*mut SSL, *const c_char, *mut c_uchar, c_uint) -> c_uint>,
            ctx: *mut SSL_CTX,
            debug: c_int,
            verify_result: c_long,
            ex_data: CRYPTO_EX_DATA,
            client_CA: *mut stack_st_X509_NAME,
            references: c_int,
            options: c_ulong,
            mode: c_ulong,
            max_cert_list: c_long,
            first_packet: c_int,
            client_version: c_int,
            max_send_fragment: c_uint,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_debug_cb:
                Option<unsafe extern "C" fn(*mut SSL, c_int, c_int, *mut c_uchar, c_int, *mut c_void)>,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_debug_arg: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_hostname: *mut c_char,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            servername_done: c_int,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_status_type: c_int,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_status_expected: c_int,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_ocsp_ids: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_ocsp_exts: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_ocsp_resp: *mut c_uchar,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_ocsp_resplen: c_int,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_ticket_expected: c_int,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC")
            ))]
            tlsext_ecpointformatlist_length: size_t,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC")
            ))]
            tlsext_ecpointformatlist: *mut c_uchar,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC")
            ))]
            tlsext_ellipticcurvelist_length: size_t,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC")
            ))]
            tlsext_ellipticcurvelist: *mut c_uchar,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_opaque_prf_input: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_opaque_prf_input_len: size_t,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_session_ticket: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_session_ticket_ext_cb: tls_session_ticket_ext_cb_fn,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tls_session_ticket_ext_cb_arg: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tls_session_secret_cb: tls_session_secret_cb_fn,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tls_session_secret_cb_arg: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            initial_ctx: *mut SSL_CTX,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_NEXTPROTONEG")
            ))]
            next_proto_negotiated: *mut c_uchar,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_NEXTPROTONEG")
            ))]
            next_proto_negotiated_len: c_uchar,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            srtp_profiles: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            srtp_profile: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_heartbeat: c_uint,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_hb_pending: c_uint,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_hb_seq: c_uint,
            renegotiate: c_int,
            #[cfg(not(osslconf = "OPENSSL_NO_SRP"))]
            srp_ctx: SRP_CTX,
            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl102))]
            alpn_client_proto_list: *mut c_uchar,
            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl102))]
            alpn_client_proto_list_len: c_uint,
        }
    }
}
cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum SSL_CTX {}
    } else if #[cfg(libressl251)] {
        #[repr(C)]
        pub struct SSL_CTX {
            method: *const SSL_METHOD,
            cipher_list: *mut stack_st_SSL_CIPHER,
            cert_store: *mut c_void,
            session_timeout: c_long,
            pub references: c_int,
            extra_certs: *mut stack_st_X509,
            verify_mode: c_int,
            sid_ctx_length: c_uint,
            sid_ctx: [c_uchar; SSL_MAX_SID_CTX_LENGTH as usize],
            param: *mut X509_VERIFY_PARAM,
            default_passwd_callback: *mut c_void,
            default_passwd_callback_userdata: *mut c_void,
            internal: *mut c_void,
        }
    } else if #[cfg(libressl)] {
        #[repr(C)]
        pub struct SSL_CTX {
            method: *mut c_void,
            cipher_list: *mut c_void,
            cipher_list_by_id: *mut c_void,
            cert_store: *mut c_void,
            sessions: *mut c_void,
            session_cache_size: c_ulong,
            session_cache_head: *mut c_void,
            session_cache_tail: *mut c_void,
            session_cache_mode: c_int,
            session_timeout: c_long,
            new_session_cb: *mut c_void,
            remove_session_cb: *mut c_void,
            get_session_cb: *mut c_void,
            stats: [c_int; 11],
            pub references: c_int,
            app_verify_callback: *mut c_void,
            app_verify_arg: *mut c_void,
            default_passwd_callback: *mut c_void,
            default_passwd_callback_userdata: *mut c_void,
            client_cert_cb: *mut c_void,
            app_gen_cookie_cb: *mut c_void,
            app_verify_cookie_cb: *mut c_void,
            ex_dat: CRYPTO_EX_DATA,
            rsa_md5: *mut c_void,
            md5: *mut c_void,
            sha1: *mut c_void,
            extra_certs: *mut c_void,
            comp_methods: *mut c_void,
            info_callback: *mut c_void,
            client_CA: *mut c_void,
            options: c_ulong,
            mode: c_ulong,
            max_cert_list: c_long,
            cert: *mut c_void,
            read_ahead: c_int,
            msg_callback: *mut c_void,
            msg_callback_arg: *mut c_void,
            verify_mode: c_int,
            sid_ctx_length: c_uint,
            sid_ctx: [c_uchar; 32],
            default_verify_callback: *mut c_void,
            generate_session_id: *mut c_void,
            param: *mut c_void,
            quiet_shutdown: c_int,
            max_send_fragment: c_uint,

            #[cfg(not(osslconf = "OPENSSL_NO_ENGINE"))]
            client_cert_engine: *mut c_void,

            tlsext_servername_callback: *mut c_void,
            tlsect_servername_arg: *mut c_void,
            tlsext_tick_key_name: [c_uchar; 16],
            tlsext_tick_hmac_key: [c_uchar; 16],
            tlsext_tick_aes_key: [c_uchar; 16],
            tlsext_ticket_key_cb: *mut c_void,
            tlsext_status_cb: *mut c_void,
            tlsext_status_arg: *mut c_void,
            tlsext_opaque_prf_input_callback: *mut c_void,
            tlsext_opaque_prf_input_callback_arg: *mut c_void,

            next_protos_advertised_cb: *mut c_void,
            next_protos_advertised_cb_arg: *mut c_void,
            next_proto_select_cb: *mut c_void,
            next_proto_select_cb_arg: *mut c_void,

            srtp_profiles: *mut c_void,
        }
    } else {
        #[repr(C)]
        pub struct SSL_CTX {
            method: *mut c_void,
            cipher_list: *mut c_void,
            cipher_list_by_id: *mut c_void,
            cert_store: *mut c_void,
            sessions: *mut c_void,
            session_cache_size: c_ulong,
            session_cache_head: *mut c_void,
            session_cache_tail: *mut c_void,
            session_cache_mode: c_int,
            session_timeout: c_long,
            new_session_cb: *mut c_void,
            remove_session_cb: *mut c_void,
            get_session_cb: *mut c_void,
            stats: [c_int; 11],
            pub references: c_int,
            app_verify_callback: *mut c_void,
            app_verify_arg: *mut c_void,
            default_passwd_callback: *mut c_void,
            default_passwd_callback_userdata: *mut c_void,
            client_cert_cb: *mut c_void,
            app_gen_cookie_cb: *mut c_void,
            app_verify_cookie_cb: *mut c_void,
            ex_dat: CRYPTO_EX_DATA,
            rsa_md5: *mut c_void,
            md5: *mut c_void,
            sha1: *mut c_void,
            extra_certs: *mut c_void,
            comp_methods: *mut c_void,
            info_callback: *mut c_void,
            client_CA: *mut c_void,
            options: c_ulong,
            mode: c_ulong,
            max_cert_list: c_long,
            cert: *mut c_void,
            read_ahead: c_int,
            msg_callback: *mut c_void,
            msg_callback_arg: *mut c_void,
            verify_mode: c_int,
            sid_ctx_length: c_uint,
            sid_ctx: [c_uchar; 32],
            default_verify_callback: *mut c_void,
            generate_session_id: *mut c_void,
            param: *mut c_void,
            quiet_shutdown: c_int,
            max_send_fragment: c_uint,

            #[cfg(not(osslconf = "OPENSSL_NO_ENGINE"))]
            client_cert_engine: *mut c_void,

            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_servername_callback: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsect_servername_arg: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_tick_key_name: [c_uchar; 16],
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_tick_hmac_key: [c_uchar; 16],
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_tick_aes_key: [c_uchar; 16],
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_ticket_key_cb: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_status_cb: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_status_arg: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_opaque_prf_input_callback: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_TLSEXT"))]
            tlsext_opaque_prf_input_callback_arg: *mut c_void,

            #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
            psk_identity_hint: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
            psk_client_callback: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
            psk_server_callback: *mut c_void,

            #[cfg(not(osslconf = "OPENSSL_NO_BUF_FREELISTS"))]
            freelist_max_len: c_uint,
            #[cfg(not(osslconf = "OPENSSL_NO_BUF_FREELISTS"))]
            wbuf_freelist: *mut c_void,
            #[cfg(not(osslconf = "OPENSSL_NO_BUF_FREELISTS"))]
            rbuf_freelist: *mut c_void,

            #[cfg(not(osslconf = "OPENSSL_NO_SRP"))]
            srp_ctx: SRP_CTX,

            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_NEXTPROTONEG")
            ))]
            next_protos_advertised_cb: *mut c_void,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_NEXTPROTONEG")
            ))]
            next_protos_advertised_cb_arg: *mut c_void,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_NEXTPROTONEG")
            ))]
            next_proto_select_cb: *mut c_void,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_NEXTPROTONEG")
            ))]
            next_proto_select_cb_arg: *mut c_void,

            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl101))]
            srtp_profiles: *mut c_void,
            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl102))]
            alpn_select_cb: *mut c_void,
            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl102))]
            alpn_select_cb_arg: *mut c_void,
            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl102))]
            alpn_client_proto_list: *mut c_void,
            #[cfg(all(not(osslconf = "OPENSSL_NO_TLSEXT"), ossl102))]
            alpn_client_proto_list_len: c_uint,

            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC"),
                ossl102
            ))]
            tlsext_ecpointformatlist_length: size_t,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC"),
                ossl102
            ))]
            tlsext_ecpointformatlist: *mut c_uchar,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC"),
                ossl102
            ))]
            tlsext_ellipticcurvelist_length: size_t,
            #[cfg(all(
                not(osslconf = "OPENSSL_NO_TLSEXT"),
                not(osslconf = "OPENSSL_NO_EC"),
                ossl102
            ))]
            tlsext_ellipticcurvelist: *mut c_uchar,
        }

        #[repr(C)]
        #[cfg(not(osslconf = "OPENSSL_NO_SRP"))]
        pub struct SRP_CTX {
            SRP_cb_arg: *mut c_void,
            TLS_ext_srp_username_callback: *mut c_void,
            SRP_verify_param_callback: *mut c_void,
            SRP_give_srp_client_pwd_callback: *mut c_void,
            login: *mut c_void,
            N: *mut c_void,
            g: *mut c_void,
            s: *mut c_void,
            B: *mut c_void,
            A: *mut c_void,
            a: *mut c_void,
            b: *mut c_void,
            v: *mut c_void,
            info: *mut c_void,
            stringth: c_int,
            srp_Mask: c_ulong,
        }
    }
}
cfg_if! {
    if #[cfg(ossl320)] {
        #[repr(C)]
        pub struct SSL_CONN_CLOSE_INFO {
            pub error_code: u64,
            pub frame_type: u64,
            pub reason: *const ::libc::c_char,
            pub reason_len: usize,
            pub flags: u32,
        }
        #[repr(C)]
        pub struct SSL_SHUTDOWN_EX_ARGS {
            pub quic_error_code: u64,
            pub quic_reason: *const c_char,
        }
        #[repr(C)]
        pub struct SSL_STREAM_RESET_ARGS {
            pub quic_error_code: u64,
        }
    }
}

pub enum COMP_CTX {}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum COMP_METHOD {}
    } else {
        #[repr(C)]
        pub struct COMP_METHOD {
            pub type_: c_int,
            pub name: *const c_char,
            init: Option<unsafe extern "C" fn(*mut COMP_CTX) -> c_int>,
            finish: Option<unsafe extern "C" fn(*mut COMP_CTX)>,
            compress: Option<
                unsafe extern "C" fn(
                    *mut COMP_CTX,
                    *mut c_uchar,
                    c_uint,
                    *mut c_uchar,
                    c_uint,
                ) -> c_int,
            >,
            expand: Option<
                unsafe extern "C" fn(
                    *mut COMP_CTX,
                    *mut c_uchar,
                    c_uint,
                    *mut c_uchar,
                    c_uint,
                ) -> c_int,
            >,
            ctrl: Option<unsafe extern "C" fn() -> c_long>,
            callback_ctrl: Option<unsafe extern "C" fn() -> c_long>,
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum CRYPTO_EX_DATA {}
    } else if #[cfg(libressl)] {
        #[repr(C)]
        pub struct CRYPTO_EX_DATA {
            pub sk: *mut stack_st_void,
        }
    } else {
        #[repr(C)]
        pub struct CRYPTO_EX_DATA {
            pub sk: *mut stack_st_void,
            pub dummy: c_int,
        }
    }
}

pub enum OCSP_RESPONSE {}

#[cfg(ossl300)]
pub enum OSSL_PROVIDER {}

#[cfg(ossl300)]
pub enum OSSL_LIB_CTX {}

#[cfg(ossl300)]
#[repr(C)]
pub struct OSSL_PARAM {
    key: *const c_char,
    data_type: c_uchar,
    data: *mut c_void,
    data_size: size_t,
    return_size: size_t,
}

#[cfg(ossl300)]
pub enum EVP_KDF {}
#[cfg(ossl300)]
pub enum EVP_KDF_CTX {}
