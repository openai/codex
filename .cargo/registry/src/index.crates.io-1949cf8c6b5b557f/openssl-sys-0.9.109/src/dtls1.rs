use libc::*;

cfg_if! {
    if #[cfg(ossl300)] {
        pub const DTLS1_COOKIE_LENGTH: c_uint = 255;
    } else {
        pub const DTLS1_COOKIE_LENGTH: c_uint = 256;
    }
}
