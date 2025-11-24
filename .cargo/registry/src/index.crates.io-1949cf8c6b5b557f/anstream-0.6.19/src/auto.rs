use crate::stream::AsLockedWrite;
use crate::stream::RawStream;
use crate::ColorChoice;
use crate::StripStream;
#[cfg(all(windows, feature = "wincon"))]
use crate::WinconStream;

/// [`std::io::Write`] that adapts ANSI escape codes to the underlying `Write`s capabilities
///
/// This includes
/// - Stripping colors for non-terminals
/// - Respecting env variables like [NO_COLOR](https://no-color.org/) or [CLICOLOR](https://bixense.com/clicolors/)
/// - *(windows)* Falling back to the wincon API where [ENABLE_VIRTUAL_TERMINAL_PROCESSING](https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences#output-sequences) is unsupported
///
/// You can customize auto-detection by calling into
/// [anstyle_query](https://docs.rs/anstyle-query/latest/anstyle_query/)
/// to get a [`ColorChoice`] and then calling [`AutoStream::new(stream, choice)`].
#[derive(Debug)]
pub struct AutoStream<S: RawStream> {
    inner: StreamInner<S>,
}

#[derive(Debug)]
enum StreamInner<S: RawStream> {
    PassThrough(S),
    Strip(StripStream<S>),
    #[cfg(all(windows, feature = "wincon"))]
    Wincon(WinconStream<S>),
}

impl<S> AutoStream<S>
where
    S: RawStream,
{
    /// Runtime control over styling behavior
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "auto")] {
    /// # use std::io::IsTerminal as _;
    /// // Like `AutoStream::choice` but without `NO_COLOR`, `CLICOLOR_FORCE`, `CI`
    /// fn choice(raw: &dyn anstream::stream::RawStream) -> anstream::ColorChoice {
    ///     let choice = anstream::ColorChoice::global();
    ///     if choice == anstream::ColorChoice::Auto {
    ///         if raw.is_terminal() && anstyle_query::term_supports_color() {
    ///             anstream::ColorChoice::Always
    ///         } else {
    ///             anstream::ColorChoice::Never
    ///         }
    ///     } else {
    ///         choice
    ///     }
    /// }
    ///
    /// let stream = std::io::stdout();
    /// let choice = choice(&stream);
    /// let auto = anstream::AutoStream::new(stream, choice);
    /// # }
    /// ```
    #[inline]
    pub fn new(raw: S, choice: ColorChoice) -> Self {
        match choice {
            #[cfg(feature = "auto")]
            ColorChoice::Auto => Self::auto(raw),
            #[cfg(not(feature = "auto"))]
            ColorChoice::Auto => Self::never(raw),
            ColorChoice::AlwaysAnsi => Self::always_ansi(raw),
            ColorChoice::Always => Self::always(raw),
            ColorChoice::Never => Self::never(raw),
        }
    }

    /// Auto-adapt for the stream's capabilities
    #[cfg(feature = "auto")]
    #[inline]
    pub fn auto(raw: S) -> Self {
        let choice = Self::choice(&raw);
        debug_assert_ne!(choice, ColorChoice::Auto);
        Self::new(raw, choice)
    }

    /// Report the desired choice for the given stream
    #[cfg(feature = "auto")]
    pub fn choice(raw: &S) -> ColorChoice {
        choice(raw)
    }

    /// Force ANSI escape codes to be passed through as-is, no matter what the inner `Write`
    /// supports.
    #[inline]
    pub fn always_ansi(raw: S) -> Self {
        #[cfg(feature = "auto")]
        {
            if raw.is_terminal() {
                let _ = anstyle_query::windows::enable_ansi_colors();
            }
        }
        Self::always_ansi_(raw)
    }

    #[inline]
    fn always_ansi_(raw: S) -> Self {
        let inner = StreamInner::PassThrough(raw);
        AutoStream { inner }
    }

    /// Force color, no matter what the inner `Write` supports.
    #[inline]
    pub fn always(raw: S) -> Self {
        if cfg!(windows) {
            #[cfg(feature = "auto")]
            let use_wincon = raw.is_terminal()
                && !anstyle_query::windows::enable_ansi_colors().unwrap_or(true)
                && !anstyle_query::term_supports_ansi_color();
            #[cfg(not(feature = "auto"))]
            let use_wincon = true;
            if use_wincon {
                Self::wincon(raw).unwrap_or_else(|raw| Self::always_ansi_(raw))
            } else {
                Self::always_ansi_(raw)
            }
        } else {
            Self::always_ansi(raw)
        }
    }

    /// Only pass printable data to the inner `Write`.
    #[inline]
    pub fn never(raw: S) -> Self {
        let inner = StreamInner::Strip(StripStream::new(raw));
        AutoStream { inner }
    }

    #[inline]
    fn wincon(raw: S) -> Result<Self, S> {
        #[cfg(all(windows, feature = "wincon"))]
        {
            Ok(Self {
                inner: StreamInner::Wincon(WinconStream::new(raw)),
            })
        }
        #[cfg(not(all(windows, feature = "wincon")))]
        {
            Err(raw)
        }
    }

    /// Get the wrapped [`RawStream`]
    #[inline]
    pub fn into_inner(self) -> S {
        match self.inner {
            StreamInner::PassThrough(w) => w,
            StreamInner::Strip(w) => w.into_inner(),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => w.into_inner(),
        }
    }

    /// Returns `true` if the descriptor/handle refers to a terminal/tty.
    #[inline]
    pub fn is_terminal(&self) -> bool {
        match &self.inner {
            StreamInner::PassThrough(w) => w.is_terminal(),
            StreamInner::Strip(w) => w.is_terminal(),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(_) => true, // its only ever a terminal
        }
    }

    /// Prefer [`AutoStream::choice`]
    ///
    /// This doesn't report what is requested but what is currently active.
    #[inline]
    #[cfg(feature = "auto")]
    pub fn current_choice(&self) -> ColorChoice {
        match &self.inner {
            StreamInner::PassThrough(_) => ColorChoice::AlwaysAnsi,
            StreamInner::Strip(_) => ColorChoice::Never,
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(_) => ColorChoice::Always,
        }
    }
}

