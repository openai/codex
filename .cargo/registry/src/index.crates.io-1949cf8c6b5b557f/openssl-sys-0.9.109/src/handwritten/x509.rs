use super::super::*;
use libc::*;

cfg_if! {
    if #[cfg(libressl400)] {
        pub enum X509_VAL {}
    } else {
        #[repr(C)]
        pub struct X509_VAL {
            pub notBefore: *mut ASN1_TIME,
            pub notAfter: *mut ASN1_TIME,
        }
    }
}

pub enum X509_NAME_ENTRY {}

stack!(stack_st_X509_NAME_ENTRY);

stack!(stack_st_X509_NAME);

pub enum X509_EXTENSION {}

stack!(stack_st_X509_EXTENSION);

pub enum X509_ATTRIBUTE {}

stack!(stack_st_X509_ATTRIBUTE);

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum X509_REQ_INFO {}
    } else {
        #[repr(C)]
        pub struct X509_REQ_INFO {
            pub enc: ASN1_ENCODING,
            pub version: *mut ASN1_INTEGER,
            pub subject: *mut X509_NAME,
            pubkey: *mut c_void,
            pub attributes: *mut stack_st_X509_ATTRIBUTE,
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum X509_CRL {}
    } else {
        #[repr(C)]
        pub struct X509_CRL {
            pub crl: *mut X509_CRL_INFO,
            sig_alg: *mut X509_ALGOR,
            signature: *mut c_void,
            references: c_int,
            flags: c_int,
            akid: *mut c_void,
            idp: *mut c_void,
            idp_flags: c_int,
            idp_reasons: c_int,
            crl_number: *mut ASN1_INTEGER,
            base_crl_number: *mut ASN1_INTEGER,
            sha1_hash: [c_uchar; 20],
            issuers: *mut c_void,
            meth: *const c_void,
            meth_data: *mut c_void,
        }
    }
}

stack!(stack_st_X509_CRL);

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum X509_CRL_INFO {}
    } else {
        #[repr(C)]
        pub struct X509_CRL_INFO {
            version: *mut ASN1_INTEGER,
            sig_alg: *mut X509_ALGOR,
            pub issuer: *mut X509_NAME,
            pub lastUpdate: *mut ASN1_TIME,
            pub nextUpdate: *mut ASN1_TIME,
            pub revoked: *mut stack_st_X509_REVOKED,
            extensions: *mut stack_st_X509_EXTENSION,
            enc: ASN1_ENCODING,
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum X509_REVOKED {}
    } else {
        #[repr(C)]
        pub struct X509_REVOKED {
            pub serialNumber: *mut ASN1_INTEGER,
            pub revocationDate: *mut ASN1_TIME,
            pub extensions: *mut stack_st_X509_EXTENSION,
            issuer: *mut stack_st_GENERAL_NAME,
            reason: c_int,
            sequence: c_int,
        }
    }
}

stack!(stack_st_X509_REVOKED);

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum X509_REQ {}
    } else {
        #[repr(C)]
        pub struct X509_REQ {
            pub req_info: *mut X509_REQ_INFO,
            sig_alg: *mut c_void,
            signature: *mut c_void,
            references: c_int,
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        pub enum X509_CINF {}
    } else {
        #[repr(C)]
        pub struct X509_CINF {
            version: *mut c_void,
            serialNumber: *mut c_void,
            signature: *mut c_void,
            issuer: *mut c_void,
            pub validity: *mut X509_VAL,
            subject: *mut c_void,
            key: *mut c_void,
            issuerUID: *mut c_void,
            subjectUID: *mut c_void,
            pub extensions: *mut stack_st_X509_EXTENSION,
            enc: ASN1_ENCODING,
        }
    }
}

stack!(stack_st_X509);

stack!(stack_st_X509_OBJECT);

stack!(stack_st_X509_LOOKUP);

extern "C" {
    pub fn X509_verify_cert_error_string(n: c_long) -> *const c_char;

    pub fn X509_sign(x: *mut X509, pkey: *mut EVP_PKEY, md: *const EVP_MD) -> c_int;

    pub fn X509_digest(
        x: *const X509,
        digest: *const EVP_MD,
        buf: *mut c_uchar,
        len: *mut c_uint,
    ) -> c_int;

    pub fn X509_REQ_sign(x: *mut X509_REQ, pkey: *mut EVP_PKEY, md: *const EVP_MD) -> c_int;
}

