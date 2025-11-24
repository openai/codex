#![allow(clippy::from_over_into)]

#[cfg(feature = "serde")]
use crate::resolve;
use crate::PrintFmt;
use crate::{resolve_frame, trace, BacktraceFmt, Symbol, SymbolName};
use core::ffi::c_void;
use std::fmt;
use std::path::{Path, PathBuf};
use std::prelude::v1::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Representation of an owned and self-contained backtrace.
///
/// This structure can be used to capture a backtrace at various points in a
/// program and later used to inspect what the backtrace was at that time.
///
/// `Backtrace` supports pretty-printing of backtraces through its `Debug`
/// implementation.
///
/// # Required features
///
/// This function requires the `std` feature of the `backtrace` crate to be
/// enabled, and the `std` feature is enabled by default.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Backtrace {
    // Frames here are listed from top-to-bottom of the stack
    frames: Box<[BacktraceFrame]>,
}

#[derive(Clone, Copy)]
struct TracePtr(*mut c_void);
/// SAFETY: These pointers are always valid within a process and are not used for mutation.
unsafe impl Send for TracePtr {}
/// SAFETY: These pointers are always valid within a process and are not used for mutation.
unsafe impl Sync for TracePtr {}

impl TracePtr {
    fn into_void(self) -> *mut c_void {
        self.0
    }
    #[cfg(feature = "serde")]
    fn from_addr(addr: usize) -> Self {
        TracePtr(addr as *mut c_void)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for TracePtr {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PrimitiveVisitor;

        impl<'de> serde::de::Visitor<'de> for PrimitiveVisitor {
            type Value = TracePtr;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("usize")
            }

            #[inline]
            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TracePtr(v as usize as *mut c_void))
            }

            #[inline]
            fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TracePtr(v as usize as *mut c_void))
            }

            #[inline]
            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if usize::BITS >= 32 {
                    Ok(TracePtr(v as usize as *mut c_void))
                } else {
                    Err(E::invalid_type(
                        serde::de::Unexpected::Unsigned(v as _),
                        &self,
                    ))
                }
            }

            #[inline]
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if usize::BITS >= 64 {
                    Ok(TracePtr(v as usize as *mut c_void))
                } else {
                    Err(E::invalid_type(
                        serde::de::Unexpected::Unsigned(v as _),
                        &self,
                    ))
                }
            }
        }

        deserializer.deserialize_u64(PrimitiveVisitor)
    }
}

#[cfg(feature = "serde")]
impl Serialize for TracePtr {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_u64(self.0 as usize as u64)
    }
}

fn _assert_send_sync() {
    fn _assert<T: Send + Sync>() {}
    _assert::<Backtrace>();
}

/// Captured version of a frame in a backtrace.
///
/// This type is returned as a list from `Backtrace::frames` and represents one
/// stack frame in a captured backtrace.
///
/// # Required features
///
/// This function requires the `std` feature of the `backtrace` crate to be
/// enabled, and the `std` feature is enabled by default.
#[derive(Clone)]
pub struct BacktraceFrame {
    frame: Frame,
    symbols: Option<Box<[BacktraceSymbol]>>,
}

#[derive(Clone)]
enum Frame {
    Raw(crate::Frame),
    #[cfg(feature = "serde")]
    Deserialized {
        ip: TracePtr,
        symbol_address: TracePtr,
        module_base_address: Option<TracePtr>,
    },
}

impl Frame {
    fn ip(&self) -> *mut c_void {
        match *self {
            Frame::Raw(ref f) => f.ip(),
            #[cfg(feature = "serde")]
            Frame::Deserialized { ip, .. } => ip.into_void(),
        }
    }

    fn symbol_address(&self) -> *mut c_void {
        match *self {
            Frame::Raw(ref f) => f.symbol_address(),
            #[cfg(feature = "serde")]
            Frame::Deserialized { symbol_address, .. } => symbol_address.into_void(),
        }
    }

    fn module_base_address(&self) -> Option<*mut c_void> {
        match *self {
            Frame::Raw(ref f) => f.module_base_address(),
            #[cfg(feature = "serde")]
            Frame::Deserialized {
                module_base_address,
                ..
            } => module_base_address.map(|addr| addr.into_void()),
        }
    }

    /// Resolve all addresses in the frame to their symbolic names.
    fn resolve_symbols(&self) -> Box<[BacktraceSymbol]> {
        let mut symbols = Vec::new();
        let sym = |symbol: &Symbol| {
            symbols.push(BacktraceSymbol {
                name: symbol.name().map(|m| m.as_bytes().into()),
                addr: symbol.addr().map(TracePtr),
                filename: symbol.filename().map(|m| m.to_owned()),
                lineno: symbol.lineno(),
                colno: symbol.colno(),
            });
        };
        match *self {
            Frame::Raw(ref f) => resolve_frame(f, sym),
            #[cfg(feature = "serde")]
            Frame::Deserialized { ip, .. } => {
                resolve(ip.into_void(), sym);
            }
        }
        symbols.into_boxed_slice()
    }
}

