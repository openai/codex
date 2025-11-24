//! A mock of libxcb that is used by the unit tests in [`x11rb::xcb_ffi`].

use std::ffi::CStr;

use libc::{c_char, c_int, c_uint, c_void};

use super::{
    iovec, xcb_connection_t, xcb_generic_error_t, xcb_generic_event_t, xcb_protocol_request_t,
    xcb_setup_t, xcb_void_cookie_t,
};
use crate::protocol::xproto::{ImageOrder, Setup};
use crate::x11_utils::Serialize;

#[repr(C)]
struct ConnectionMock {
    error: c_int,
    setup: Vec<u8>,
}

// From xcb.h
pub(crate) unsafe fn xcb_flush(_c: *mut xcb_connection_t) -> c_int {
    unimplemented!();
}

pub(crate) unsafe fn xcb_get_maximum_request_length(_c: *mut xcb_connection_t) -> u32 {
    unimplemented!();
}

pub(crate) unsafe fn xcb_prefetch_maximum_request_length(_c: *mut xcb_connection_t) {
    unimplemented!();
}

pub(crate) unsafe fn xcb_wait_for_event(_c: *mut xcb_connection_t) -> *mut xcb_generic_event_t {
    unimplemented!();
}

pub(crate) unsafe fn xcb_poll_for_event(_c: *mut xcb_connection_t) -> *mut xcb_generic_event_t {
    unimplemented!();
}

pub(crate) unsafe fn xcb_request_check(
    _c: *mut xcb_connection_t,
    _void_cookie: xcb_void_cookie_t,
) -> *mut xcb_generic_error_t {
    unimplemented!();
}

pub(crate) unsafe fn xcb_discard_reply64(_c: *mut xcb_connection_t, _sequence: u64) {
    unimplemented!();
}

pub(crate) unsafe fn xcb_get_setup(c: *mut xcb_connection_t) -> *const xcb_setup_t {
    // The pointer is suitable aligned since our xcb_connect() mock above created it
    #[allow(clippy::cast_ptr_alignment)]
    ((*(c as *const ConnectionMock)).setup.as_ptr() as _)
}

#[cfg(unix)]
pub(crate) unsafe fn xcb_get_file_descriptor(_c: *mut xcb_connection_t) -> c_int {
    unimplemented!();
}

pub(crate) unsafe fn xcb_connection_has_error(c: *mut xcb_connection_t) -> c_int {
    // The pointer is suitable aligned since our xcb_connect() mock above created it
    #[allow(clippy::cast_ptr_alignment)]
    (*(c as *const ConnectionMock)).error
}

pub(crate) unsafe fn xcb_disconnect(c: *mut xcb_connection_t) {
    // The pointer is suitable aligned since our xcb_connect() mock above created it
    #[allow(clippy::cast_ptr_alignment)]
    let _ = Box::from_raw(c as *mut ConnectionMock);
}

pub(crate) unsafe fn xcb_connect(
    displayname: *const c_char,
    screenp: *mut c_int,
) -> *mut xcb_connection_t {
    // Test that the provided displayname is correct
    assert_eq!(
        CStr::from_ptr(displayname).to_str().unwrap(),
        "display name",
        "Did not get the expected displayname",
    );
    std::ptr::write(screenp, 0);

    let length_field = 10;
    let setup = Setup {
        status: 0,
        protocol_major_version: 0,
        protocol_minor_version: 0,
        length: length_field,
        release_number: 0,
        resource_id_base: 0,
        resource_id_mask: 0,
        motion_buffer_size: 0,
        maximum_request_length: 0,
        image_byte_order: ImageOrder::LSB_FIRST,
        bitmap_format_bit_order: ImageOrder::LSB_FIRST,
        bitmap_format_scanline_unit: 0,
        bitmap_format_scanline_pad: 0,
        min_keycode: 0,
        max_keycode: 0,
        vendor: Default::default(),
        pixmap_formats: Default::default(),
        roots: Default::default(),
    };
    let setup = setup.serialize();
    assert_eq!(setup.len(), 4 * length_field as usize);

    let mock = ConnectionMock { error: 0, setup };
    Box::into_raw(Box::new(mock)) as _
}

pub(crate) unsafe fn xcb_generate_id(_c: *mut xcb_connection_t) -> u32 {
    unimplemented!();
}

// From xcbext.h
pub(crate) unsafe fn xcb_send_request64(
    _c: *mut xcb_connection_t,
    _flags: c_int,
    _vector: *mut iovec,
    _request: *const xcb_protocol_request_t,
) -> u64 {
    unimplemented!();
}

#[cfg(unix)]
pub(crate) unsafe fn xcb_send_request_with_fds64(
    _c: *mut xcb_connection_t,
    _flags: c_int,
    _vector: *mut iovec,
    _request: *const xcb_protocol_request_t,
    _num_fds: c_uint,
    _fds: *mut c_int,
) -> u64 {
    unimplemented!();
}

pub(crate) unsafe fn xcb_wait_for_reply64(
    _c: *mut xcb_connection_t,
    _request: u64,
    _e: *mut *mut xcb_generic_error_t,
) -> *mut c_void {
    unimplemented!();
}

pub(crate) unsafe fn xcb_poll_for_reply64(
    _c: *mut xcb_connection_t,
    _request: u64,
    _reply: *mut *mut c_void,
    _error: *mut *mut xcb_generic_error_t,
) -> c_int {
    unimplemented!();
}
