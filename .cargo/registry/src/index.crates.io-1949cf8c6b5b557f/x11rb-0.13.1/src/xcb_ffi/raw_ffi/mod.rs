//! The low-level foreign function interface to interact with libxcb.
//!
//! This module contains some `#[repr(C)]` type definitions that match libxcb's definitions. The
//! actual functions are defined in the `ffi` submodule. There is also a `test` submodule that
//! contains a mock of the interface that is used for unit tests.

use std::ptr::NonNull;

#[cfg(not(all(test, unix)))]
use libc::c_void;
#[cfg(unix)]
pub(crate) use libc::iovec;
use libc::{c_char, c_int, c_uint};

// As defined in xcb_windefs.h
#[cfg(not(unix))]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub(crate) struct iovec {
    pub(crate) iov_base: *mut c_void,
    pub(crate) iov_len: c_int,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_connection_t {
    _unused: [u8; 0],
}

#[derive(Debug)]
pub(crate) struct XcbConnectionWrapper {
    ptr: NonNull<xcb_connection_t>,
    should_drop: bool,
}

// libxcb is fully thread-safe (well, except for xcb_disconnect()), so the following is
// actually fine and safe:
unsafe impl Send for XcbConnectionWrapper {}
unsafe impl Sync for XcbConnectionWrapper {}

impl Drop for XcbConnectionWrapper {
    fn drop(&mut self) {
        if self.should_drop {
            unsafe {
                xcb_disconnect(self.ptr.as_ptr());
            }
        }
    }
}

impl XcbConnectionWrapper {
    pub(crate) unsafe fn new(ptr: *mut xcb_connection_t, should_drop: bool) -> Self {
        Self {
            ptr: NonNull::new_unchecked(ptr),
            should_drop,
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut xcb_connection_t {
        self.ptr.as_ptr()
    }
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_generic_event_t {
    pub(crate) response_type: u8,
    pub(crate) pad0: u8,
    pub(crate) sequence: u16,
    pub(crate) pad: [u32; 7],
    pub(crate) full_sequence: u32,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_generic_error_t {
    pub(crate) response_type: u8,
    pub(crate) error_code: u8,
    pub(crate) sequence: u16,
    pub(crate) resource_id: u32,
    pub(crate) minor_code: u16,
    pub(crate) major_code: u8,
    pub(crate) pad0: u8,
    pub(crate) pad: [u32; 5],
    pub(crate) full_sequence: u32,
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_void_cookie_t {
    pub(crate) sequence: c_uint,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_extension_t {
    pub(crate) name: *const c_char,
    pub(crate) global_id: c_int,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_protocol_request_t {
    pub(crate) count: usize,
    pub(crate) ext: *mut xcb_extension_t,
    pub(crate) opcode: u8,
    pub(crate) isvoid: u8,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub(crate) struct xcb_setup_t {
    _unused: [u8; 0],
}

pub(crate) mod connection_errors {
    use std::os::raw::c_int;

    pub(crate) const ERROR: c_int = 1;
    pub(crate) const EXT_NOTSUPPORTED: c_int = 2;
    pub(crate) const MEM_INSUFFICIENT: c_int = 3;
    pub(crate) const REQ_LEN_EXCEED: c_int = 4;
    pub(crate) const PARSE_ERR: c_int = 5;
    pub(crate) const INVALID_SCREEN: c_int = 6;
    pub(crate) const FDPASSING_FAILED: c_int = 7;
}

pub(crate) mod send_request_flags {
    use libc::c_int;

    pub(crate) const CHECKED: c_int = 1;
    pub(crate) const RAW: c_int = 2;
    //pub(crate) const DISCARD_REPLY: c_int = 4;
    pub(crate) const REPLY_FDS: c_int = 8;
}

#[cfg(not(test))]
mod ffi;

#[cfg(not(test))]
pub(crate) use ffi::*;

#[cfg(test)]
mod test;

#[cfg(test)]
pub(crate) use test::*;
