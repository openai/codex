//! Backtrace strategy for Windows platforms.
//!
//! This module contains the ability to generate a backtrace on Windows using one
//! of two possible methods. The `StackWalkEx` function is primarily used if
//! possible, but not all systems have that. Failing that the `StackWalk64`
//! function is used instead. Note that `StackWalkEx` is favored because it
//! handles debuginfo internally and returns inline frame information.
//!
//! Note that all dbghelp support is loaded dynamically, see `src/dbghelp.rs`
//! for more information about that.

use super::super::{dbghelp, windows_sys::*};
use core::ffi::c_void;
use core::mem;

#[derive(Clone, Copy)]
pub enum StackFrame {
    New(STACKFRAME_EX),
    Old(STACKFRAME64),
}

#[derive(Clone, Copy)]
pub struct Frame {
    pub(crate) stack_frame: StackFrame,
    base_address: *mut c_void,
}

// we're just sending around raw pointers and reading them, never interpreting
// them so this should be safe to both send and share across threads.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    pub fn ip(&self) -> *mut c_void {
        self.addr_pc().Offset as *mut _
    }

    pub fn sp(&self) -> *mut c_void {
        self.addr_stack().Offset as *mut _
    }

    pub fn symbol_address(&self) -> *mut c_void {
        self.ip()
    }

    pub fn module_base_address(&self) -> Option<*mut c_void> {
        Some(self.base_address)
    }

    #[cfg(not(target_env = "gnu"))]
    pub fn inline_context(&self) -> Option<u32> {
        match self.stack_frame {
            StackFrame::New(ref new) => Some(new.InlineFrameContext),
            StackFrame::Old(_) => None,
        }
    }

    fn addr_pc(&self) -> &ADDRESS64 {
        match self.stack_frame {
            StackFrame::New(ref new) => &new.AddrPC,
            StackFrame::Old(ref old) => &old.AddrPC,
        }
    }

    fn addr_pc_mut(&mut self) -> &mut ADDRESS64 {
        match self.stack_frame {
            StackFrame::New(ref mut new) => &mut new.AddrPC,
            StackFrame::Old(ref mut old) => &mut old.AddrPC,
        }
    }

    fn addr_frame_mut(&mut self) -> &mut ADDRESS64 {
        match self.stack_frame {
            StackFrame::New(ref mut new) => &mut new.AddrFrame,
            StackFrame::Old(ref mut old) => &mut old.AddrFrame,
        }
    }

    fn addr_stack(&self) -> &ADDRESS64 {
        match self.stack_frame {
            StackFrame::New(ref new) => &new.AddrStack,
            StackFrame::Old(ref old) => &old.AddrStack,
        }
    }

    fn addr_stack_mut(&mut self) -> &mut ADDRESS64 {
        match self.stack_frame {
            StackFrame::New(ref mut new) => &mut new.AddrStack,
            StackFrame::Old(ref mut old) => &mut old.AddrStack,
        }
    }
}

#[repr(C, align(16))] // required by `CONTEXT`, is a FIXME in windows metadata right now
struct MyContext(CONTEXT);

#[inline(always)]
pub unsafe fn trace(cb: &mut dyn FnMut(&super::Frame) -> bool) {
    // Allocate necessary structures for doing the stack walk
    let process = GetCurrentProcess();
    let thread = GetCurrentThread();

    let mut context = mem::zeroed::<MyContext>();
    RtlCaptureContext(&mut context.0);

    // Ensure this process's symbols are initialized
    let dbghelp = match dbghelp::init() {
        Ok(dbghelp) => dbghelp,
        Err(()) => return, // oh well...
    };

    let function_table_access = dbghelp.SymFunctionTableAccess64();
    let get_module_base = dbghelp.SymGetModuleBase64();

    let process_handle = GetCurrentProcess();

    // Attempt to use `StackWalkEx` if we can, but fall back to `StackWalk64`
    // since it's in theory supported on more systems.
    match (*dbghelp.dbghelp()).StackWalkEx() {
        #[allow(non_snake_case)]
        Some(StackWalkEx) => {
            let mut inner: STACKFRAME_EX = mem::zeroed();
            inner.StackFrameSize = mem::size_of::<STACKFRAME_EX>() as u32;
            let mut frame = super::Frame {
                inner: Frame {
                    stack_frame: StackFrame::New(inner),
                    base_address: 0 as _,
                },
            };
            let image = init_frame(&mut frame.inner, &context.0);
            let frame_ptr = match &mut frame.inner.stack_frame {
                StackFrame::New(ptr) => ptr as *mut STACKFRAME_EX,
                _ => unreachable!(),
            };

            while StackWalkEx(
                image as u32,
                process,
                thread,
                frame_ptr,
                &mut context.0 as *mut CONTEXT as *mut _,
                None,
                Some(function_table_access),
                Some(get_module_base),
                None,
                0,
            ) == TRUE
            {
                frame.inner.base_address = get_module_base(process_handle, frame.ip() as _) as _;

                if !cb(&frame) {
                    break;
                }
            }
        }
        None => {
            let mut frame = super::Frame {
                inner: Frame {
                    stack_frame: StackFrame::Old(mem::zeroed()),
                    base_address: 0 as _,
                },
            };
            let image = init_frame(&mut frame.inner, &context.0);
            let frame_ptr = match &mut frame.inner.stack_frame {
                StackFrame::Old(ptr) => ptr as *mut STACKFRAME64,
                _ => unreachable!(),
            };

            while dbghelp.StackWalk64()(
                image as u32,
                process,
                thread,
                frame_ptr,
                &mut context.0 as *mut CONTEXT as *mut _,
                None,
                Some(function_table_access),
                Some(get_module_base),
                None,
            ) == TRUE
            {
                frame.inner.base_address = get_module_base(process_handle, frame.ip() as _) as _;

                if !cb(&frame) {
                    break;
                }
            }
        }
    }
}

#[cfg(target_arch = "x86")]
fn init_frame(frame: &mut Frame, ctx: &CONTEXT) -> u16 {
    frame.addr_pc_mut().Offset = ctx.Eip as u64;
    frame.addr_pc_mut().Mode = AddrModeFlat;
    frame.addr_stack_mut().Offset = ctx.Esp as u64;
    frame.addr_stack_mut().Mode = AddrModeFlat;
    frame.addr_frame_mut().Offset = ctx.Ebp as u64;
    frame.addr_frame_mut().Mode = AddrModeFlat;

    IMAGE_FILE_MACHINE_I386
}

#[cfg(target_arch = "arm")]
fn init_frame(frame: &mut Frame, ctx: &CONTEXT) -> u16 {
    frame.addr_pc_mut().Offset = ctx.Pc as u64;
    frame.addr_pc_mut().Mode = AddrModeFlat;
    frame.addr_stack_mut().Offset = ctx.Sp as u64;
    frame.addr_stack_mut().Mode = AddrModeFlat;
    unsafe {
        frame.addr_frame_mut().Offset = ctx.R11 as u64;
    }
    frame.addr_frame_mut().Mode = AddrModeFlat;
    IMAGE_FILE_MACHINE_ARMNT
}
