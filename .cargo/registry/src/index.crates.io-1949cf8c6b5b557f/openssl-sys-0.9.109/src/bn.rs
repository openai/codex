use libc::*;

#[cfg(target_pointer_width = "64")]
pub type BN_ULONG = c_ulonglong;
#[cfg(target_pointer_width = "32")]
pub type BN_ULONG = c_uint;

#[cfg(ossl110)]
pub const BN_FLG_MALLOCED: c_int = 0x01;
#[cfg(ossl110)]
pub const BN_FLG_STATIC_DATA: c_int = 0x02;
#[cfg(ossl110)]
pub const BN_FLG_CONSTTIME: c_int = 0x04;
#[cfg(ossl110)]
pub const BN_FLG_SECURE: c_int = 0x08;
