use core::{fmt, str};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::path::Path;
        use std::prelude::v1::*;
    }
}

use super::backtrace::Frame;
use super::types::BytesOrWideString;
use core::ffi::c_void;
use rustc_demangle::{try_demangle, Demangle};

/// Resolve an address to a symbol, passing the symbol to the specified
/// closure.
///
/// This function will look up the given address in areas such as the local
/// symbol table, dynamic symbol table, or DWARF debug info (depending on the
/// activated implementation) to find symbols to yield.
///
/// The closure may not be called if resolution could not be performed, and it
/// also may be called more than once in the case of inlined functions.
///
/// Symbols yielded represent the execution at the specified `addr`, returning
/// file/line pairs for that address (if available).
///
/// Note that if you have a `Frame` then it's recommended to use the
/// `resolve_frame` function instead of this one.
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
///         let ip = frame.ip();
///
///         backtrace::resolve(ip, |symbol| {
///             // ...
///         });
///
///         false // only look at the top frame
///     });
/// }
/// ```
#[cfg(feature = "std")]
pub fn resolve<F: FnMut(&Symbol)>(addr: *mut c_void, cb: F) {
    let _guard = crate::lock::lock();
    unsafe { resolve_unsynchronized(addr, cb) }
}

/// Resolve a previously captured frame to a symbol, passing the symbol to the
/// specified closure.
///
/// This function performs the same function as `resolve` except that it takes a
/// `Frame` as an argument instead of an address. This can allow some platform
/// implementations of backtracing to provide more accurate symbol information
/// or information about inline frames for example. It's recommended to use this
/// if you can.
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
///         backtrace::resolve_frame(frame, |symbol| {
///             // ...
///         });
///
///         false // only look at the top frame
///     });
/// }
/// ```
#[cfg(feature = "std")]
pub fn resolve_frame<F: FnMut(&Symbol)>(frame: &Frame, cb: F) {
    let _guard = crate::lock::lock();
    unsafe { resolve_frame_unsynchronized(frame, cb) }
}

pub enum ResolveWhat<'a> {
    Address(*mut c_void),
    Frame(&'a Frame),
}

impl<'a> ResolveWhat<'a> {
    #[allow(dead_code)]
    fn address_or_ip(&self) -> *mut c_void {
        match self {
            ResolveWhat::Address(a) => adjust_ip(*a),
            ResolveWhat::Frame(f) => adjust_ip(f.ip()),
        }
    }
}

// IP values from stack frames are typically (always?) the instruction
// *after* the call that's the actual stack trace. Symbolizing this on
// causes the filename/line number to be one ahead and perhaps into
// the void if it's near the end of the function.
//
// This appears to basically always be the case on all platforms, so we always
// subtract one from a resolved ip to resolve it to the previous call
// instruction instead of the instruction being returned to.
//
// Ideally we would not do this. Ideally we would require callers of the
// `resolve` APIs here to manually do the -1 and account that they want location
// information for the *previous* instruction, not the current. Ideally we'd
// also expose on `Frame` if we are indeed the address of the next instruction
// or the current.
//
// For now though this is a pretty niche concern so we just internally always
// subtract one. Consumers should keep working and getting pretty good results,
// so we should be good enough.
fn adjust_ip(a: *mut c_void) -> *mut c_void {
    if a.is_null() {
        a
    } else {
        (a as usize - 1) as *mut c_void
    }
}

/// Same as `resolve`, only unsafe as it's unsynchronized.
///
/// This function does not have synchronization guarantees but is available when
/// the `std` feature of this crate isn't compiled in. See the `resolve`
/// function for more documentation and examples.
///
/// # Panics
///
/// See information on `resolve` for caveats on `cb` panicking.
pub unsafe fn resolve_unsynchronized<F>(addr: *mut c_void, mut cb: F)
where
    F: FnMut(&Symbol),
{
    unsafe { imp::resolve(ResolveWhat::Address(addr), &mut cb) }
}

