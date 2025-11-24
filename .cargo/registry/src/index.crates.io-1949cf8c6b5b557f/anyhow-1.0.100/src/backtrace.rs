#[cfg(std_backtrace)]
pub(crate) use std::backtrace::{Backtrace, BacktraceStatus};

#[cfg(all(not(std_backtrace), feature = "backtrace"))]
pub(crate) use self::capture::{Backtrace, BacktraceStatus};

#[cfg(not(any(std_backtrace, feature = "backtrace")))]
pub(crate) enum Backtrace {}

#[cfg(std_backtrace)]
macro_rules! impl_backtrace {
    () => {
        std::backtrace::Backtrace
    };
}

#[cfg(all(not(std_backtrace), feature = "backtrace"))]
macro_rules! impl_backtrace {
    () => {
        impl core::fmt::Debug + core::fmt::Display
    };
}

#[cfg(any(std_backtrace, feature = "backtrace"))]
macro_rules! backtrace {
    () => {
        Some(crate::backtrace::Backtrace::capture())
    };
}

#[cfg(not(any(std_backtrace, feature = "backtrace")))]
macro_rules! backtrace {
    () => {
        None
    };
}

#[cfg(error_generic_member_access)]
macro_rules! backtrace_if_absent {
    ($err:expr) => {
        match $crate::nightly::request_ref_backtrace($err as &dyn core::error::Error) {
            Some(_) => None,
            None => backtrace!(),
        }
    };
}

#[cfg(all(
    any(feature = "std", not(anyhow_no_core_error)),
    not(error_generic_member_access),
    any(std_backtrace, feature = "backtrace")
))]
macro_rules! backtrace_if_absent {
    ($err:expr) => {
        backtrace!()
    };
}

#[cfg(all(
    any(feature = "std", not(anyhow_no_core_error)),
    not(std_backtrace),
    not(feature = "backtrace"),
))]
macro_rules! backtrace_if_absent {
    ($err:expr) => {
        None
    };
}

#[cfg(all(not(std_backtrace), feature = "backtrace"))]
mod capture {
    use alloc::borrow::{Cow, ToOwned as _};
    use alloc::vec::Vec;
    use backtrace::{BacktraceFmt, BytesOrWideString, Frame, PrintFmt, SymbolName};
    use core::cell::UnsafeCell;
    use core::fmt::{self, Debug, Display};
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::env;
    use std::path::{self, Path, PathBuf};
    use std::sync::Once;

    pub(crate) struct Backtrace {
        inner: Inner,
    }

    pub(crate) enum BacktraceStatus {
        Unsupported,
        Disabled,
        Captured,
    }

    enum Inner {
        Unsupported,
        Disabled,
        Captured(LazilyResolvedCapture),
    }

    struct Capture {
        actual_start: usize,
        resolved: bool,
        frames: Vec<BacktraceFrame>,
    }

    struct BacktraceFrame {
        frame: Frame,
        symbols: Vec<BacktraceSymbol>,
    }

    struct BacktraceSymbol {
        name: Option<Vec<u8>>,
        filename: Option<BytesOrWide>,
        lineno: Option<u32>,
        colno: Option<u32>,
    }

    enum BytesOrWide {
        Bytes(Vec<u8>),
        Wide(Vec<u16>),
    }

    impl Debug for Backtrace {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            let capture = match &self.inner {
                Inner::Unsupported => return fmt.write_str("<unsupported>"),
                Inner::Disabled => return fmt.write_str("<disabled>"),
                Inner::Captured(c) => c.force(),
            };

            let frames = &capture.frames[capture.actual_start..];

            write!(fmt, "Backtrace ")?;

            let mut dbg = fmt.debug_list();

            for frame in frames {
                if frame.frame.ip().is_null() {
                    continue;
                }

                dbg.entries(&frame.symbols);
            }