/// Captured version of a symbol in a backtrace.
///
/// This type is returned as a list from `BacktraceFrame::symbols` and
/// represents the metadata for a symbol in a backtrace.
///
/// # Required features
///
/// This function requires the `std` feature of the `backtrace` crate to be
/// enabled, and the `std` feature is enabled by default.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct BacktraceSymbol {
    name: Option<Box<[u8]>>,
    addr: Option<TracePtr>,
    filename: Option<PathBuf>,
    lineno: Option<u32>,
    colno: Option<u32>,
}

impl Backtrace {
    /// Captures a backtrace at the callsite of this function, returning an
    /// owned representation.
    ///
    /// This function is useful for representing a backtrace as an object in
    /// Rust. This returned value can be sent across threads and printed
    /// elsewhere, and the purpose of this value is to be entirely self
    /// contained.
    ///
    /// Note that on some platforms acquiring a full backtrace and resolving it
    /// can be extremely expensive. If the cost is too much for your application
    /// it's recommended to instead use `Backtrace::new_unresolved()` which
    /// avoids the symbol resolution step (which typically takes the longest)
    /// and allows deferring that to a later date.
    ///
    /// # Examples
    ///
    /// ```
    /// use backtrace::Backtrace;
    ///
    /// let current_backtrace = Backtrace::new();
    /// ```
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    #[inline(never)] // want to make sure there's a frame here to remove
    pub fn new() -> Backtrace {
        let mut bt = Self::create(Self::new as usize);
        bt.resolve();
        bt
    }

    /// Similar to `new` except that this does not resolve any symbols, this
    /// simply captures the backtrace as a list of addresses.
    ///
    /// At a later time the `resolve` function can be called to resolve this
    /// backtrace's symbols into readable names. This function exists because
    /// the resolution process can sometimes take a significant amount of time
    /// whereas any one backtrace may only be rarely printed.
    ///
    /// # Examples
    ///
    /// ```
    /// use backtrace::Backtrace;
    ///
    /// let mut current_backtrace = Backtrace::new_unresolved();
    /// println!("{current_backtrace:?}"); // no symbol names
    /// current_backtrace.resolve();
    /// println!("{current_backtrace:?}"); // symbol names now present
    /// ```
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    #[inline(never)] // want to make sure there's a frame here to remove
    pub fn new_unresolved() -> Backtrace {
        Self::create(Self::new_unresolved as usize)
    }

    fn create(ip: usize) -> Backtrace {
        let mut frames = Vec::new();
        trace(|frame| {
            frames.push(BacktraceFrame {
                frame: Frame::Raw(frame.clone()),
                symbols: None,
            });

            // clear inner frames, and start with call site.
            if frame.symbol_address() as usize == ip {
                frames.clear();
            }

            true
        });
        frames.shrink_to_fit();

        Backtrace {
            frames: frames.into_boxed_slice(),
        }
    }

    /// Returns the frames from when this backtrace was captured.
    ///
    /// The first entry of this slice is likely the function `Backtrace::new`,
    /// and the last frame is likely something about how this thread or the main
    /// function started.
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn frames(&self) -> &[BacktraceFrame] {
        self.frames.as_ref()
    }

    /// If this backtrace was created from `new_unresolved` then this function
    /// will resolve all addresses in the backtrace to their symbolic names.
    ///
    /// If this backtrace has been previously resolved or was created through
    /// `new`, this function does nothing.
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn resolve(&mut self) {
        self.frames.iter_mut().for_each(BacktraceFrame::resolve);
    }
}

impl From<Vec<BacktraceFrame>> for Backtrace {
    fn from(frames: Vec<BacktraceFrame>) -> Self {
        Backtrace {
            frames: frames.into_boxed_slice(),
        }
    }
}

impl From<crate::Frame> for BacktraceFrame {
    fn from(frame: crate::Frame) -> Self {
        BacktraceFrame {
            frame: Frame::Raw(frame),
            symbols: None,
        }
    }
}

// we don't want to implement `impl From<Backtrace> for Vec<BacktraceFrame>` on purpose,
// because "... additional directions for Vec<T> can weaken type inference ..."
// more information on https://github.com/rust-lang/backtrace-rs/pull/526
impl Into<Vec<BacktraceFrame>> for Backtrace {
    fn into(self) -> Vec<BacktraceFrame> {
        self.frames.into_vec()
    }
}