/// Same as `resolve_frame`, only unsafe as it's unsynchronized.
///
/// This function does not have synchronization guarantees but is available
/// when the `std` feature of this crate isn't compiled in. See the
/// `resolve_frame` function for more documentation and examples.
///
/// # Panics
///
/// See information on `resolve_frame` for caveats on `cb` panicking.
pub unsafe fn resolve_frame_unsynchronized<F>(frame: &Frame, mut cb: F)
where
    F: FnMut(&Symbol),
{
    unsafe { imp::resolve(ResolveWhat::Frame(frame), &mut cb) }
}

/// A trait representing the resolution of a symbol in a file.
///
/// This trait is yielded as a trait object to the closure given to the
/// `backtrace::resolve` function, and it is virtually dispatched as it's
/// unknown which implementation is behind it.
///
/// A symbol can give contextual information about a function, for example the
/// name, filename, line number, precise address, etc. Not all information is
/// always available in a symbol, however, so all methods return an `Option`.
pub struct Symbol {
    // TODO: this lifetime bound needs to be persisted eventually to `Symbol`,
    // but that's currently a breaking change. For now this is safe since
    // `Symbol` is only ever handed out by reference and can't be cloned.
    inner: imp::Symbol<'static>,
}

impl Symbol {
    /// Returns the name of this function.
    ///
    /// The returned structure can be used to query various properties about the
    /// symbol name:
    ///
    /// * The `Display` implementation will print out the demangled symbol.
    /// * The raw `str` value of the symbol can be accessed (if it's valid
    ///   utf-8).
    /// * The raw bytes for the symbol name can be accessed.
    pub fn name(&self) -> Option<SymbolName<'_>> {
        self.inner.name()
    }

    /// Returns the starting address of this function.
    pub fn addr(&self) -> Option<*mut c_void> {
        self.inner.addr()
    }

    /// Returns the raw filename as a slice. This is mainly useful for `no_std`
    /// environments.
    pub fn filename_raw(&self) -> Option<BytesOrWideString<'_>> {
        self.inner.filename_raw()
    }

    /// Returns the column number for where this symbol is currently executing.
    ///
    /// Only gimli currently provides a value here and even then only if `filename`
    /// returns `Some`, and so it is then consequently subject to similar caveats.
    pub fn colno(&self) -> Option<u32> {
        self.inner.colno()
    }

    /// Returns the line number for where this symbol is currently executing.
    ///
    /// This return value is typically `Some` if `filename` returns `Some`, and
    /// is consequently subject to similar caveats.
    pub fn lineno(&self) -> Option<u32> {
        self.inner.lineno()
    }

    /// Returns the file name where this function was defined.
    ///
    /// This is currently only available when libbacktrace or gimli is being
    /// used (e.g. unix platforms other) and when a binary is compiled with
    /// debuginfo. If neither of these conditions is met then this will likely
    /// return `None`.
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    #[cfg(feature = "std")]
    #[allow(unreachable_code)]
    pub fn filename(&self) -> Option<&Path> {
        self.inner.filename()
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Symbol");
        if let Some(name) = self.name() {
            d.field("name", &name);
        }
        if let Some(addr) = self.addr() {
            d.field("addr", &addr);
        }

        #[cfg(feature = "std")]
        {
            if let Some(filename) = self.filename() {
                d.field("filename", &filename);
            }
        }

        if let Some(lineno) = self.lineno() {
            d.field("lineno", &lineno);
        }
        d.finish()
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "cpp_demangle")] {
        // Maybe a parsed C++ symbol, if parsing the mangled symbol as Rust
        // failed.
        struct OptionCppSymbol<'a>(Option<::cpp_demangle::BorrowedSymbol<'a>>);

        impl<'a> OptionCppSymbol<'a> {
            fn parse(input: &'a [u8]) -> OptionCppSymbol<'a> {
                OptionCppSymbol(::cpp_demangle::BorrowedSymbol::new(input).ok())
            }

            fn none() -> OptionCppSymbol<'a> {
                OptionCppSymbol(None)
            }
        }
    }
}

/// A wrapper around a symbol name to provide ergonomic accessors to the
/// demangled name, the raw bytes, the raw string, etc.
pub struct SymbolName<'a> {
    bytes: &'a [u8],
    demangled: Option<Demangle<'a>>,
    #[cfg(feature = "cpp_demangle")]
    cpp_demangled: OptionCppSymbol<'a>,
}