            dbg.finish()
        }
    }

    impl Debug for BacktraceFrame {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            let mut dbg = fmt.debug_list();
            dbg.entries(&self.symbols);
            dbg.finish()
        }
    }

    impl Debug for BacktraceSymbol {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            write!(fmt, "{{ ")?;

            if let Some(fn_name) = self.name.as_ref().map(|b| SymbolName::new(b)) {
                write!(fmt, "fn: \"{:#}\"", fn_name)?;
            } else {
                write!(fmt, "fn: <unknown>")?;
            }

            if let Some(fname) = self.filename.as_ref() {
                write!(fmt, ", file: \"{:?}\"", fname)?;
            }

            if let Some(line) = self.lineno {
                write!(fmt, ", line: {:?}", line)?;
            }

            write!(fmt, " }}")
        }
    }

    impl Debug for BytesOrWide {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            output_filename(
                fmt,
                match self {
                    BytesOrWide::Bytes(w) => BytesOrWideString::Bytes(w),
                    BytesOrWide::Wide(w) => BytesOrWideString::Wide(w),
                },
                PrintFmt::Short,
                env::current_dir().as_ref().ok(),
            )
        }
    }

    impl Backtrace {
        fn enabled() -> bool {
            static ENABLED: AtomicUsize = AtomicUsize::new(0);
            match ENABLED.load(Ordering::Relaxed) {
                0 => {}
                1 => return false,
                _ => return true,
            }
            let enabled = match env::var_os("RUST_LIB_BACKTRACE") {
                Some(s) => s != "0",
                None => match env::var_os("RUST_BACKTRACE") {
                    Some(s) => s != "0",
                    None => false,
                },
            };
            ENABLED.store(enabled as usize + 1, Ordering::Relaxed);
            enabled
        }

        #[inline(never)] // want to make sure there's a frame here to remove
        pub(crate) fn capture() -> Backtrace {
            if Backtrace::enabled() {
                Backtrace::create(Backtrace::capture as usize)
            } else {
                let inner = Inner::Disabled;
                Backtrace { inner }
            }
        }

        // Capture a backtrace which starts just before the function addressed
        // by `ip`
        fn create(ip: usize) -> Backtrace {
            let mut frames = Vec::new();
            let mut actual_start = None;
            backtrace::trace(|frame| {
                frames.push(BacktraceFrame {
                    frame: frame.clone(),
                    symbols: Vec::new(),
                });
                if frame.symbol_address() as usize == ip && actual_start.is_none() {
                    actual_start = Some(frames.len() + 1);
                }
                true
            });

            // If no frames came out assume that this is an unsupported platform
            // since `backtrace` doesn't provide a way of learning this right
            // now, and this should be a good enough approximation.
            let inner = if frames.is_empty() {
                Inner::Unsupported
            } else {
                Inner::Captured(LazilyResolvedCapture::new(Capture {
                    actual_start: actual_start.unwrap_or(0),
                    frames,
                    resolved: false,
                }))
            };

            Backtrace { inner }
        }

        pub(crate) fn status(&self) -> BacktraceStatus {
            match self.inner {
                Inner::Unsupported => BacktraceStatus::Unsupported,
                Inner::Disabled => BacktraceStatus::Disabled,
                Inner::Captured(_) => BacktraceStatus::Captured,
            }
        }
    }

    impl Display for Backtrace {
        fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            let capture = match &self.inner {
                Inner::Unsupported => return fmt.write_str("unsupported backtrace"),
                Inner::Disabled => return fmt.write_str("disabled backtrace"),
                Inner::Captured(c) => c.force(),
            };

            let full = fmt.alternate();
            let (frames, style) = if full {
                (&capture.frames[..], PrintFmt::Full)
            } else {
                (&capture.frames[capture.actual_start..], PrintFmt::Short)
            };

            // When printing paths we try to strip the cwd if it exists,
            // otherwise we just print the path as-is. Note that we also only do
            // this for the short format, because if it's full we presumably
            // want to print everything.
            let cwd = env::current_dir();
            let mut print_path = move |fmt: &mut fmt::Formatter, path: BytesOrWideString| {
                output_filename(fmt, path, style, cwd.as_ref().ok())
            };

            let mut f = BacktraceFmt::new(fmt, style, &mut print_path);
            f.add_context()?;
            for frame in frames {
                let mut f = f.frame();
                if frame.symbols.is_empty() {
                    f.print_raw(frame.frame.ip(), None, None, None)?;
                } else {
                    for symbol in frame.symbols.iter() {
                        f.print_raw_with_column(
                            frame.frame.ip(),
                            symbol.name.as_ref().map(|b| SymbolName::new(b)),
                            symbol.filename.as_ref().map(|b| match b {
                                BytesOrWide::Bytes(w) => BytesOrWideString::Bytes(w),
                                BytesOrWide::Wide(w) => BytesOrWideString::Wide(w),
                            }),
                            symbol.lineno,
                            symbol.colno,
                        )?;
                    }
                }
            }
            f.finish()?;
            Ok(())
        }
    }

    struct LazilyResolvedCapture {
        sync: Once,
        capture: UnsafeCell<Capture>,
    }

    impl LazilyResolvedCapture {
        fn new(capture: Capture) -> Self {
            LazilyResolvedCapture {
                sync: Once::new(),
                capture: UnsafeCell::new(capture),
            }
        }

        fn force(&self) -> &Capture {
            self.sync.call_once(|| {
                // Safety: This exclusive reference can't overlap with any
                // others. `Once` guarantees callers will block until this
                // closure returns. `Once` also guarantees only a single caller
                // will enter this closure.
                unsafe { &mut *self.capture.get() }.resolve();
            });

            // Safety: This shared reference can't overlap with the exclusive
            // reference above.
            unsafe { &*self.capture.get() }
        }
    }

    // Safety: Access to the inner value is synchronized using a thread-safe
    // `Once`. So long as `Capture` is `Sync`, `LazilyResolvedCapture` is too
    unsafe impl Sync for LazilyResolvedCapture where Capture: Sync {}

    impl Capture {
        fn resolve(&mut self) {
            // If we're already resolved, nothing to do!
            if self.resolved {
                return;
            }
            self.resolved = true;

            for frame in self.frames.iter_mut() {
                let symbols = &mut frame.symbols;
                let frame = &frame.frame;
                backtrace::resolve_frame(frame, |symbol| {
                    symbols.push(BacktraceSymbol {
                        name: symbol.name().map(|m| m.as_bytes().to_vec()),
                        filename: symbol.filename_raw().map(|b| match b {
                            BytesOrWideString::Bytes(b) => BytesOrWide::Bytes(b.to_owned()),
                            BytesOrWideString::Wide(b) => BytesOrWide::Wide(b.to_owned()),
                        }),
                        lineno: symbol.lineno(),
                        colno: symbol.colno(),
                    });
                });
            }
        }
    }

    // Prints the filename of the backtrace frame.
    fn output_filename(
        fmt: &mut fmt::Formatter,
        bows: BytesOrWideString,
        print_fmt: PrintFmt,
        cwd: Option<&PathBuf>,
    ) -> fmt::Result {
        let file: Cow<Path> = match bows {
            #[cfg(unix)]
            BytesOrWideString::Bytes(bytes) => {
                use std::os::unix::ffi::OsStrExt;
                Path::new(std::ffi::OsStr::from_bytes(bytes)).into()
            }
            #[cfg(not(unix))]
            BytesOrWideString::Bytes(bytes) => {
                Path::new(std::str::from_utf8(bytes).unwrap_or("<unknown>")).into()
            }
            #[cfg(windows)]
            BytesOrWideString::Wide(wide) => {
                use std::os::windows::ffi::OsStringExt;
                Cow::Owned(std::ffi::OsString::from_wide(wide).into())
            }
            #[cfg(not(windows))]
            BytesOrWideString::Wide(_wide) => Path::new("<unknown>").into(),
        };
        if print_fmt == PrintFmt::Short && file.is_absolute() {
            if let Some(cwd) = cwd {
                if let Ok(stripped) = file.strip_prefix(&cwd) {
                    if let Some(s) = stripped.to_str() {
                        return write!(fmt, ".{}{}", path::MAIN_SEPARATOR, s);
                    }
                }
            }
        }
        Display::fmt(&file.display(), fmt)
    }
}

fn _assert_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<Backtrace>();
}