const_ptr_api! {
    extern "C" {
        pub fn i2d_X509_bio(b: *mut BIO, x: #[const_ptr_if(ossl300)] X509) -> c_int;
        pub fn i2d_X509_REQ_bio(b: *mut BIO, x: #[const_ptr_if(ossl300)] X509_REQ) -> c_int;
        pub fn i2d_PrivateKey_bio(b: *mut BIO, x: #[const_ptr_if(ossl300)] EVP_PKEY) -> c_int;
        pub fn i2d_PUBKEY_bio(b: *mut BIO, x: #[const_ptr_if(ossl300)] EVP_PKEY) -> c_int;

        pub fn i2d_PUBKEY(k: #[const_ptr_if(ossl300)] EVP_PKEY, buf: *mut *mut u8) -> c_int;
        pub fn i2d_RSA_PUBKEY(k: #[const_ptr_if(ossl300)] RSA, buf: *mut *mut u8) -> c_int;
        pub fn i2d_DSA_PUBKEY(a: #[const_ptr_if(ossl300)] DSA, pp: *mut *mut c_uchar) -> c_int;
        pub fn i2d_PrivateKey(k: #[const_ptr_if(ossl300)] EVP_PKEY, buf: *mut *mut u8) -> c_int;
        pub fn i2d_ECPrivateKey(ec_key: #[const_ptr_if(ossl300)] EC_KEY, pp: *mut *mut c_uchar) -> c_int;
        pub fn i2d_EC_PUBKEY(a: #[const_ptr_if(ossl300)] EC_KEY, pp: *mut *mut c_uchar) -> c_int;
    }
}
extern "C" {
    pub fn d2i_PUBKEY(k: *mut *mut EVP_PKEY, buf: *mut *const u8, len: c_long) -> *mut EVP_PKEY;
    pub fn d2i_RSA_PUBKEY(k: *mut *mut RSA, buf: *mut *const u8, len: c_long) -> *mut RSA;
    pub fn d2i_DSA_PUBKEY(k: *mut *mut DSA, pp: *mut *const c_uchar, length: c_long) -> *mut DSA;
    pub fn d2i_EC_PUBKEY(
        a: *mut *mut EC_KEY,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut EC_KEY;

    pub fn d2i_ECPrivateKey(
        k: *mut *mut EC_KEY,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut EC_KEY;
}

const_ptr_api! {
    extern "C" {
        #[cfg(any(ossl102, libressl350))]
        pub fn X509_ALGOR_get0(
            paobj: *mut #[const_ptr_if(any(ossl110, libressl350))] ASN1_OBJECT,
            pptype: *mut c_int,
            ppval: *mut #[const_ptr_if(any(ossl110, libressl350))] c_void,
            alg: #[const_ptr_if(any(ossl110, libressl350))] X509_ALGOR,
        );
    }
}

extern "C" {
    pub fn X509_gmtime_adj(time: *mut ASN1_TIME, adj: c_long) -> *mut ASN1_TIME;

    pub fn X509_to_X509_REQ(x: *mut X509, pkey: *mut EVP_PKEY, md: *const EVP_MD) -> *mut X509_REQ;

    pub fn X509_ALGOR_free(x: *mut X509_ALGOR);

    pub fn X509_REVOKED_new() -> *mut X509_REVOKED;
    pub fn X509_REVOKED_free(x: *mut X509_REVOKED);
}
const_ptr_api! {
    extern "C" {
        #[cfg(any(ossl110, libressl270))]
        pub fn X509_REVOKED_dup(rev: #[const_ptr_if(ossl300)] X509_REVOKED) -> *mut X509_REVOKED;
    }
}

extern "C" {
    pub fn d2i_X509_REVOKED(
        a: *mut *mut X509_REVOKED,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut X509_REVOKED;
}
const_ptr_api! {
    extern "C" {
        pub fn i2d_X509_REVOKED(x: #[const_ptr_if(ossl300)] X509_REVOKED, buf: *mut *mut u8) -> c_int;
    }
}
extern "C" {
    pub fn X509_CRL_new() -> *mut X509_CRL;
    pub fn X509_CRL_free(x: *mut X509_CRL);
    pub fn d2i_X509_CRL(
        a: *mut *mut X509_CRL,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut X509_CRL;
}
const_ptr_api! {
    extern "C" {
        pub fn i2d_X509_CRL(x: #[const_ptr_if(ossl300)] X509_CRL, buf: *mut *mut u8) -> c_int;
        #[cfg(any(ossl110, libressl270))]
        pub fn X509_CRL_dup(x: #[const_ptr_if(ossl300)] X509_CRL) -> *mut X509_CRL;
    }
}

extern "C" {
    pub fn X509_REQ_new() -> *mut X509_REQ;
    pub fn X509_REQ_free(x: *mut X509_REQ);
    pub fn d2i_X509_REQ(
        a: *mut *mut X509_REQ,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut X509_REQ;
}
const_ptr_api! {
    extern "C" {
        pub fn i2d_X509_REQ(x: #[const_ptr_if(ossl300)] X509_REQ, buf: *mut *mut u8) -> c_int;

        #[cfg(any(ossl102, libressl273))]
        pub fn X509_get0_signature(
            psig: *mut #[const_ptr_if(any(ossl110, libressl273))] ASN1_BIT_STRING,
            palg: *mut #[const_ptr_if(any(ossl110, libressl273))] X509_ALGOR,
            x: *const X509,
        );

        #[cfg(any(ossl110, libressl270))]
        pub fn X509_REQ_dup(x: #[const_ptr_if(ossl300)] X509_REQ) -> *mut X509_REQ;
    }
}
extern "C" {
    #[cfg(ossl102)]
    pub fn X509_get_signature_nid(x: *const X509) -> c_int;

    pub fn X509_EXTENSION_free(ext: *mut X509_EXTENSION);

    pub fn X509_NAME_ENTRY_free(x: *mut X509_NAME_ENTRY);

    pub fn X509_NAME_new() -> *mut X509_NAME;
    pub fn X509_NAME_cmp(x: *const X509_NAME, y: *const X509_NAME) -> c_int;
    pub fn X509_NAME_free(x: *mut X509_NAME);

    pub fn X509_new() -> *mut X509;
    pub fn X509_free(x: *mut X509);
}
const_ptr_api! {
    extern "C" {
        pub fn i2d_X509(x: #[const_ptr_if(ossl300)] X509, buf: *mut *mut u8) -> c_int;
        #[cfg(any(ossl110, libressl270))]
        pub fn X509_NAME_dup(x: #[const_ptr_if(ossl300)] X509_NAME) -> *mut X509_NAME;
        #[cfg(any(ossl110, libressl270))]
        pub fn X509_dup(x: #[const_ptr_if(ossl300)] X509) -> *mut X509;
        #[cfg(any(ossl101, libressl350))]
        pub fn X509_NAME_add_entry(
            name: *mut X509_NAME,
            ne: #[const_ptr_if(any(ossl110, libressl))] X509_NAME_ENTRY,
            loc: c_int,
            set: c_int,
            ) -> c_int;
    }
}
extern "C" {
    pub fn d2i_X509(a: *mut *mut X509, pp: *mut *const c_uchar, length: c_long) -> *mut X509;
    pub fn d2i_X509_bio(b: *mut BIO, a: *mut *mut X509) -> *mut X509;

    pub fn X509_get_pubkey(x: *mut X509) -> *mut EVP_PKEY;

    pub fn X509_set_version(x: *mut X509, version: c_long) -> c_int;
    #[cfg(ossl110)]
    pub fn X509_get_version(x: *const X509) -> c_long;
    pub fn X509_set_serialNumber(x: *mut X509, sn: *mut ASN1_INTEGER) -> c_int;
    pub fn X509_get_serialNumber(x: *mut X509) -> *mut ASN1_INTEGER;
    pub fn X509_alias_get0(x: *mut X509, len: *mut c_int) -> *mut c_uchar;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_set_issuer_name(x: *mut X509, name: #[const_ptr_if(ossl300)] X509_NAME) -> c_int;
    }
}
extern "C" {
    pub fn X509_issuer_name_hash(x: *mut X509) -> c_ulong;
    pub fn X509_subject_name_hash(x: *mut X509) -> c_ulong;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_get_issuer_name(x: #[const_ptr_if(any(ossl110, libressl280))] X509) -> *mut X509_NAME;
        pub fn X509_set_subject_name(x: *mut X509, name: #[const_ptr_if(ossl300)] X509_NAME) -> c_int;
        pub fn X509_get_subject_name(x: #[const_ptr_if(any(ossl110, libressl280))] X509) -> *mut X509_NAME;
    }
}
cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        extern "C" {
            pub fn X509_set1_notBefore(x: *mut X509, tm: *const ASN1_TIME) -> c_int;
            pub fn X509_set1_notAfter(x: *mut X509, tm: *const ASN1_TIME) -> c_int;
        }
    } else {
        extern "C" {
            pub fn X509_set_notBefore(x: *mut X509, tm: *const ASN1_TIME) -> c_int;
            pub fn X509_set_notAfter(x: *mut X509, tm: *const ASN1_TIME) -> c_int;
        }
    }
}
extern "C" {
    #[cfg(any(ossl110, libressl350))]
    pub fn X509_REQ_get_version(req: *const X509_REQ) -> c_long;
    pub fn X509_REQ_set_version(req: *mut X509_REQ, version: c_long) -> c_int;
    #[cfg(any(ossl110, libressl350))]
    pub fn X509_REQ_get_subject_name(req: *const X509_REQ) -> *mut X509_NAME;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_REQ_set_subject_name(req: *mut X509_REQ, name: #[const_ptr_if(ossl300)] X509_NAME) -> c_int;
    }
}
extern "C" {
    pub fn X509_REQ_set_pubkey(req: *mut X509_REQ, pkey: *mut EVP_PKEY) -> c_int;
    pub fn X509_REQ_get_pubkey(req: *mut X509_REQ) -> *mut EVP_PKEY;
    pub fn X509_REQ_get_extensions(req: *mut X509_REQ) -> *mut stack_st_X509_EXTENSION;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_REQ_add_extensions(req: *mut X509_REQ, exts: #[const_ptr_if(ossl300)] stack_st_X509_EXTENSION)
            -> c_int;
    }
}
extern "C" {
    pub fn X509_REQ_get_attr_count(req: *const X509_REQ) -> c_int;
    pub fn X509_REQ_get_attr_by_NID(req: *const X509_REQ, nid: c_int, lastpos: c_int) -> c_int;
    pub fn X509_REQ_get_attr(req: *const X509_REQ, loc: c_int) -> *mut X509_ATTRIBUTE;
    pub fn X509_REQ_delete_attr(req: *mut X509_REQ, loc: c_int) -> *mut X509_ATTRIBUTE;
    pub fn X509_REQ_add1_attr_by_txt(
        req: *mut X509_REQ,
        attrname: *const c_char,
        chtype: c_int,
        bytes: *const c_uchar,
        len: c_int,
    ) -> c_int;
    pub fn X509_REQ_add1_attr_by_NID(
        req: *mut X509_REQ,
        nid: c_int,
        chtype: c_int,
        bytes: *const c_uchar,
        len: c_int,
    ) -> c_int;
    pub fn X509_REQ_add1_attr_by_OBJ(
        req: *mut X509_REQ,
        obj: *const ASN1_OBJECT,
        chtype: c_int,
        bytes: *const c_uchar,
        len: c_int,
    ) -> c_int;
}
extern "C" {
    pub fn X509_set_pubkey(x: *mut X509, pkey: *mut EVP_PKEY) -> c_int;
    pub fn X509_REQ_verify(req: *mut X509_REQ, pkey: *mut EVP_PKEY) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    pub fn X509_getm_notBefore(x: *const X509) -> *mut ASN1_TIME;
    #[cfg(any(ossl110, libressl273))]
    pub fn X509_getm_notAfter(x: *const X509) -> *mut ASN1_TIME;
    #[cfg(any(ossl110, libressl273))]
    pub fn X509_up_ref(x: *mut X509) -> c_int;

    #[cfg(any(ossl110, libressl270))]
    pub fn X509_REVOKED_get0_serialNumber(req: *const X509_REVOKED) -> *const ASN1_INTEGER;
    #[cfg(any(ossl110, libressl270))]
    pub fn X509_REVOKED_get0_revocationDate(req: *const X509_REVOKED) -> *const ASN1_TIME;
    #[cfg(any(ossl110, libressl270))]
    pub fn X509_REVOKED_get0_extensions(r: *const X509_REVOKED) -> *const stack_st_X509_EXTENSION;

    pub fn X509_REVOKED_set_serialNumber(r: *mut X509_REVOKED, serial: *mut ASN1_INTEGER) -> c_int;
    pub fn X509_REVOKED_set_revocationDate(r: *mut X509_REVOKED, tm: *mut ASN1_TIME) -> c_int;

    pub fn X509_CRL_sign(x: *mut X509_CRL, pkey: *mut EVP_PKEY, md: *const EVP_MD) -> c_int;
    pub fn X509_CRL_digest(
        x: *const X509_CRL,
        digest: *const EVP_MD,
        md: *mut c_uchar,
        len: *mut c_uint,
    ) -> c_int;
    pub fn X509_CRL_verify(crl: *mut X509_CRL, pkey: *mut EVP_PKEY) -> c_int;
    pub fn X509_CRL_get0_by_cert(
        x: *mut X509_CRL,
        ret: *mut *mut X509_REVOKED,
        cert: *mut X509,
    ) -> c_int;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_CRL_get0_by_serial(
            x: *mut X509_CRL,
            ret: *mut *mut X509_REVOKED,
            serial: #[const_ptr_if(ossl300)] ASN1_INTEGER,
        ) -> c_int;
    }
}

extern "C" {
    #[cfg(any(ossl110, libressl281))]
    pub fn X509_CRL_get_REVOKED(crl: *mut X509_CRL) -> *mut stack_st_X509_REVOKED;
    #[cfg(any(ossl110, libressl281))]
    pub fn X509_CRL_get0_nextUpdate(x: *const X509_CRL) -> *const ASN1_TIME;
    #[cfg(any(ossl110, libressl281))]
    pub fn X509_CRL_get0_lastUpdate(x: *const X509_CRL) -> *const ASN1_TIME;
    #[cfg(any(ossl110, libressl281))]
    pub fn X509_CRL_get_issuer(x: *const X509_CRL) -> *mut X509_NAME;

    #[cfg(ossl110)]
    pub fn X509_get0_extensions(req: *const X509) -> *const stack_st_X509_EXTENSION;

    pub fn X509_CRL_set_version(crl: *mut X509_CRL, version: c_long) -> c_int;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_CRL_set_issuer_name(crl: *mut X509_CRL, name: #[const_ptr_if(ossl300)] X509_NAME) -> c_int;
    }
}
extern "C" {
    pub fn X509_CRL_sort(crl: *mut X509_CRL) -> c_int;

    #[cfg(any(ossl110, libressl270))]
    pub fn X509_CRL_up_ref(crl: *mut X509_CRL) -> c_int;
    pub fn X509_CRL_add0_revoked(crl: *mut X509_CRL, rev: *mut X509_REVOKED) -> c_int;
}
cfg_if! {
    if #[cfg(any(ossl110, libressl270))] {
        extern "C" {
            pub fn X509_CRL_set1_lastUpdate(crl: *mut X509_CRL, tm: *const ASN1_TIME) -> c_int;
            pub fn X509_CRL_set1_nextUpdate(crl: *mut X509_CRL, tm: *const ASN1_TIME) -> c_int;
        }
    } else {
        // libressl270 kept them, ossl110 "#define"s them to the variants above
        extern "C" {
            pub fn X509_CRL_set_lastUpdate(crl: *mut X509_CRL, tm: *const ASN1_TIME) -> c_int;
            pub fn X509_CRL_set_nextUpdate(crl: *mut X509_CRL, tm: *const ASN1_TIME) -> c_int;
        }
    }
}

const_ptr_api! {
    extern "C" {
        pub fn X509_NAME_entry_count(n: #[const_ptr_if(any(ossl110, libressl280))] X509_NAME) -> c_int;
        pub fn X509_NAME_get_index_by_NID(n: #[const_ptr_if(any(ossl300, libressl280))] X509_NAME, nid: c_int, last_pos: c_int) -> c_int;
        pub fn X509_NAME_get_entry(n: #[const_ptr_if(any(ossl110, libressl280))] X509_NAME, loc: c_int) -> *mut X509_NAME_ENTRY;
        pub fn X509_NAME_add_entry_by_NID(
            x: *mut X509_NAME,
            field: c_int,
            ty: c_int,
            bytes: #[const_ptr_if(any(ossl110, libressl280))] c_uchar,
            len: c_int,
            loc: c_int,
            set: c_int,
        ) -> c_int;
        pub fn i2d_X509_NAME(n: #[const_ptr_if(ossl300)] X509_NAME, buf: *mut *mut u8) -> c_int;
        pub fn X509_NAME_ENTRY_get_object(ne: #[const_ptr_if(any(ossl110, libressl280))] X509_NAME_ENTRY) -> *mut ASN1_OBJECT;
        pub fn X509_NAME_ENTRY_get_data(ne: #[const_ptr_if(any(ossl110, libressl280))] X509_NAME_ENTRY) -> *mut ASN1_STRING;
    }
}
extern "C" {
    pub fn X509_NAME_add_entry_by_txt(
        x: *mut X509_NAME,
        field: *const c_char,
        ty: c_int,
        bytes: *const c_uchar,
        len: c_int,
        loc: c_int,
        set: c_int,
    ) -> c_int;
    pub fn d2i_X509_NAME(
        n: *mut *mut X509_NAME,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut X509_NAME;
}

// "raw" X509_EXTENSION related functions
extern "C" {
    // in X509
    pub fn X509_delete_ext(x: *mut X509, loc: c_int) -> *mut X509_EXTENSION;
    pub fn X509_add_ext(x: *mut X509, ext: *mut X509_EXTENSION, loc: c_int) -> c_int;
    pub fn X509_add1_ext_i2d(
        x: *mut X509,
        nid: c_int,
        value: *mut c_void,
        crit: c_int,
        flags: c_ulong,
    ) -> c_int;
    // in X509_CRL
    pub fn X509_CRL_delete_ext(x: *mut X509_CRL, loc: c_int) -> *mut X509_EXTENSION;
    pub fn X509_CRL_add_ext(x: *mut X509_CRL, ext: *mut X509_EXTENSION, loc: c_int) -> c_int;
    pub fn X509_CRL_add1_ext_i2d(
        x: *mut X509_CRL,
        nid: c_int,
        value: *mut c_void,
        crit: c_int,
        flags: c_ulong,
    ) -> c_int;
    // in X509_REVOKED
    pub fn X509_REVOKED_delete_ext(x: *mut X509_REVOKED, loc: c_int) -> *mut X509_EXTENSION;
    pub fn X509_REVOKED_add_ext(
        x: *mut X509_REVOKED,
        ext: *mut X509_EXTENSION,
        loc: c_int,
    ) -> c_int;
    pub fn X509_REVOKED_add1_ext_i2d(
        x: *mut X509_REVOKED,
        nid: c_int,
        value: *mut c_void,
        crit: c_int,
        flags: c_ulong,
    ) -> c_int;
    // X509_EXTENSION stack
    // - these getters always used *const STACK
    pub fn X509v3_get_ext_count(x: *const stack_st_X509_EXTENSION) -> c_int;
    pub fn X509v3_get_ext_by_NID(
        x: *const stack_st_X509_EXTENSION,
        nid: c_int,
        lastpos: c_int,
    ) -> c_int;
    pub fn X509v3_get_ext_by_critical(
        x: *const stack_st_X509_EXTENSION,
        crit: c_int,
        lastpos: c_int,
    ) -> c_int;
    pub fn X509v3_get_ext(x: *const stack_st_X509_EXTENSION, loc: c_int) -> *mut X509_EXTENSION;
    pub fn X509v3_delete_ext(x: *mut stack_st_X509_EXTENSION, loc: c_int) -> *mut X509_EXTENSION;
    pub fn X509v3_add_ext(
        x: *mut *mut stack_st_X509_EXTENSION,
        ex: *mut X509_EXTENSION,
        loc: c_int,
    ) -> *mut stack_st_X509_EXTENSION;
    // - X509V3_add1_i2d in x509v3.rs
    // X509_EXTENSION itself
    pub fn X509_EXTENSION_create_by_NID(
        ex: *mut *mut X509_EXTENSION,
        nid: c_int,
        crit: c_int,
        data: *mut ASN1_OCTET_STRING,
    ) -> *mut X509_EXTENSION;
    pub fn X509_EXTENSION_set_critical(ex: *mut X509_EXTENSION, crit: c_int) -> c_int;
    pub fn X509_EXTENSION_set_data(ex: *mut X509_EXTENSION, data: *mut ASN1_OCTET_STRING) -> c_int;
    pub fn X509_EXTENSION_get_object(ext: *mut X509_EXTENSION) -> *mut ASN1_OBJECT;
    pub fn X509_EXTENSION_get_data(ext: *mut X509_EXTENSION) -> *mut ASN1_OCTET_STRING;
}

const_ptr_api! {
    extern "C" {
        pub fn i2d_X509_EXTENSION(ext: #[const_ptr_if(ossl300)] X509_EXTENSION, pp: *mut *mut c_uchar) -> c_int;
    }
}

const_ptr_api! {
    extern "C" {
        // in X509
        pub fn X509_get_ext_count(x: #[const_ptr_if(any(ossl110, libressl280))] X509) -> c_int;
        pub fn X509_get_ext_by_NID(x: #[const_ptr_if(any(ossl110, libressl280))] X509, nid: c_int, lastpos: c_int) -> c_int;
        pub fn X509_get_ext_by_OBJ(x: #[const_ptr_if(any(ossl110, libressl280))] X509, obj: #[const_ptr_if(any(ossl110, libressl280))] ASN1_OBJECT, lastpos: c_int) -> c_int;
        pub fn X509_get_ext_by_critical(x: #[const_ptr_if(any(ossl110, libressl280))] X509, crit: c_int, lastpos: c_int) -> c_int;
        pub fn X509_get_ext(x: #[const_ptr_if(any(ossl110, libressl280))] X509, loc: c_int) -> *mut X509_EXTENSION;
        pub fn X509_get_ext_d2i(
            x: #[const_ptr_if(any(ossl110, libressl280))] X509,
            nid: c_int,
            crit: *mut c_int,
            idx: *mut c_int,
        ) -> *mut c_void;
        // in X509_CRL
        pub fn X509_CRL_get_ext_count(x: #[const_ptr_if(any(ossl110, libressl280))] X509_CRL) -> c_int;
        pub fn X509_CRL_get_ext_by_NID(x: #[const_ptr_if(any(ossl110, libressl280))] X509_CRL, nid: c_int, lastpos: c_int) -> c_int;
        pub fn X509_CRL_get_ext_by_OBJ(x: #[const_ptr_if(any(ossl110, libressl280))] X509_CRL, obj: #[const_ptr_if(any(ossl110, libressl280))] ASN1_OBJECT, lastpos: c_int) -> c_int;
        pub fn X509_CRL_get_ext_by_critical(x: #[const_ptr_if(any(ossl110, libressl280))] X509_CRL, crit: c_int, lastpos: c_int) -> c_int;
        pub fn X509_CRL_get_ext(x: #[const_ptr_if(any(ossl110, libressl280))] X509_CRL, loc: c_int) -> *mut X509_EXTENSION;
        pub fn X509_CRL_get_ext_d2i(
            x: #[const_ptr_if(any(ossl110, libressl280))] X509_CRL,
            nid: c_int,
            crit: *mut c_int,
            idx: *mut c_int,
        ) -> *mut c_void;
        // in X509_REVOKED
        pub fn X509_REVOKED_get_ext_count(x: #[const_ptr_if(any(ossl110, libressl280))] X509_REVOKED) -> c_int;
        pub fn X509_REVOKED_get_ext_by_NID(x: #[const_ptr_if(any(ossl110, libressl280))] X509_REVOKED, nid: c_int, lastpos: c_int) -> c_int;
        pub fn X509_REVOKED_get_ext_by_OBJ(x: #[const_ptr_if(any(ossl110, libressl280))] X509_REVOKED, obj: #[const_ptr_if(any(ossl110, libressl280))] ASN1_OBJECT, lastpos: c_int) -> c_int;
        pub fn X509_REVOKED_get_ext_by_critical(x: #[const_ptr_if(any(ossl110, libressl280))] X509_REVOKED, crit: c_int, lastpos: c_int) -> c_int;
        pub fn X509_REVOKED_get_ext(x: #[const_ptr_if(any(ossl110, libressl280))] X509_REVOKED, loc: c_int) -> *mut X509_EXTENSION;
        pub fn X509_REVOKED_get_ext_d2i(
            x: #[const_ptr_if(any(ossl110, libressl280))] X509_REVOKED,
            nid: c_int,
            crit: *mut c_int,
            idx: *mut c_int,
        ) -> *mut c_void;
        // X509_EXTENSION stack
        pub fn X509v3_get_ext_by_OBJ(x: *const stack_st_X509_EXTENSION, obj: #[const_ptr_if(any(ossl110, libressl280))] ASN1_OBJECT, lastpos: c_int) -> c_int;
        // X509_EXTENSION itself
        pub fn X509_EXTENSION_create_by_OBJ(ex: *mut *mut X509_EXTENSION, obj: #[const_ptr_if(any(ossl110, libressl280))] ASN1_OBJECT, crit: c_int, data: *mut ASN1_OCTET_STRING) -> *mut X509_EXTENSION;
        pub fn X509_EXTENSION_set_object(ex: *mut X509_EXTENSION, obj: #[const_ptr_if(any(ossl110, libressl280))] ASN1_OBJECT) -> c_int;
        pub fn X509_EXTENSION_get_critical(ex: #[const_ptr_if(any(ossl110, libressl280))] X509_EXTENSION) -> c_int;
    }
}

extern "C" {
    pub fn X509_verify_cert(ctx: *mut X509_STORE_CTX) -> c_int;
}

const_ptr_api! {
    extern "C" {
        #[cfg(any(ossl110, libressl270))]
        pub fn X509_STORE_get0_objects(ctx: #[const_ptr_if(ossl300)] X509_STORE) -> *mut stack_st_X509_OBJECT;
        #[cfg(ossl300)]
        pub fn X509_STORE_get1_all_certs(ctx: *mut X509_STORE) -> *mut stack_st_X509;
    }
}

#[cfg(any(ossl110, libressl270))]
extern "C" {
    pub fn X509_OBJECT_get0_X509(x: *const X509_OBJECT) -> *mut X509;
}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        extern "C" {
            pub fn X509_OBJECT_free(a: *mut X509_OBJECT);
        }
    } else {
        extern "C" {
            pub fn X509_OBJECT_free_contents(a: *mut X509_OBJECT);
        }
    }
}

extern "C" {
    pub fn X509_get_default_cert_file_env() -> *const c_char;
    pub fn X509_get_default_cert_file() -> *const c_char;
    pub fn X509_get_default_cert_dir_env() -> *const c_char;
    pub fn X509_get_default_cert_dir() -> *const c_char;
}

extern "C" {
    pub fn X509_cmp(a: *const X509, b: *const X509) -> c_int;
    pub fn X509_issuer_and_serial_cmp(a: *const X509, b: *const X509) -> c_int;
    pub fn X509_issuer_name_cmp(a: *const X509, b: *const X509) -> c_int;
    pub fn X509_subject_name_cmp(a: *const X509, b: *const X509) -> c_int;
    pub fn X509_CRL_cmp(a: *const X509_CRL, b: *const X509_CRL) -> c_int;
    pub fn X509_CRL_match(a: *const X509_CRL, b: *const X509_CRL) -> c_int;
}

extern "C" {
    pub fn X509_print(bio: *mut BIO, x509: *mut X509) -> c_int;
    pub fn X509_REQ_print(bio: *mut BIO, req: *mut X509_REQ) -> c_int;
}

cfg_if! {
    if #[cfg(libressl390)] {
        pub enum X509_PURPOSE {}
    } else {
        #[repr(C)]
        pub struct X509_PURPOSE {
            pub purpose: c_int,
            pub trust: c_int, // Default trust ID
            pub flags: c_int,
            pub check_purpose:
                Option<unsafe extern "C" fn(*const X509_PURPOSE, *const X509, c_int) -> c_int>,
            pub name: *mut c_char,
            pub sname: *mut c_char,
            pub usr_data: *mut c_void,
        }
    }
}

const_ptr_api! {
    extern "C" {
        pub fn X509_PURPOSE_get_by_sname(sname: #[const_ptr_if(any(ossl110, libressl280))] c_char) -> c_int;
        pub fn X509_PURPOSE_get_id(purpose: #[const_ptr_if(any(ossl110, libressl280))] X509_PURPOSE) -> c_int;
        pub fn X509_PURPOSE_get0(idx: c_int) -> #[const_ptr_if(libressl390)] X509_PURPOSE;
    }
}

extern "C" {
    pub fn X509_ATTRIBUTE_new() -> *mut X509_ATTRIBUTE;
    pub fn X509_ATTRIBUTE_free(attr: *mut X509_ATTRIBUTE);
    pub fn X509_ATTRIBUTE_create(
        nid: c_int,
        atrtype: c_int,
        value: *mut c_void,
    ) -> *mut X509_ATTRIBUTE;
    pub fn X509_ATTRIBUTE_create_by_NID(
        attr: *mut *mut X509_ATTRIBUTE,
        nid: c_int,
        atrtype: c_int,
        data: *const c_void,
        len: c_int,
    ) -> *mut X509_ATTRIBUTE;
    pub fn X509_ATTRIBUTE_create_by_OBJ(
        attr: *mut *mut X509_ATTRIBUTE,
        obj: *const ASN1_OBJECT,
        atrtype: c_int,
        data: *const c_void,
        len: c_int,
    ) -> *mut X509_ATTRIBUTE;
    pub fn X509_ATTRIBUTE_create_by_txt(
        attr: *mut *mut X509_ATTRIBUTE,
        atrname: *const c_char,
        atrtype: c_int,
        bytes: *const c_uchar,
        len: c_int,
    ) -> *mut X509_ATTRIBUTE;
    pub fn X509_ATTRIBUTE_set1_object(attr: *mut X509_ATTRIBUTE, obj: *const ASN1_OBJECT) -> c_int;
    pub fn X509_ATTRIBUTE_set1_data(
        attr: *mut X509_ATTRIBUTE,
        attrtype: c_int,
        data: *const c_void,
        len: c_int,
    ) -> c_int;
    pub fn X509_ATTRIBUTE_get0_data(
        attr: *mut X509_ATTRIBUTE,
        idx: c_int,
        atrtype: c_int,
        data: *mut c_void,
    ) -> *mut c_void;
    pub fn X509_ATTRIBUTE_get0_object(attr: *mut X509_ATTRIBUTE) -> *mut ASN1_OBJECT;
    pub fn X509_ATTRIBUTE_get0_type(attr: *mut X509_ATTRIBUTE, idx: c_int) -> *mut ASN1_TYPE;
    pub fn d2i_X509_ATTRIBUTE(
        a: *mut *mut X509_ATTRIBUTE,
        pp: *mut *const c_uchar,
        length: c_long,
    ) -> *mut X509_ATTRIBUTE;
}
const_ptr_api! {
    extern "C" {
        pub fn X509_ATTRIBUTE_count(
            attr: #[const_ptr_if(any(ossl110, libressl280))] X509_ATTRIBUTE // const since OpenSSL v1.1.0
        ) -> c_int;
        pub fn i2d_X509_ATTRIBUTE(x: #[const_ptr_if(ossl300)] X509_ATTRIBUTE, buf: *mut *mut u8) -> c_int;
        pub fn X509_ATTRIBUTE_dup(x: #[const_ptr_if(ossl300)] X509_ATTRIBUTE) -> *mut X509_ATTRIBUTE;
    }
}
