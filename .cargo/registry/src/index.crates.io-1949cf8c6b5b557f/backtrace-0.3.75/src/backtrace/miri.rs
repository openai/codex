use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::c_void;

unsafe extern "Rust" {
    fn miri_backtrace_size(flags: u64) -> usize;
    fn miri_get_backtrace(flags: u64, buf: *mut *mut ());
    fn miri_resolve_frame(ptr: *mut (), flags: u64) -> MiriFrame;
    fn miri_resolve_frame_names(ptr: *mut (), flags: u64, name_buf: *mut u8, filename_buf: *mut u8);
}

#[repr(C)]
pub struct MiriFrame {
    pub name_len: usize,
    pub filename_len: usize,
    pub lineno: u32,
    pub colno: u32,
    pub fn_ptr: *mut c_void,
}

#[derive(Clone, Debug)]
pub struct FullMiriFrame {
    pub name: Box<[u8]>,
    pub filename: Box<[u8]>,
    pub lineno: u32,
    pub colno: u32,
    pub fn_ptr: *mut c_void,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub addr: *mut c_void,
    pub inner: FullMiriFrame,
}

// SAFETY: Miri guarantees that the returned pointer
// can be used from any thread.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    pub fn ip(&self) -> *mut c_void {
        self.addr
    }

    pub fn sp(&self) -> *mut c_void {
        core::ptr::null_mut()
    }

    pub fn symbol_address(&self) -> *mut c_void {
        self.inner.fn_ptr
    }

    pub fn module_base_address(&self) -> Option<*mut c_void> {
        None
    }
}

// SAFETY: This function is safe to call. It is only marked as `unsafe` to
// avoid having to allow `unused_unsafe` since other implementations are
// unsafe.
pub unsafe fn trace<F: FnMut(&super::Frame) -> bool>(cb: F) {
    // SAFETY: Miri guarantees that the backtrace API functions
    // can be called from any thread.
    unsafe { trace_unsynchronized(cb) };
}

pub fn resolve_addr(ptr: *mut c_void) -> Frame {
    // SAFETY: Miri will stop execution with an error if this pointer
    // is invalid.
    let frame = unsafe { miri_resolve_frame(ptr.cast::<()>(), 1) };

    let mut name = Vec::with_capacity(frame.name_len);
    let mut filename = Vec::with_capacity(frame.filename_len);

    // SAFETY: name and filename have been allocated with the amount
    // of memory miri has asked for, and miri guarantees it will initialize it
    unsafe {
        miri_resolve_frame_names(
            ptr.cast::<()>(),
            0,
            name.as_mut_ptr(),
            filename.as_mut_ptr(),
        );

        name.set_len(frame.name_len);
        filename.set_len(frame.filename_len);
    }

    Frame {
        addr: ptr,
        inner: FullMiriFrame {
            name: name.into(),
            filename: filename.into(),
            lineno: frame.lineno,
            colno: frame.colno,
            fn_ptr: frame.fn_ptr,
        },
    }
}

unsafe fn trace_unsynchronized<F: FnMut(&super::Frame) -> bool>(mut cb: F) {
    let len = unsafe { miri_backtrace_size(0) };

    let mut frames = Vec::with_capacity(len);

    unsafe {
        miri_get_backtrace(1, frames.as_mut_ptr());

        frames.set_len(len);
    }

    for ptr in frames.iter() {
        let frame = resolve_addr((*ptr).cast::<c_void>());
        if !cb(&super::Frame { inner: frame }) {
            return;
        }
    }
}
