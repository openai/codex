//! Symbolication strategy using `dbghelp.dll` on Windows, only used for MSVC
//!
//! This symbolication strategy, like with backtraces, uses dynamically loaded
//! information from `dbghelp.dll`. (see `src/dbghelp.rs` for info about why
//! it's dynamically loaded).
//!
//! This API selects its resolution strategy based on the frame provided or the
//! information we have at hand. If a frame from `StackWalkEx` is given to us
//! then we use similar APIs to generate correct information about inlined
//! functions. Otherwise if all we have is an address or an older stack frame
//! from `StackWalk64` we use the older APIs for symbolication.
//!
//! There's a good deal of support in this module, but a good chunk of it is
//! converting back and forth between Windows types and Rust types. For example
//! symbols come to us as wide strings which we then convert to utf-8 strings if
//! we can.

#![allow(bad_style)]

use super::super::{dbghelp, windows_sys::*};
use super::{BytesOrWideString, ResolveWhat, SymbolName};
use core::ffi::c_void;
use core::marker;
use core::mem;
use core::ptr;
use core::slice;

// FIXME: replace with ptr::from_ref once MSRV is high enough
#[inline(always)]
#[must_use]
const fn ptr_from_ref<T: ?Sized>(r: &T) -> *const T {
    r
}

// Store an OsString on std so we can provide the symbol name and filename.
pub struct Symbol<'a> {
    name: *const [u8],
    addr: *mut c_void,
    line: Option<u32>,
    filename: Option<*const [u16]>,
    #[cfg(feature = "std")]
    _filename_cache: Option<::std::ffi::OsString>,
    #[cfg(not(feature = "std"))]
    _filename_cache: (),
    _marker: marker::PhantomData<&'a i32>,
}

impl Symbol<'_> {
    pub fn name(&self) -> Option<SymbolName<'_>> {
        Some(SymbolName::new(unsafe { &*self.name }))
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        Some(self.addr)
    }

    pub fn filename_raw(&self) -> Option<BytesOrWideString<'_>> {
        self.filename
            .map(|slice| unsafe { BytesOrWideString::Wide(&*slice) })
    }

    pub fn colno(&self) -> Option<u32> {
        None
    }

    pub fn lineno(&self) -> Option<u32> {
        self.line
    }

    #[cfg(feature = "std")]
    pub fn filename(&self) -> Option<&::std::path::Path> {
        use std::path::Path;

        self._filename_cache.as_ref().map(Path::new)
    }
}

#[repr(C, align(8))]
struct Aligned8<T>(T);

#[cfg(not(target_vendor = "win7"))]
pub unsafe fn resolve(what: ResolveWhat<'_>, cb: &mut dyn FnMut(&super::Symbol)) {
    // Ensure this process's symbols are initialized
    let dbghelp = match dbghelp::init() {
        Ok(dbghelp) => dbghelp,
        Err(()) => return, // oh well...
    };
    unsafe {
        match what {
            ResolveWhat::Address(_) => {
                resolve_with_inline(&dbghelp, what.address_or_ip(), None, cb)
            }
            ResolveWhat::Frame(frame) => {
                resolve_with_inline(&dbghelp, frame.ip(), frame.inner.inline_context(), cb)
            }
        };
    }
}

#[cfg(target_vendor = "win7")]
pub unsafe fn resolve(what: ResolveWhat<'_>, cb: &mut dyn FnMut(&super::Symbol)) {
    // Ensure this process's symbols are initialized
    let dbghelp = match dbghelp::init() {
        Ok(dbghelp) => dbghelp,
        Err(()) => return, // oh well...
    };

    unsafe {
        let resolve_inner = if (*dbghelp.dbghelp()).SymAddrIncludeInlineTrace().is_some() {
            // We are on a version of dbghelp 6.2+, which contains the more modern
            // Inline APIs.
            resolve_with_inline
        } else {
            // We are on an older version of dbghelp which doesn't contain the Inline
            // APIs.
            resolve_legacy
        };
        match what {
            ResolveWhat::Address(_) => resolve_inner(&dbghelp, what.address_or_ip(), None, cb),
            ResolveWhat::Frame(frame) => {
                resolve_inner(&dbghelp, frame.ip(), frame.inner.inline_context(), cb)
            }
        };
    }
}

/// Resolve the address using the legacy dbghelp API.
///
/// This should work all the way down to Windows XP. The inline context is
/// ignored, since this concept was only introduced in dbghelp 6.2+.
#[cfg(target_vendor = "win7")]
unsafe fn resolve_legacy(
    dbghelp: &dbghelp::Init,
    addr: *mut c_void,
    _inline_context: Option<u32>,
    cb: &mut dyn FnMut(&super::Symbol),
) -> Option<()> {
    let addr = super::adjust_ip(addr) as u64;
    unsafe {
        do_resolve(
            |info| dbghelp.SymFromAddrW()(GetCurrentProcess(), addr, &mut 0, info),
            |line| dbghelp.SymGetLineFromAddrW64()(GetCurrentProcess(), addr, &mut 0, line),
            cb,
        );
    }
    Some(())
}

