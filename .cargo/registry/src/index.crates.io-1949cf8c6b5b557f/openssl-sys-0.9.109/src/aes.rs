use libc::*;

pub const AES_ENCRYPT: c_int = 1;
pub const AES_DECRYPT: c_int = 0;

pub const AES_MAXNR: c_int = 14;
pub const AES_BLOCK_SIZE: c_int = 16;