impl<'a> SymbolName<'a> {
    /// Creates a new symbol name from the raw underlying bytes.
    pub fn new(bytes: &'a [u8]) -> SymbolName<'a> {
        let str_bytes = str::from_utf8(bytes).ok();
        let demangled = str_bytes.and_then(|s| try_demangle(s).ok());

        #[cfg(feature = "cpp_demangle")]
        let cpp = if demangled.is_none() {
            OptionCppSymbol::parse(bytes)
        } else {
            OptionCppSymbol::none()
        };

        SymbolName {
            bytes,
            demangled,
            #[cfg(feature = "cpp_demangle")]
            cpp_demangled: cpp,
        }
    }

    /// Returns the raw (mangled) symbol name as a `str` if the symbol is valid utf-8.
    ///
    /// Use the `Display` implementation if you want the demangled version.
    pub fn as_str(&self) -> Option<&'a str> {
        self.demangled
            .as_ref()
            .map(|s| s.as_str())
            .or_else(|| str::from_utf8(self.bytes).ok())
    }

    /// Returns the raw symbol name as a list of bytes
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

fn format_symbol_name(
    fmt: fn(&str, &mut fmt::Formatter<'_>) -> fmt::Result,
    mut bytes: &[u8],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    while bytes.len() > 0 {
        match str::from_utf8(bytes) {
            Ok(name) => {
                fmt(name, f)?;
                break;
            }
            Err(err) => {
                fmt("\u{FFFD}", f)?;

                match err.error_len() {
                    Some(len) => bytes = &bytes[err.valid_up_to() + len..],
                    None => break,
                }
            }
        }
    }
    Ok(())
}

impl<'a> fmt::Display for SymbolName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref s) = self.demangled {
            return s.fmt(f);
        }

        #[cfg(feature = "cpp_demangle")]
        {
            if let Some(ref cpp) = self.cpp_demangled.0 {
                return cpp.fmt(f);
            }
        }

        format_symbol_name(fmt::Display::fmt, self.bytes, f)
    }
}

impl<'a> fmt::Debug for SymbolName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref s) = self.demangled {
            return s.fmt(f);
        }

        #[cfg(all(feature = "std", feature = "cpp_demangle"))]
        {
            use std::fmt::Write;

            // This may to print if the demangled symbol isn't actually
            // valid, so handle the error here gracefully by not propagating
            // it outwards.
            if let Some(ref cpp) = self.cpp_demangled.0 {
                let mut s = String::new();
                if write!(s, "{cpp}").is_ok() {
                    return s.fmt(f);
                }
            }
        }

        format_symbol_name(fmt::Debug::fmt, self.bytes, f)
    }
}

/// Attempt to reclaim that cached memory used to symbolicate addresses.
///
/// This method will attempt to release any global data structures that have
/// otherwise been cached globally or in the thread which typically represent
/// parsed DWARF information or similar.
///
/// # Caveats
///
/// While this function is always available it doesn't actually do anything on
/// most implementations. Libraries like dbghelp or libbacktrace do not provide
/// facilities to deallocate state and manage the allocated memory. For now the
/// `std` feature of this crate is the only feature where this
/// function has any effect.
#[cfg(feature = "std")]
pub fn clear_symbol_cache() {
    let _guard = crate::lock::lock();
    unsafe {
        imp::clear_symbol_cache();
    }
}

cfg_if::cfg_if! {
    if #[cfg(miri)] {
        mod miri;
        use miri as imp;
    } else if #[cfg(all(windows, target_env = "msvc", not(target_vendor = "uwp")))] {
        mod dbghelp;
        use dbghelp as imp;
    } else if #[cfg(all(
        any(unix, all(windows, target_env = "gnu")),
        not(target_vendor = "uwp"),
        not(target_os = "emscripten"),
        any(not(backtrace_in_libstd), feature = "backtrace"),
    ))] {
        mod gimli;
        use gimli as imp;
    } else {
        mod noop;
        use noop as imp;
    }
}
