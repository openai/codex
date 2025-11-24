//! Empty implementation of unwinding used when no other implementation is
//! appropriate.

use core::ffi::c_void;
use core::ptr::null_mut;

// SAFETY: This function is safe to call. It is only marked as `unsafe` to
// avoid having to allow `unused_unsafe` since other implementations are
// unsafe.
#[inline(always)]
pub unsafe fn trace(_cb: &mut dyn FnMut(&super::Frame) -> bool) {}

#[derive(Clone)]
pub struct Frame;

impl Frame {
    pub fn ip(&self) -> *mut c_void {
        null_mut()
    }

    pub fn sp(&self) -> *mut c_void {
        null_mut()
    }

    pub fn symbol_address(&self) -> *mut c_void {
        null_mut()
    }

    pub fn module_base_address(&self) -> Option<*mut c_void> {
        None
    }
}
