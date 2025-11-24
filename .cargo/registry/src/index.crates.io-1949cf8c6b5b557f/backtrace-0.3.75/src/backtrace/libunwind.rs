//! Backtrace support using libunwind/gcc_s/etc APIs.
//!
//! This module contains the ability to unwind the stack using libunwind-style
//! APIs. Note that there's a whole bunch of implementations of the
//! libunwind-like API, and this is just trying to be compatible with most of
//! them all at once instead of being picky.
//!
//! The libunwind API is powered by `_Unwind_Backtrace` and is in practice very
//! reliable at generating a backtrace. It's not entirely clear how it does it
//! (frame pointers? eh_frame info? both?) but it seems to work!
//!
//! Most of the complexity of this module is handling the various platform
//! differences across libunwind implementations. Otherwise this is a pretty
//! straightforward Rust binding to the libunwind APIs.
//!
//! This is the default unwinding API for all non-Windows platforms currently.

use core::ffi::c_void;
use core::ptr::addr_of_mut;

pub enum Frame {
    Raw(*mut uw::_Unwind_Context),
    Cloned {
        ip: *mut c_void,
        sp: *mut c_void,
        symbol_address: *mut c_void,
    },
}

// With a raw libunwind pointer it should only ever be access in a readonly
// threadsafe fashion, so it's `Sync`. When sending to other threads via `Clone`
// we always switch to a version which doesn't retain interior pointers, so we
// should be `Send` as well.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    pub fn ip(&self) -> *mut c_void {
        let ctx = match *self {
            Frame::Raw(ctx) => ctx,
            Frame::Cloned { ip, .. } => return ip,
        };
        #[allow(unused_mut)]
        let mut ip = unsafe { uw::_Unwind_GetIP(ctx) as *mut c_void };

        // To reduce TCB size in SGX enclaves, we do not want to implement
        // symbol resolution functionality. Rather, we can print the offset of
        // the address here, which could be later mapped to correct function.
        #[cfg(all(target_env = "sgx", target_vendor = "fortanix"))]
        {
            let image_base = super::sgx_image_base::get_image_base();
            ip = usize::wrapping_sub(ip as usize, image_base as _) as _;
        }
        ip
    }

    pub fn sp(&self) -> *mut c_void {
        match *self {
            Frame::Raw(ctx) => unsafe { uw::get_sp(ctx) as *mut c_void },
            Frame::Cloned { sp, .. } => sp,
        }
    }

    pub fn symbol_address(&self) -> *mut c_void {
        if let Frame::Cloned { symbol_address, .. } = *self {
            return symbol_address;
        }

        // The macOS linker emits a "compact" unwind table that only includes an
        // entry for a function if that function either has an LSDA or its
        // encoding differs from that of the previous entry.  Consequently, on
        // macOS, `_Unwind_FindEnclosingFunction` is unreliable (it can return a
        // pointer to some totally unrelated function).  Instead, we just always
        // return the ip.
        //
        // https://github.com/rust-lang/rust/issues/74771#issuecomment-664056788
        //
        // Note the `skip_inner_frames.rs` test is skipped on macOS due to this
        // clause, and if this is fixed that test in theory can be run on macOS!
        if cfg!(target_vendor = "apple") {
            self.ip()
        } else {
            unsafe { uw::_Unwind_FindEnclosingFunction(self.ip()) }
        }
    }

    pub fn module_base_address(&self) -> Option<*mut c_void> {
        None
    }
}

impl Clone for Frame {
    fn clone(&self) -> Frame {
        Frame::Cloned {
            ip: self.ip(),
            sp: self.sp(),
            symbol_address: self.symbol_address(),
        }
    }
}

struct Bomb {
    enabled: bool,
}

impl Drop for Bomb {
    fn drop(&mut self) {
        if self.enabled {
            panic!("cannot panic during the backtrace function");
        }
    }
}

#[inline(always)]
pub unsafe fn trace(mut cb: &mut dyn FnMut(&super::Frame) -> bool) {
    unsafe {
        uw::_Unwind_Backtrace(trace_fn, addr_of_mut!(cb).cast());
    }

    extern "C" fn trace_fn(
        ctx: *mut uw::_Unwind_Context,
        arg: *mut c_void,
    ) -> uw::_Unwind_Reason_Code {
        let cb = unsafe { &mut *arg.cast::<&mut dyn FnMut(&super::Frame) -> bool>() };
        let cx = super::Frame {
            inner: Frame::Raw(ctx),
        };

        let mut bomb = Bomb { enabled: true };
        let keep_going = cb(&cx);
        bomb.enabled = false;

        if keep_going {
            uw::_URC_NO_REASON
        } else {
            uw::_URC_FAILURE
        }
    }
}

