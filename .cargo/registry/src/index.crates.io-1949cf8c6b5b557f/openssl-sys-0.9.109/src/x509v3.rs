use libc::*;

use super::*;

#[repr(C)]
pub struct GENERAL_NAME {
    pub type_: c_int,
    // FIXME should be a union
    pub d: *mut c_void,
}

stack!(stack_st_GENERAL_NAME);

pub const GEN_OTHERNAME: c_int = 0;
pub const GEN_EMAIL: c_int = 1;
pub const GEN_DNS: c_int = 2;
pub const GEN_X400: c_int = 3;
pub const GEN_DIRNAME: c_int = 4;
pub const GEN_EDIPARTY: c_int = 5;
pub const GEN_URI: c_int = 6;
pub const GEN_IPADD: c_int = 7;
pub const GEN_RID: c_int = 8;

#[cfg(any(ossl102, libressl261))]
pub const X509_CHECK_FLAG_ALWAYS_CHECK_SUBJECT: c_uint = 0x1;
#[cfg(any(ossl102, libressl261))]
pub const X509_CHECK_FLAG_NO_WILDCARDS: c_uint = 0x2;
#[cfg(any(ossl102, libressl261))]
pub const X509_CHECK_FLAG_NO_PARTIAL_WILDCARDS: c_uint = 0x4;
#[cfg(any(ossl102, libressl261))]
pub const X509_CHECK_FLAG_MULTI_LABEL_WILDCARDS: c_uint = 0x8;
#[cfg(any(ossl102, libressl261))]
pub const X509_CHECK_FLAG_SINGLE_LABEL_SUBDOMAINS: c_uint = 0x10;
#[cfg(ossl110)]
pub const X509_CHECK_FLAG_NEVER_CHECK_SUBJECT: c_uint = 0x20;

pub const X509V3_ADD_DEFAULT: c_ulong = 0;
pub const X509V3_ADD_APPEND: c_ulong = 1;
pub const X509V3_ADD_REPLACE: c_ulong = 2;
pub const X509V3_ADD_REPLACE_EXISTING: c_ulong = 3;
pub const X509V3_ADD_KEEP_EXISTING: c_ulong = 4;
pub const X509V3_ADD_DELETE: c_ulong = 5;
pub const X509V3_ADD_SILENT: c_ulong = 0x10;

pub const EXFLAG_BCONS: u32 = 0x1;
pub const EXFLAG_KUSAGE: u32 = 0x2;
pub const EXFLAG_XKUSAGE: u32 = 0x4;
pub const EXFLAG_NSCERT: u32 = 0x8;
pub const EXFLAG_CA: u32 = 0x10;
pub const EXFLAG_SI: u32 = 0x20;
pub const EXFLAG_V1: u32 = 0x40;
pub const EXFLAG_INVALID: u32 = 0x80;
pub const EXFLAG_SET: u32 = 0x100;
pub const EXFLAG_CRITICAL: u32 = 0x200;
pub const EXFLAG_PROXY: u32 = 0x400;
pub const EXFLAG_INVALID_POLICY: u32 = 0x800;
pub const EXFLAG_FRESHEST: u32 = 0x1000;
#[cfg(any(ossl102, libressl261))]
pub const EXFLAG_SS: u32 = 0x2000;

pub const X509v3_KU_DIGITAL_SIGNATURE: u32 = 0x0080;
pub const X509v3_KU_NON_REPUDIATION: u32 = 0x0040;
pub const X509v3_KU_KEY_ENCIPHERMENT: u32 = 0x0020;
pub const X509v3_KU_DATA_ENCIPHERMENT: u32 = 0x0010;
pub const X509v3_KU_KEY_AGREEMENT: u32 = 0x0008;
pub const X509v3_KU_KEY_CERT_SIGN: u32 = 0x0004;
pub const X509v3_KU_CRL_SIGN: u32 = 0x0002;
pub const X509v3_KU_ENCIPHER_ONLY: u32 = 0x0001;
pub const X509v3_KU_DECIPHER_ONLY: u32 = 0x8000;
pub const X509v3_KU_UNDEF: u32 = 0xffff;

pub const XKU_SSL_SERVER: u32 = 0x1;
pub const XKU_SSL_CLIENT: u32 = 0x2;
pub const XKU_SMIME: u32 = 0x4;
pub const XKU_CODE_SIGN: u32 = 0x8;
pub const XKU_SGC: u32 = 0x10;
pub const XKU_OCSP_SIGN: u32 = 0x20;
pub const XKU_TIMESTAMP: u32 = 0x40;
pub const XKU_DVCS: u32 = 0x80;
#[cfg(ossl110)]
pub const XKU_ANYEKU: u32 = 0x100;

pub const X509_PURPOSE_SSL_CLIENT: c_int = 1;
pub const X509_PURPOSE_SSL_SERVER: c_int = 2;
pub const X509_PURPOSE_NS_SSL_SERVER: c_int = 3;
pub const X509_PURPOSE_SMIME_SIGN: c_int = 4;
pub const X509_PURPOSE_SMIME_ENCRYPT: c_int = 5;
pub const X509_PURPOSE_CRL_SIGN: c_int = 6;
pub const X509_PURPOSE_ANY: c_int = 7;
pub const X509_PURPOSE_OCSP_HELPER: c_int = 8;
pub const X509_PURPOSE_TIMESTAMP_SIGN: c_int = 9;
#[cfg(ossl320)]
pub const X509_PURPOSE_CODE_SIGN: c_int = 10;
pub const X509_PURPOSE_MIN: c_int = 1;
cfg_if! {
    if #[cfg(ossl320)] {
        pub const X509_PURPOSE_MAX: c_int = 10;
    } else {
        pub const X509_PURPOSE_MAX: c_int = 9;
    }
}

pub const CRL_REASON_UNSPECIFIED: c_int = 0;
pub const CRL_REASON_KEY_COMPROMISE: c_int = 1;
pub const CRL_REASON_CA_COMPROMISE: c_int = 2;
pub const CRL_REASON_AFFILIATION_CHANGED: c_int = 3;
pub const CRL_REASON_SUPERSEDED: c_int = 4;
pub const CRL_REASON_CESSATION_OF_OPERATION: c_int = 5;
pub const CRL_REASON_CERTIFICATE_HOLD: c_int = 6;
pub const CRL_REASON_REMOVE_FROM_CRL: c_int = 8;
pub const CRL_REASON_PRIVILEGE_WITHDRAWN: c_int = 9;
pub const CRL_REASON_AA_COMPROMISE: c_int = 10;
