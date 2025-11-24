use super::super::*;
use libc::*;

extern "C" {
    pub fn BIO_set_flags(b: *mut BIO, flags: c_int);
    pub fn BIO_clear_flags(b: *mut BIO, flags: c_int);
}

pub type bio_info_cb =
    Option<unsafe extern "C" fn(*mut BIO, c_int, *const c_char, c_int, c_long, c_long)>;

cfg_if! {
    if #[cfg(any(ossl110, libressl280))] {
        pub enum BIO_METHOD {}
    } else {
        #[repr(C)]
        pub struct BIO_METHOD {
            pub type_: c_int,
            pub name: *const c_char,
            pub bwrite: Option<unsafe extern "C" fn(*mut BIO, *const c_char, c_int) -> c_int>,
            pub bread: Option<unsafe extern "C" fn(*mut BIO, *mut c_char, c_int) -> c_int>,
            pub bputs: Option<unsafe extern "C" fn(*mut BIO, *const c_char) -> c_int>,
            pub bgets: Option<unsafe extern "C" fn(*mut BIO, *mut c_char, c_int) -> c_int>,
            pub ctrl: Option<unsafe extern "C" fn(*mut BIO, c_int, c_long, *mut c_void) -> c_long>,
            pub create: Option<unsafe extern "C" fn(*mut BIO) -> c_int>,
            pub destroy: Option<unsafe extern "C" fn(*mut BIO) -> c_int>,
            pub callback_ctrl: Option<unsafe extern "C" fn(*mut BIO, c_int, bio_info_cb) -> c_long>,
        }
    }
}

const_ptr_api! {
    extern "C" {
        pub fn BIO_s_file() -> #[const_ptr_if(any(ossl110, libressl280))] BIO_METHOD;
        pub fn BIO_new(type_: #[const_ptr_if(any(ossl110, libressl280))] BIO_METHOD) -> *mut BIO;
    }
}
extern "C" {
    #[cfg(not(osslconf = "OPENSSL_NO_STDIO"))]
    pub fn BIO_new_fp(stream: *mut FILE, close_flag: c_int) -> *mut BIO;
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_set_data(a: *mut BIO, data: *mut c_void);
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_get_data(a: *mut BIO) -> *mut c_void;
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_set_init(a: *mut BIO, init: c_int);
    pub fn BIO_write(b: *mut BIO, buf: *const c_void, len: c_int) -> c_int;
    pub fn BIO_read(b: *mut BIO, buf: *mut c_void, len: c_int) -> c_int;
    pub fn BIO_ctrl(b: *mut BIO, cmd: c_int, larg: c_long, parg: *mut c_void) -> c_long;
    pub fn BIO_free_all(b: *mut BIO);
}

const_ptr_api! {
    extern "C" {
        pub fn BIO_s_mem() -> #[const_ptr_if(any(ossl110, libressl280))] BIO_METHOD;
        pub fn BIO_new_mem_buf(buf: #[const_ptr_if(any(ossl102, libressl280))] c_void, len: c_int) -> *mut BIO;
    }
}

extern "C" {
    #[cfg(not(osslconf = "OPENSSL_NO_SOCK"))]
    pub fn BIO_new_socket(sock: c_int, close_flag: c_int) -> *mut BIO;

    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_new(type_: c_int, name: *const c_char) -> *mut BIO_METHOD;
    #[cfg(any(ossl110, libressl273))]
    pub fn BIO_meth_free(biom: *mut BIO_METHOD);
}

#[allow(clashing_extern_declarations)]
extern "C" {
    #[cfg(any(ossl110, libressl273))]
    #[link_name = "BIO_meth_set_write"]
    pub fn BIO_meth_set_write__fixed_rust(
        biom: *mut BIO_METHOD,
        write: Option<unsafe extern "C" fn(*mut BIO, *const c_char, c_int) -> c_int>,
    ) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    #[link_name = "BIO_meth_set_read"]
    pub fn BIO_meth_set_read__fixed_rust(
        biom: *mut BIO_METHOD,
        read: Option<unsafe extern "C" fn(*mut BIO, *mut c_char, c_int) -> c_int>,
    ) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    #[link_name = "BIO_meth_set_puts"]
    pub fn BIO_meth_set_puts__fixed_rust(
        biom: *mut BIO_METHOD,
        read: Option<unsafe extern "C" fn(*mut BIO, *const c_char) -> c_int>,
    ) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    #[link_name = "BIO_meth_set_ctrl"]
    pub fn BIO_meth_set_ctrl__fixed_rust(
        biom: *mut BIO_METHOD,
        read: Option<unsafe extern "C" fn(*mut BIO, c_int, c_long, *mut c_void) -> c_long>,
    ) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    #[link_name = "BIO_meth_set_create"]
    pub fn BIO_meth_set_create__fixed_rust(
        biom: *mut BIO_METHOD,
        create: Option<unsafe extern "C" fn(*mut BIO) -> c_int>,
    ) -> c_int;
    #[cfg(any(ossl110, libressl273))]
    #[link_name = "BIO_meth_set_destroy"]
    pub fn BIO_meth_set_destroy__fixed_rust(
        biom: *mut BIO_METHOD,
        destroy: Option<unsafe extern "C" fn(*mut BIO) -> c_int>,
    ) -> c_int;
}

#[cfg(ossl320)]
extern "C" {
    pub fn BIO_meth_set_sendmmsg(
        biom: *mut BIO_METHOD,
        f: Option<
            unsafe extern "C" fn(
                arg1: *mut BIO,
                arg2: *mut BIO_MSG,
                arg3: usize,
                arg4: usize,
                arg5: u64,
                arg6: *mut usize,
            ) -> c_int,
        >,
    ) -> c_int;
    pub fn BIO_meth_set_recvmmsg(
        biom: *mut BIO_METHOD,
        f: Option<
            unsafe extern "C" fn(
                arg1: *mut BIO,
                arg2: *mut BIO_MSG,
                arg3: usize,
                arg4: usize,
                arg5: u64,
                arg6: *mut usize,
            ) -> c_int,
        >,
    ) -> c_int;
    pub fn BIO_new_bio_dgram_pair(
        bio1: *mut *mut BIO,
        writebuf1: usize,
        bio2: *mut *mut BIO,
        writebuf2: usize,
    ) -> c_int;
    pub fn BIO_s_dgram_pair() -> *const BIO_METHOD;
    pub fn BIO_s_datagram() -> *const BIO_METHOD;
    pub fn BIO_get_rpoll_descriptor(b: *mut BIO, desc: *mut BIO_POLL_DESCRIPTOR) -> c_int;
    pub fn BIO_get_wpoll_descriptor(b: *mut BIO, desc: *mut BIO_POLL_DESCRIPTOR) -> c_int;
    pub fn BIO_sendmmsg(
        b: *mut BIO,
        msg: *mut BIO_MSG,
        stride: usize,
        num_msg: usize,
        flags: u64,
        msgs_processed: *mut usize,
    ) -> c_int;
    pub fn BIO_recvmmsg(
        b: *mut BIO,
        msg: *mut BIO_MSG,
        stride: usize,
        num_msg: usize,
        flags: u64,
        msgs_processed: *mut usize,
    ) -> c_int;
    pub fn BIO_err_is_non_fatal(errcode: c_uint) -> c_int;
}