/// Unwind library interface used for backtraces
///
/// Note that dead code is allowed as here are just bindings
/// iOS doesn't use all of them it but adding more
/// platform-specific configs pollutes the code too much
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod uw {
    pub use self::_Unwind_Reason_Code::*;

    use core::ffi::c_void;

    #[repr(C)]
    pub enum _Unwind_Reason_Code {
        _URC_NO_REASON = 0,
        _URC_FOREIGN_EXCEPTION_CAUGHT = 1,
        _URC_FATAL_PHASE2_ERROR = 2,
        _URC_FATAL_PHASE1_ERROR = 3,
        _URC_NORMAL_STOP = 4,
        _URC_END_OF_STACK = 5,
        _URC_HANDLER_FOUND = 6,
        _URC_INSTALL_CONTEXT = 7,
        _URC_CONTINUE_UNWIND = 8,
        _URC_FAILURE = 9, // used only by ARM EABI
    }

    pub enum _Unwind_Context {}

    pub type _Unwind_Trace_Fn =
        extern "C" fn(ctx: *mut _Unwind_Context, arg: *mut c_void) -> _Unwind_Reason_Code;

    unsafe extern "C" {
        pub fn _Unwind_Backtrace(
            trace: _Unwind_Trace_Fn,
            trace_argument: *mut c_void,
        ) -> _Unwind_Reason_Code;
    }

    cfg_if::cfg_if! {
        // available since GCC 4.2.0, should be fine for our purpose
        if #[cfg(all(
            not(all(target_os = "android", target_arch = "arm")),
            not(all(target_os = "freebsd", target_arch = "arm")),
            not(all(target_os = "linux", target_arch = "arm")),
            not(all(target_os = "horizon", target_arch = "arm")),
            not(all(target_os = "rtems", target_arch = "arm")),
            not(all(target_os = "vita", target_arch = "arm")),
            not(all(target_os = "nuttx", target_arch = "arm")),
        ))] {
            unsafe extern "C" {
                pub fn _Unwind_GetIP(ctx: *mut _Unwind_Context) -> libc::uintptr_t;
                pub fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void;

                #[cfg(not(all(target_os = "linux", target_arch = "s390x")))]
                // This function is a misnomer: rather than getting this frame's
                // Canonical Frame Address (aka the caller frame's SP) it
                // returns this frame's SP.
                //
                // https://github.com/libunwind/libunwind/blob/d32956507cf29d9b1a98a8bce53c78623908f4fe/src/unwind/GetCFA.c#L28-L35
                #[link_name = "_Unwind_GetCFA"]
                pub fn get_sp(ctx: *mut _Unwind_Context) -> libc::uintptr_t;

            }

            // s390x uses a biased CFA value, therefore we need to use
            // _Unwind_GetGR to get the stack pointer register (%r15)
            // instead of relying on _Unwind_GetCFA.
            #[cfg(all(target_os = "linux", target_arch = "s390x"))]
            pub unsafe fn get_sp(ctx: *mut _Unwind_Context) -> libc::uintptr_t {
                unsafe extern "C" {
                    pub fn _Unwind_GetGR(ctx: *mut _Unwind_Context, index: libc::c_int) -> libc::uintptr_t;
                }
                unsafe { _Unwind_GetGR(ctx, 15) }
            }
        } else {
            use core::ptr::addr_of_mut;

            // On android and arm, the function `_Unwind_GetIP` and a bunch of
            // others are macros, so we define functions containing the
            // expansion of the macros.
            //
            // TODO: link to the header file that defines these macros, if you
            // can find it. (I, fitzgen, cannot find the header file that some
            // of these macro expansions were originally borrowed from.)
            #[repr(C)]
            enum _Unwind_VRS_Result {
                _UVRSR_OK = 0,
                _UVRSR_NOT_IMPLEMENTED = 1,
                _UVRSR_FAILED = 2,
            }
            #[repr(C)]
            enum _Unwind_VRS_RegClass {
                _UVRSC_CORE = 0,
                _UVRSC_VFP = 1,
                _UVRSC_FPA = 2,
                _UVRSC_WMMXD = 3,
                _UVRSC_WMMXC = 4,
            }
            #[repr(C)]
            enum _Unwind_VRS_DataRepresentation {
                _UVRSD_UINT32 = 0,
                _UVRSD_VFPX = 1,
                _UVRSD_FPAX = 2,
                _UVRSD_UINT64 = 3,
                _UVRSD_FLOAT = 4,
                _UVRSD_DOUBLE = 5,
            }

            type _Unwind_Word = libc::c_uint;
            unsafe extern "C" {
                fn _Unwind_VRS_Get(
                    ctx: *mut _Unwind_Context,
                    klass: _Unwind_VRS_RegClass,
                    word: _Unwind_Word,
                    repr: _Unwind_VRS_DataRepresentation,
                    data: *mut c_void,
                ) -> _Unwind_VRS_Result;
            }

            pub unsafe fn _Unwind_GetIP(ctx: *mut _Unwind_Context) -> libc::uintptr_t {
                let mut val: _Unwind_Word = 0;
                let ptr = addr_of_mut!(val);
                unsafe {
                    let _ = _Unwind_VRS_Get(
                        ctx,
                        _Unwind_VRS_RegClass::_UVRSC_CORE,
                        15,
                        _Unwind_VRS_DataRepresentation::_UVRSD_UINT32,
                        ptr.cast::<c_void>(),
                    );
                }
                (val & !1) as libc::uintptr_t
            }

            // R13 is the stack pointer on arm.
            const SP: _Unwind_Word = 13;

            pub unsafe fn get_sp(ctx: *mut _Unwind_Context) -> libc::uintptr_t {
                let mut val: _Unwind_Word = 0;
                let ptr = addr_of_mut!(val);
                unsafe {
                    let _ = _Unwind_VRS_Get(
                        ctx,
                        _Unwind_VRS_RegClass::_UVRSC_CORE,
                        SP,
                        _Unwind_VRS_DataRepresentation::_UVRSD_UINT32,
                        ptr.cast::<c_void>(),
                    );
                }
                val as libc::uintptr_t
            }

            // This function also doesn't exist on Android or ARM/Linux, so make it
            // a no-op.
            pub unsafe fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void {
                pc
            }
        }
    }
}
