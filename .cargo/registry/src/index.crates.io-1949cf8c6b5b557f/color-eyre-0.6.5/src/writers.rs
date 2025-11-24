use crate::config::{lib_verbosity, panic_verbosity, Verbosity};
use fmt::Write;
use std::fmt::{self, Display};
#[cfg(feature = "capture-spantrace")]
use tracing_error::{SpanTrace, SpanTraceStatus};

#[allow(explicit_outlives_requirements)]
pub(crate) struct HeaderWriter<'a, H, W>
where
    H: ?Sized,
{
    inner: W,
    header: &'a H,
    started: bool,
}

pub(crate) trait WriterExt: Sized {
    fn header<H: ?Sized>(self, header: &H) -> HeaderWriter<'_, H, Self>;
}

impl<W> WriterExt for W {
    fn header<H: ?Sized>(self, header: &H) -> HeaderWriter<'_, H, Self> {
        HeaderWriter {
            inner: self,
            header,
            started: false,
        }
    }
}

#[cfg(feature = "issue-url")]
pub(crate) trait DisplayExt: Sized + Display {
    fn with_header<H: Display>(self, header: H) -> Header<Self, H>;
    fn with_footer<F: Display>(self, footer: F) -> Footer<Self, F>;
}

#[cfg(feature = "issue-url")]
impl<T> DisplayExt for T
where
    T: Display,
{
    fn with_footer<F: Display>(self, footer: F) -> Footer<Self, F> {
        Footer { body: self, footer }
    }

    fn with_header<H: Display>(self, header: H) -> Header<Self, H> {
        Header {
            body: self,
            h: header,
        }
    }
}

pub(crate) struct ReadyHeaderWriter<'a, 'b, H: ?Sized, W>(&'b mut HeaderWriter<'a, H, W>);

impl<'a, H: ?Sized, W> HeaderWriter<'a, H, W> {
    pub(crate) fn ready(&mut self) -> ReadyHeaderWriter<'a, '_, H, W> {
        self.started = false;

        ReadyHeaderWriter(self)
    }

    pub(crate) fn in_progress(&mut self) -> ReadyHeaderWriter<'a, '_, H, W> {
        self.started = true;

        ReadyHeaderWriter(self)
    }
}

impl<H: ?Sized, W> fmt::Write for ReadyHeaderWriter<'_, '_, H, W>
where
    H: Display,
    W: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if !self.0.started && !s.is_empty() {
            self.0.inner.write_fmt(format_args!("{}", self.0.header))?;
            self.0.started = true;
        }

        self.0.inner.write_str(s)
    }
}

#[cfg(feature = "issue-url")]
pub(crate) struct FooterWriter<W> {
    inner: W,
    had_output: bool,
}

#[cfg(feature = "issue-url")]
impl<W> fmt::Write for FooterWriter<W>
where
    W: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if !self.had_output && !s.is_empty() {
            self.had_output = true;
        }

        self.inner.write_str(s)
    }
}

#[cfg(feature = "issue-url")]
#[allow(explicit_outlives_requirements)]
pub(crate) struct Footer<B, H>
where
    B: Display,
    H: Display,
{
    body: B,
    footer: H,
}

#[cfg(feature = "issue-url")]
impl<B, H> fmt::Display for Footer<B, H>
where
    B: Display,
    H: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut inner_f = FooterWriter {
            inner: &mut *f,
            had_output: false,
        };

        write!(&mut inner_f, "{}", self.body)?;

        if inner_f.had_output {
            self.footer.fmt(f)?;
        }

        Ok(())
    }
}

#[cfg(feature = "issue-url")]
#[allow(explicit_outlives_requirements)]
pub(crate) struct Header<B, H>
where
    B: Display,
    H: Display,
{
    body: B,
    h: H,
}

#[cfg(feature = "issue-url")]
impl<B, H> fmt::Display for Header<B, H>
where
    B: Display,
    H: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f.header(&self.h).ready(), "{}", self.body)?;

        Ok(())
    }
}

#[cfg(feature = "capture-spantrace")]
pub(crate) struct FormattedSpanTrace<'a>(pub(crate) &'a SpanTrace);

#[cfg(feature = "capture-spantrace")]
impl fmt::Display for FormattedSpanTrace<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use indenter::indented;
        use indenter::Format;

        if self.0.status() == SpanTraceStatus::CAPTURED {
            write!(
                indented(f).with_format(Format::Uniform { indentation: "  " }),
                "{}",
                color_spantrace::colorize(self.0)
            )?;
        }

        Ok(())
    }
}

pub(crate) struct EnvSection<'a> {
    pub(crate) bt_captured: &'a bool,
    #[cfg(feature = "capture-spantrace")]
    pub(crate) span_trace: Option<&'a SpanTrace>,
}

impl fmt::Display for EnvSection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = if std::thread::panicking() {
            panic_verbosity()
        } else {
            lib_verbosity()
        };
        write!(f, "{}", BacktraceOmited(!self.bt_captured))?;

        let mut separated = HeaderWriter {
            inner: &mut *f,
            header: &"\n",
            started: false,
        };
        write!(&mut separated.ready(), "{}", SourceSnippets(v))?;
        #[cfg(feature = "capture-spantrace")]
        write!(
            &mut separated.ready(),
            "{}",
            SpanTraceOmited(self.span_trace)
        )?;
        Ok(())
    }
}

#[cfg(feature = "capture-spantrace")]
struct SpanTraceOmited<'a>(Option<&'a SpanTrace>);

#[cfg(feature = "capture-spantrace")]
impl fmt::Display for SpanTraceOmited<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span_trace) = self.0 {
            if span_trace.status() == SpanTraceStatus::UNSUPPORTED {
                writeln!(f, "Warning: SpanTrace capture is Unsupported.")?;
                write!(
                    f,
                    "Ensure that you've setup a tracing-error ErrorLayer and the semver versions are compatible"
                )?;
            }
        }

        Ok(())
    }
}

struct BacktraceOmited(bool);

impl fmt::Display for BacktraceOmited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print some info on how to increase verbosity.
        if self.0 {
            write!(
                f,
                "Backtrace omitted. Run with RUST_BACKTRACE=1 environment variable to display it."
            )?;
        } else {
            // This text only makes sense if frames are displayed.
            write!(
                f,
                "Run with COLORBT_SHOW_HIDDEN=1 environment variable to disable frame filtering."
            )?;
        }

        Ok(())
    }
}

struct SourceSnippets(Verbosity);

impl fmt::Display for SourceSnippets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 <= Verbosity::Medium {
            write!(
                f,
                "Run with RUST_BACKTRACE=full to include source snippets."
            )?;
        }

        Ok(())
    }
}