/// Resolve the address using the modern dbghelp APIs.
///
/// Note that calling this function requires having dbghelp 6.2+ loaded - and
/// will panic otherwise.
unsafe fn resolve_with_inline(
    dbghelp: &dbghelp::Init,
    addr: *mut c_void,
    inline_context: Option<u32>,
    cb: &mut dyn FnMut(&super::Symbol),
) -> Option<()> {
    unsafe {
        let current_process = GetCurrentProcess();
        // Ensure we have the functions we need. Return if any aren't found.
        let SymFromInlineContextW = (*dbghelp.dbghelp()).SymFromInlineContextW()?;
        let SymGetLineFromInlineContextW = (*dbghelp.dbghelp()).SymGetLineFromInlineContextW()?;

        let addr = super::adjust_ip(addr) as u64;

        let (inlined_frame_count, inline_context) = if let Some(ic) = inline_context {
            (0, ic)
        } else {
            let SymAddrIncludeInlineTrace = (*dbghelp.dbghelp()).SymAddrIncludeInlineTrace()?;
            let SymQueryInlineTrace = (*dbghelp.dbghelp()).SymQueryInlineTrace()?;

            let mut inlined_frame_count = SymAddrIncludeInlineTrace(current_process, addr);

            let mut inline_context = 0;

            // If there is are inlined frames but we can't load them for some reason OR if there are no
            // inlined frames, then we disregard inlined_frame_count and inline_context.
            if (inlined_frame_count > 0
                && SymQueryInlineTrace(
                    current_process,
                    addr,
                    0,
                    addr,
                    addr,
                    &mut inline_context,
                    &mut 0,
                ) != TRUE)
                || inlined_frame_count == 0
            {
                inlined_frame_count = 0;
                inline_context = 0;
            }

            (inlined_frame_count, inline_context)
        };

        let last_inline_context = inline_context + 1 + inlined_frame_count;

        for inline_context in inline_context..last_inline_context {
            do_resolve(
                |info| SymFromInlineContextW(current_process, addr, inline_context, &mut 0, info),
                |line| {
                    SymGetLineFromInlineContextW(
                        current_process,
                        addr,
                        inline_context,
                        0,
                        &mut 0,
                        line,
                    )
                },
                cb,
            );
        }
    }
    Some(())
}

unsafe fn do_resolve(
    sym_from_addr: impl FnOnce(*mut SYMBOL_INFOW) -> BOOL,
    get_line_from_addr: impl FnOnce(&mut IMAGEHLP_LINEW64) -> BOOL,
    cb: &mut dyn FnMut(&super::Symbol),
) {
    const SIZE: usize = 2 * MAX_SYM_NAME as usize + mem::size_of::<SYMBOL_INFOW>();
    let mut data = Aligned8([0u8; SIZE]);
    let info = unsafe { &mut *data.0.as_mut_ptr().cast::<SYMBOL_INFOW>() };
    info.MaxNameLen = MAX_SYM_NAME as u32;
    // the struct size in C.  the value is different to
    // `size_of::<SYMBOL_INFOW>() - MAX_SYM_NAME + 1` (== 81)
    // due to struct alignment.
    info.SizeOfStruct = 88;

    if sym_from_addr(info) != TRUE {
        return;
    }

    // If the symbol name is greater than MaxNameLen, SymFromAddrW will
    // give a buffer of (MaxNameLen - 1) characters and set NameLen to
    // the real value.
    let name_len = ::core::cmp::min(info.NameLen as usize, info.MaxNameLen as usize - 1);
    let name_ptr = info.Name.as_ptr().cast::<u16>();

    // Reencode the utf-16 symbol to utf-8 so we can use `SymbolName::new` like
    // all other platforms
    let mut name_buffer = [0_u8; 256];
    let mut name_len = unsafe {
        WideCharToMultiByte(
            CP_UTF8,
            0,
            name_ptr,
            name_len as i32,
            name_buffer.as_mut_ptr(),
            name_buffer.len() as i32,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
        ) as usize
    };
    if name_len == 0 {
        // If the returned length is zero that means the buffer wasn't big enough.
        // However, the buffer will be filled with as much as will fit.
        name_len = name_buffer.len();
    } else if name_len > name_buffer.len() {
        // This can't happen.
        return;
    }
    let name = ptr::addr_of!(name_buffer[..name_len]);

    let mut line = IMAGEHLP_LINEW64 {
        SizeOfStruct: 0,
        Key: core::ptr::null_mut(),
        LineNumber: 0,
        FileName: core::ptr::null_mut(),
        Address: 0,
    };
    line.SizeOfStruct = mem::size_of::<IMAGEHLP_LINEW64>() as u32;

    let mut filename = None;
    let mut lineno = None;
    if get_line_from_addr(&mut line) == TRUE {
        lineno = Some(line.LineNumber);

        let base = line.FileName;
        let mut len = 0;
        while unsafe { *base.offset(len) != 0 } {
            len += 1;
        }

        let len = len as usize;

        unsafe {
            filename = Some(ptr_from_ref(slice::from_raw_parts(base, len)));
        }
    }

    cb(&super::Symbol {
        inner: Symbol {
            name,
            addr: info.Address as *mut _,
            line: lineno,
            filename,
            _filename_cache: unsafe { cache(filename) },
            _marker: marker::PhantomData,
        },
    })
}

#[cfg(feature = "std")]
unsafe fn cache(filename: Option<*const [u16]>) -> Option<::std::ffi::OsString> {
    use std::os::windows::ffi::OsStringExt;
    unsafe { filename.map(|f| ::std::ffi::OsString::from_wide(&*f)) }
}

#[cfg(not(feature = "std"))]
unsafe fn cache(_filename: Option<*const [u16]>) {}

pub unsafe fn clear_symbol_cache() {}
