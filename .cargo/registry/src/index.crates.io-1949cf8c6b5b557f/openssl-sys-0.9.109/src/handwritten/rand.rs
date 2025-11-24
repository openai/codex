use libc::*;

extern "C" {
    pub fn RAND_bytes(buf: *mut u8, num: c_int) -> c_int;

    #[cfg(ossl111)]
    pub fn RAND_priv_bytes(buf: *mut u8, num: c_int) -> c_int;

    #[cfg(ossl111)]
    pub fn RAND_keep_random_devices_open(keep: c_int);

    pub fn RAND_status() -> c_int;

    pub fn RAND_add(buf: *const c_void, num: c_int, randomness: c_double);
}