impl BacktraceFrame {
    /// Same as `Frame::ip`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn ip(&self) -> *mut c_void {
        self.frame.ip()
    }

    /// Same as `Frame::symbol_address`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn symbol_address(&self) -> *mut c_void {
        self.frame.symbol_address()
    }

    /// Same as `Frame::module_base_address`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn module_base_address(&self) -> Option<*mut c_void> {
        self.frame.module_base_address()
    }

    /// Returns the list of symbols that this frame corresponds to.
    ///
    /// Normally there is only one symbol per frame, but sometimes if a number
    /// of functions are inlined into one frame then multiple symbols will be
    /// returned. The first symbol listed is the "innermost function", whereas
    /// the last symbol is the outermost (last caller).
    ///
    /// Note that if this frame came from an unresolved backtrace then this will
    /// return an empty list.
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn symbols(&self) -> &[BacktraceSymbol] {
        self.symbols.as_ref().map(|s| &s[..]).unwrap_or(&[])
    }

    /// Resolve all addresses in this frame to their symbolic names.
    ///
    /// If this frame has been previously resolved, this function does nothing.
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn resolve(&mut self) {
        if self.symbols.is_none() {
            self.symbols = Some(self.frame.resolve_symbols());
        }
    }
}

impl BacktraceSymbol {
    /// Same as `Symbol::name`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn name(&self) -> Option<SymbolName<'_>> {
        self.name.as_ref().map(|s| SymbolName::new(s))
    }

    /// Same as `Symbol::addr`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn addr(&self) -> Option<*mut c_void> {
        self.addr.map(|s| s.into_void())
    }

    /// Same as `Symbol::filename`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn filename(&self) -> Option<&Path> {
        self.filename.as_deref()
    }

    /// Same as `Symbol::lineno`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn lineno(&self) -> Option<u32> {
        self.lineno
    }

    /// Same as `Symbol::colno`
    ///
    /// # Required features
    ///
    /// This function requires the `std` feature of the `backtrace` crate to be
    /// enabled, and the `std` feature is enabled by default.
    pub fn colno(&self) -> Option<u32> {
        self.colno
    }
}

impl fmt::Debug for Backtrace {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let style = if fmt.alternate() {
            PrintFmt::Full
        } else {
            PrintFmt::Short
        };

        // When printing paths we try to strip the cwd if it exists, otherwise
        // we just print the path as-is. Note that we also only do this for the
        // short format, because if it's full we presumably want to print
        // everything.
        let cwd = std::env::current_dir();
        let mut print_path =
            move |fmt: &mut fmt::Formatter<'_>, path: crate::BytesOrWideString<'_>| {
                let path = path.into_path_buf();
                if style == PrintFmt::Full {
                    if let Ok(cwd) = &cwd {
                        if let Ok(suffix) = path.strip_prefix(cwd) {
                            return fmt::Display::fmt(&suffix.display(), fmt);
                        }
                    }
                }
                fmt::Display::fmt(&path.display(), fmt)
            };

        let mut f = BacktraceFmt::new(fmt, style, &mut print_path);
        f.add_context()?;
        for frame in &self.frames {
            f.frame().backtrace_frame(frame)?;
        }
        f.finish()?;
        Ok(())
    }
}

impl Default for Backtrace {
    fn default() -> Backtrace {
        Backtrace::new()
    }
}

impl fmt::Debug for BacktraceFrame {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BacktraceFrame")
            .field("ip", &self.ip())
            .field("symbol_address", &self.symbol_address())
            .finish()
    }
}

impl fmt::Debug for BacktraceSymbol {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BacktraceSymbol")
            .field("name", &self.name())
            .field("addr", &self.addr())
            .field("filename", &self.filename())
            .field("lineno", &self.lineno())
            .field("colno", &self.colno())
            .finish()
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::*;
    use serde::de::Deserializer;
    use serde::ser::Serializer;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct SerializedFrame {
        ip: usize,
        symbol_address: usize,
        module_base_address: Option<usize>,
        symbols: Option<Box<[BacktraceSymbol]>>,
    }

    impl Serialize for BacktraceFrame {
        fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let BacktraceFrame { frame, symbols } = self;
            SerializedFrame {
                ip: frame.ip() as usize,
                symbol_address: frame.symbol_address() as usize,
                module_base_address: frame.module_base_address().map(|sym_a| sym_a as usize),
                symbols: symbols.clone(),
            }
            .serialize(s)
        }
    }

    impl<'a> Deserialize<'a> for BacktraceFrame {
        fn deserialize<D>(d: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'a>,
        {
            let frame: SerializedFrame = SerializedFrame::deserialize(d)?;
            Ok(BacktraceFrame {
                frame: Frame::Deserialized {
                    ip: TracePtr::from_addr(frame.ip),
                    symbol_address: TracePtr::from_addr(frame.symbol_address),
                    module_base_address: frame.module_base_address.map(TracePtr::from_addr),
                },
                symbols: frame.symbols,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_conversion() {
        let mut frames = vec![];
        crate::trace(|frame| {
            let converted = BacktraceFrame::from(frame.clone());
            frames.push(converted);
            true
        });

        let mut manual = Backtrace::from(frames);
        manual.resolve();
        let frames = manual.frames();

        for frame in frames {
            println!("{:?}", frame.ip());
            println!("{:?}", frame.symbol_address());
            println!("{:?}", frame.module_base_address());
            println!("{:?}", frame.symbols());
        }
    }
}
