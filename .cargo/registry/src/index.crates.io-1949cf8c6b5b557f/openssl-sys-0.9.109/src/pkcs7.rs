use libc::*;

pub const PKCS7_TEXT: c_int = 0x1;
pub const PKCS7_NOCERTS: c_int = 0x2;
pub const PKCS7_NOSIGS: c_int = 0x4;
pub const PKCS7_NOCHAIN: c_int = 0x8;
pub const PKCS7_NOINTERN: c_int = 0x10;
pub const PKCS7_NOVERIFY: c_int = 0x20;
pub const PKCS7_DETACHED: c_int = 0x40;
pub const PKCS7_BINARY: c_int = 0x80;
pub const PKCS7_NOATTR: c_int = 0x100;
pub const PKCS7_NOSMIMECAP: c_int = 0x200;
pub const PKCS7_NOOLDMIMETYPE: c_int = 0x400;
pub const PKCS7_CRLFEOL: c_int = 0x800;
pub const PKCS7_STREAM: c_int = 0x1000;
pub const PKCS7_NOCRL: c_int = 0x2000;
pub const PKCS7_PARTIAL: c_int = 0x4000;
pub const PKCS7_REUSE_DIGEST: c_int = 0x8000;
#[cfg(not(any(ossl101, ossl102, libressl)))]
pub const PKCS7_NO_DUAL_CONTENT: c_int = 0x10000;