#[cfg(feature = "auto")]
fn choice(raw: &dyn RawStream) -> ColorChoice {
    let choice = ColorChoice::global();
    match choice {
        ColorChoice::Auto => {
            let clicolor = anstyle_query::clicolor();
            let clicolor_enabled = clicolor.unwrap_or(false);
            let clicolor_disabled = !clicolor.unwrap_or(true);
            if anstyle_query::no_color() {
                ColorChoice::Never
            } else if anstyle_query::clicolor_force() {
                ColorChoice::Always
            } else if clicolor_disabled {
                ColorChoice::Never
            } else if raw.is_terminal()
                && (anstyle_query::term_supports_color()
                    || clicolor_enabled
                    || anstyle_query::is_ci())
            {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            }
        }
        ColorChoice::AlwaysAnsi | ColorChoice::Always | ColorChoice::Never => choice,
    }
}

impl AutoStream<std::io::Stdout> {
    /// Get exclusive access to the `AutoStream`
    ///
    /// Why?
    /// - Faster performance when writing in a loop
    /// - Avoid other threads interleaving output with the current thread
    #[inline]
    pub fn lock(self) -> AutoStream<std::io::StdoutLock<'static>> {
        let inner = match self.inner {
            StreamInner::PassThrough(w) => StreamInner::PassThrough(w.lock()),
            StreamInner::Strip(w) => StreamInner::Strip(w.lock()),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => StreamInner::Wincon(w.lock()),
        };
        AutoStream { inner }
    }
}

impl AutoStream<std::io::Stderr> {
    /// Get exclusive access to the `AutoStream`
    ///
    /// Why?
    /// - Faster performance when writing in a loop
    /// - Avoid other threads interleaving output with the current thread
    #[inline]
    pub fn lock(self) -> AutoStream<std::io::StderrLock<'static>> {
        let inner = match self.inner {
            StreamInner::PassThrough(w) => StreamInner::PassThrough(w.lock()),
            StreamInner::Strip(w) => StreamInner::Strip(w.lock()),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => StreamInner::Wincon(w.lock()),
        };
        AutoStream { inner }
    }
}

impl<S> std::io::Write for AutoStream<S>
where
    S: RawStream + AsLockedWrite,
{
    // Must forward all calls to ensure locking happens appropriately
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            StreamInner::PassThrough(w) => w.as_locked_write().write(buf),
            StreamInner::Strip(w) => w.write(buf),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => w.write(buf),
        }
    }
    #[inline]
    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        match &mut self.inner {
            StreamInner::PassThrough(w) => w.as_locked_write().write_vectored(bufs),
            StreamInner::Strip(w) => w.write_vectored(bufs),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => w.write_vectored(bufs),
        }
    }
    // is_write_vectored: nightly only
    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.inner {
            StreamInner::PassThrough(w) => w.as_locked_write().flush(),
            StreamInner::Strip(w) => w.flush(),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => w.flush(),
        }
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match &mut self.inner {
            StreamInner::PassThrough(w) => w.as_locked_write().write_all(buf),
            StreamInner::Strip(w) => w.write_all(buf),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => w.write_all(buf),
        }
    }
    // write_all_vectored: nightly only
    #[inline]
    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        match &mut self.inner {
            StreamInner::PassThrough(w) => w.as_locked_write().write_fmt(args),
            StreamInner::Strip(w) => w.write_fmt(args),
            #[cfg(all(windows, feature = "wincon"))]
            StreamInner::Wincon(w) => w.write_fmt(args),
        }
    }
}
