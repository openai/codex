//! Various types and functions that are used by both the client and the server
//! libraries.

use std::os::unix::io::RawFd;
use std::{
    fmt,
    os::raw::{c_char, c_int, c_void},
};

#[repr(C)]
pub struct wl_message {
    pub name: *const c_char,
    pub signature: *const c_char,
    pub types: *const *const wl_interface,
}

#[repr(C)]
pub struct wl_interface {
    pub name: *const c_char,
    pub version: c_int,
    pub request_count: c_int,
    pub requests: *const wl_message,
    pub event_count: c_int,
    pub events: *const wl_message,
}

impl fmt::Debug for wl_interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wl_interface@{self:p}")
    }
}

unsafe impl Send for wl_interface {}
unsafe impl Sync for wl_interface {}

#[repr(C)]
pub struct wl_list {
    pub prev: *mut wl_list,
    pub next: *mut wl_list,
}

#[repr(C)]
pub struct wl_array {
    pub size: usize,
    pub alloc: usize,
    pub data: *mut c_void,
}

pub type wl_fixed_t = i32;

pub fn wl_fixed_to_double(f: wl_fixed_t) -> f64 {
    f64::from(f) / 256.
}

pub fn wl_fixed_from_double(d: f64) -> wl_fixed_t {
    (d * 256.) as i32
}

pub fn wl_fixed_to_int(f: wl_fixed_t) -> i32 {
    f / 256
}

pub fn wl_fixed_from_int(i: i32) -> wl_fixed_t {
    i * 256
}

// must be the appropriate size
// can contain i32, u32 and pointers
#[repr(C)]
pub union wl_argument {
    pub i: i32,
    pub u: u32,
    pub f: wl_fixed_t,
    pub s: *const c_char,
    pub o: *const c_void,
    pub n: u32,
    pub a: *const wl_array,
    pub h: RawFd,
}

pub type wl_dispatcher_func_t = unsafe extern "C" fn(
    *const c_void,
    *mut c_void,
    u32,
    *const wl_message,
    *const wl_argument,
) -> c_int;

pub type wl_log_func_t = unsafe extern "C" fn(*const c_char, *const c_void);
