use core::ffi::c_void;
use core::fmt;

/// Inspects the current call-stack, passing all active frames into the closure
/// provided to calculate a stack trace.
///
/// This function is the workhorse of this library in calculating the stack
/// traces for a program. The given closure `cb` is yielded instances of a
/// `Frame` which represent information about that call frame on the stack. The
/// closure is yielded frames in a top-down fashion (most recently called
/// functions first).
///
/// The closure's return value is an indication of whether the backtrace should
/// continue. A return value of `false` will terminate the backtrace and return
/// immediately.
///
/// Once a `Frame` is acquired you will likely want to call `backtrace::resolve`
/// to convert the `ip` (instruction pointer) or symbol address to a `Symbol`
/// through which the name and/or filename/line number can be learned.
///
/// Note that this is a relatively low-level function and if you'd like to, for
/// example, capture a backtrace to be inspected later, then the `Backtrace`
/// type may be more appropriate.
///
/// # Required features
///
/// This function requires the `std` feature of the `backtrace` crate to be
/// enabled, and the `std` feature is enabled by default.
///
/// # Panics
///
/// This function strives to never panic, but if the `cb` provided panics then
/// some platforms will force a double panic to abort the process. Some
/// platforms use a C library which internally uses callbacks which cannot be
/// unwound through, so panicking from `cb` may trigger a process abort.
///
/// # Example
///
/// ```
/// extern crate backtrace;
///
/// fn main() {
///     backtrace::trace(|frame| {
///         // ...
///
///         true // continue the backtrace
///     });
/// }
/// ```
#[cfg(feature = "std")]
pub fn trace<F: FnMut(&Frame) -> bool>(cb: F) {
    let _guard = crate::lock::lock();
    unsafe { trace_unsynchronized(cb) }
}

/// Same as `trace`, only unsafe as it's unsynchronized.
///
/// This function does not have synchronization guarantees but is available
/// when the `std` feature of this crate isn't compiled in. See the `trace`
/// function for more documentation and examples.
///
/// # Panics
///
/// See information on `trace` for caveats on `cb` panicking.
pub unsafe fn trace_unsynchronized<F: FnMut(&Frame) -> bool>(mut cb: F) {
    unsafe { trace_imp(&mut cb) }
}

/// A trait representing one frame of a backtrace, yielded to the `trace`
/// function of this crate.
///
/// The tracing function's closure will be yielded frames, and the frame is
/// virtually dispatched as the underlying implementation is not always known
/// until runtime.
#[derive(Clone)]
pub struct Frame {
    pub(crate) inner: FrameImp,
}

impl Frame {
    /// Returns the current instruction pointer of this frame.
    ///
    /// This is normally the next instruction to execute in the frame, but not
    /// all implementations list this with 100% accuracy (but it's generally
    /// pretty close).
    ///
    /// It is recommended to pass this value to `backtrace::resolve` to turn it
    /// into a symbol name.
    pub fn ip(&self) -> *mut c_void {
        self.inner.ip()
    }

    /// Returns the current stack pointer of this frame.
    ///
    /// In the case that a backend cannot recover the stack pointer for this
    /// frame, a null pointer is returned.
    pub fn sp(&self) -> *mut c_void {
        self.inner.sp()
    }

    /// Returns the starting symbol address of the frame of this function.
    ///
    /// This will attempt to rewind the instruction pointer returned by `ip` to
    /// the start of the function, returning that value. In some cases, however,
    /// backends will just return `ip` from this function.
    ///
    /// The returned value can sometimes be used if `backtrace::resolve` failed
    /// on the `ip` given above.
    pub fn symbol_address(&self) -> *mut c_void {
        self.inner.symbol_address()
    }

    /// Returns the base address of the module to which the frame belongs.
    pub fn module_base_address(&self) -> Option<*mut c_void> {
        self.inner.module_base_address()
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("ip", &self.ip())
            .field("symbol_address", &self.symbol_address())
            .finish()
    }
}

#[cfg(all(target_env = "sgx", target_vendor = "fortanix"))]
mod sgx_image_base {

    #[cfg(not(feature = "std"))]
    pub(crate) mod imp {
        use core::ffi::c_void;
        use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};

        static IMAGE_BASE: AtomicUsize = AtomicUsize::new(0);

        /// Set the image base address. This is only available for Fortanix SGX
        /// target when the `std` feature is not enabled. This can be used in the
        /// standard library to set the correct base address.
        #[doc(hidden)]
        pub fn set_image_base(base_addr: *mut c_void) {
            IMAGE_BASE.store(base_addr as _, SeqCst);
        }

        pub(crate) fn get_image_base() -> *mut c_void {
            IMAGE_BASE.load(SeqCst) as _
        }
    }

    #[cfg(feature = "std")]
    mod imp {
        use core::ffi::c_void;

        pub(crate) fn get_image_base() -> *mut c_void {
            std::os::fortanix_sgx::mem::image_base() as _
        }
    }

    pub(crate) use imp::get_image_base;
}

#[cfg(all(target_env = "sgx", target_vendor = "fortanix", not(feature = "std")))]
pub use sgx_image_base::imp::set_image_base;

cfg_if::cfg_if! {
    // This needs to come first, to ensure that
    // Miri takes priority over the host platform
    if #[cfg(miri)] {
        pub(crate) mod miri;
        use self::miri::trace as trace_imp;
        pub(crate) use self::miri::Frame as FrameImp;
    } else if #[cfg(
        any(
            all(
                unix,
                not(target_os = "emscripten"),
                not(all(target_os = "ios", target_arch = "arm")),
            ),
            all(
                target_env = "sgx",
                target_vendor = "fortanix",
            ),
        )
    )] {
        mod libunwind;
        use self::libunwind::trace as trace_imp;
        pub(crate) use self::libunwind::Frame as FrameImp;
    } else if #[cfg(all(windows, not(target_vendor = "uwp")))] {
        cfg_if::cfg_if! {
            if #[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm64ec"))] {
                mod win64;
                use self::win64::trace as trace_imp;
                pub(crate) use self::win64::Frame as FrameImp;
            } else if #[cfg(any(target_arch = "x86", target_arch = "arm"))] {
                mod win32;
                use self::win32::trace as trace_imp;
                pub(crate) use self::win32::Frame as FrameImp;
            }
        }
    } else {
        mod noop;
        use self::noop::trace as trace_imp;
        pub(crate) use self::noop::Frame as FrameImp;
    }
}
