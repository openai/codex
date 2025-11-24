use libc::*;

use super::*;

pub const BIO_TYPE_NONE: c_int = 0;

pub const BIO_CTRL_EOF: c_int = 2;
pub const BIO_CTRL_INFO: c_int = 3;
pub const BIO_CTRL_FLUSH: c_int = 11;
pub const BIO_CTRL_DGRAM_QUERY_MTU: c_int = 40;
pub const BIO_C_SET_BUF_MEM_EOF_RETURN: c_int = 130;

pub unsafe fn BIO_set_retry_read(b: *mut BIO) {
    BIO_set_flags(b, BIO_FLAGS_READ | BIO_FLAGS_SHOULD_RETRY)
}

pub unsafe fn BIO_set_retry_write(b: *mut BIO) {
    BIO_set_flags(b, BIO_FLAGS_WRITE | BIO_FLAGS_SHOULD_RETRY)
}

pub unsafe fn BIO_clear_retry_flags(b: *mut BIO) {
    BIO_clear_flags(b, BIO_FLAGS_RWS | BIO_FLAGS_SHOULD_RETRY)
}

pub const BIO_FLAGS_READ: c_int = 0x01;
pub const BIO_FLAGS_WRITE: c_int = 0x02;
pub const BIO_FLAGS_IO_SPECIAL: c_int = 0x04;
pub const BIO_FLAGS_RWS: c_int = BIO_FLAGS_READ | BIO_FLAGS_WRITE | BIO_FLAGS_IO_SPECIAL;
pub const BIO_FLAGS_SHOULD_RETRY: c_int = 0x08;

pub unsafe fn BIO_get_mem_data(b: *mut BIO, pp: *mut *mut c_char) -> c_long {
    BIO_ctrl(b, BIO_CTRL_INFO, 0, pp as *mut c_void)
}

extern "C" {
    #[deprecated(note = "use BIO_meth_set_write__fixed_rust instead")]
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_set_write(
        biom: *mut BIO_METHOD,
        write: unsafe extern "C" fn(*mut BIO, *const c_char, c_int) -> c_int,
    ) -> c_int;
    #[deprecated(note = "use BIO_meth_set_read__fixed_rust instead")]
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_set_read(
        biom: *mut BIO_METHOD,
        read: unsafe extern "C" fn(*mut BIO, *mut c_char, c_int) -> c_int,
    ) -> c_int;
    #[deprecated(note = "use BIO_meth_set_puts__fixed_rust instead")]
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_set_puts(
        biom: *mut BIO_METHOD,
        read: unsafe extern "C" fn(*mut BIO, *const c_char) -> c_int,
    ) -> c_int;
    #[deprecated(note = "use BIO_meth_set_ctrl__fixed_rust instead")]
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_set_ctrl(
        biom: *mut BIO_METHOD,
        read: unsafe extern "C" fn(*mut BIO, c_int, c_long, *mut c_void) -> c_long,
    ) -> c_int;
    #[deprecated(note = "use BIO_meth_set_create__fixed_rust instead")]
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_set_create(
        biom: *mut BIO_METHOD,
        create: unsafe extern "C" fn(*mut BIO) -> c_int,
    ) -> c_int;
    #[deprecated(note = "use BIO_meth_set_destroy__fixed_rust instead")]
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_set_destroy(
        biom: *mut BIO_METHOD,
        destroy: unsafe extern "C" fn(*mut BIO) -> c_int,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(ossl320)] {
        use std::ptr;

        pub const BIO_CTRL_DGRAM_GET_MTU: c_int = 41;
        pub const BIO_CTRL_DGRAM_SET_MTU: c_int = 42;
        pub const BIO_CTRL_DGRAM_GET_LOCAL_ADDR_CAP: c_int = 82;
        pub const BIO_CTRL_DGRAM_GET_LOCAL_ADDR_ENABLE: c_int = 83;
        pub const BIO_CTRL_DGRAM_SET_LOCAL_ADDR_ENABLE: c_int = 84;
        pub const BIO_CTRL_DGRAM_GET_CAPS: c_int = 86;
        pub const BIO_CTRL_DGRAM_SET_CAPS: c_int = 87;
        pub const BIO_CTRL_DGRAM_GET_NO_TRUNC: c_int = 88;
        pub const BIO_CTRL_DGRAM_SET_NO_TRUNC: c_int = 89;

        pub unsafe fn BIO_dgram_get_no_trunc(bio: *mut BIO) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_GET_NO_TRUNC, 0, ptr::null_mut()) as c_int
        }
        pub unsafe fn BIO_dgram_set_no_trunc(bio: *mut BIO, enable: c_int) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_SET_NO_TRUNC, enable as c_long, ptr::null_mut()) as c_int
        }
        pub unsafe fn BIO_dgram_get_cap(bio: *mut BIO) -> u32 {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_GET_CAPS, 0, ptr::null_mut()) as u32
        }
        pub unsafe fn BIO_dgram_set_cap(bio: *mut BIO, cap: u32) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_SET_CAPS, cap as c_long, ptr::null_mut()) as c_int
        }
        pub unsafe fn BIO_dgram_get_local_addr_cap(bio: *mut BIO) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_GET_LOCAL_ADDR_CAP, 0, ptr::null_mut()) as c_int
        }
        pub unsafe fn BIO_dgram_get_local_addr_enable(bio: *mut BIO, enable: *mut c_int) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_GET_LOCAL_ADDR_ENABLE, 0, enable as *mut c_void) as c_int
        }
        pub unsafe fn BIO_dgram_set_local_addr_enable(bio: *mut BIO, enable: c_int) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_SET_LOCAL_ADDR_ENABLE, enable as c_long, ptr::null_mut()) as c_int
        }
        pub unsafe fn BIO_dgram_get_mtu(bio: *mut BIO) -> c_uint {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_GET_MTU, 0, ptr::null_mut()) as c_uint
        }
        pub unsafe fn BIO_dgram_set_mtu(bio: *mut BIO, mtu: c_uint) -> c_int {
            BIO_ctrl(bio, BIO_CTRL_DGRAM_SET_MTU, mtu as c_long, ptr::null_mut()) as c_int
        }
    }
}
