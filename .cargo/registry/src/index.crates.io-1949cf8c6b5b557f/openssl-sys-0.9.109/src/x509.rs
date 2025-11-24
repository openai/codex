use libc::*;

pub const X509_FILETYPE_PEM: c_int = 1;
pub const X509_FILETYPE_ASN1: c_int = 2;
pub const X509_FILETYPE_DEFAULT: c_int = 3;

pub const ASN1_R_HEADER_TOO_LONG: c_int = 123;

cfg_if! {
    if #[cfg(not(any(ossl110, libressl350)))] {
        pub const X509_LU_FAIL: c_int = 0;
        pub const X509_LU_X509: c_int = 1;
        pub const X509_LU_CRL: c_int = 2;
    }
}
