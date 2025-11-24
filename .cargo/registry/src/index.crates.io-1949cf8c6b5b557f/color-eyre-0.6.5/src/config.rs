//! Configuration options for customizing the behavior of the provided panic
//! and error reporting hooks
use crate::{
    section::PanicMessage,
    writers::{EnvSection, WriterExt},
};
use fmt::Display;
use indenter::{indented, Format};
use owo_colors::{style, OwoColorize, Style};
use std::env;
use std::fmt::Write as _;
use std::{fmt, path::PathBuf, sync::Arc};

/// A struct that represents a theme that is used by `color_eyre`
#[derive(Debug, Copy, Clone, Default)]
pub struct Theme {
    pub(crate) file: Style,
    pub(crate) line_number: Style,
    pub(crate) spantrace_target: Style,
    pub(crate) spantrace_fields: Style,
    pub(crate) active_line: Style,
    pub(crate) error: Style,
    pub(crate) help_info_note: Style,
    pub(crate) help_info_warning: Style,
    pub(crate) help_info_suggestion: Style,
    pub(crate) help_info_error: Style,
    pub(crate) dependency_code: Style,
    pub(crate) crate_code: Style,
    pub(crate) code_hash: Style,
    pub(crate) panic_header: Style,
    pub(crate) panic_message: Style,
    pub(crate) panic_file: Style,
    pub(crate) panic_line_number: Style,
    pub(crate) hidden_frames: Style,
}

macro_rules! theme_setters {
    ($(#[$meta:meta] $name:ident),* $(,)?) => {
        $(
            #[$meta]
            pub fn $name(mut self, style: Style) -> Self {
                self.$name = style;
                self
            }
        )*
    };
}

impl Theme {
    /// Creates a blank theme
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a theme for dark backgrounds. This is the default
    pub fn dark() -> Self {
        Self {
            file: style().purple(),
            line_number: style().purple(),
            active_line: style().white().bold(),
            error: style().bright_red(),
            help_info_note: style().bright_cyan(),
            help_info_warning: style().bright_yellow(),
            help_info_suggestion: style().bright_cyan(),
            help_info_error: style().bright_red(),
            dependency_code: style().green(),
            crate_code: style().bright_red(),
            code_hash: style().bright_black(),
            panic_header: style().red(),
            panic_message: style().cyan(),
            panic_file: style().purple(),
            panic_line_number: style().purple(),
            hidden_frames: style().bright_cyan(),
            spantrace_target: style().bright_red(),
            spantrace_fields: style().bright_cyan(),
        }
    }

    // XXX it would be great, if someone with more style optimizes the light theme below. I just fixed the biggest problems, but ideally there would be darker colors (however, the standard ANSI colors don't seem to have many dark enough colors. Maybe xterm colors or RGB colors would be better (however, again, see my comment regarding xterm colors in `color_spantrace`))

    /// Returns a theme for light backgrounds
    pub fn light() -> Self {
        Self {
            file: style().purple(),
            line_number: style().purple(),
            spantrace_target: style().red(),
            spantrace_fields: style().blue(),
            active_line: style().bold(),
            error: style().red(),
            help_info_note: style().blue(),
            help_info_warning: style().bright_red(),
            help_info_suggestion: style().blue(),
            help_info_error: style().red(),
            dependency_code: style().green(),
            crate_code: style().red(),
            code_hash: style().bright_black(),
            panic_header: style().red(),
            panic_message: style().blue(),
            panic_file: style().purple(),
            panic_line_number: style().purple(),
            hidden_frames: style().blue(),
        }
    }

    theme_setters! {
        /// Styles printed paths
        file,
        /// Styles the line number of a file
        line_number,
        /// Styles the `color_spantrace` target (i.e. the module and function name, and so on)
        spantrace_target,
        /// Styles fields associated with a the `tracing::Span`.
        spantrace_fields,
        /// Styles the selected line of displayed code
        active_line,
        // XXX not sure how to describe this better (or if this is even completely correct)
        /// Styles errors printed by `EyreHandler`
        error,
        /// Styles the "note" section header
        help_info_note,
        /// Styles the "warning" section header
        help_info_warning,
        /// Styles the "suggestion" section header
        help_info_suggestion,
        /// Styles the "error" section header
        help_info_error,
        /// Styles code that is not part of your crate
        dependency_code,
        /// Styles code that's in your crate
        crate_code,
        /// Styles the hash after `dependency_code` and `crate_code`
        code_hash,
        /// Styles the header of a panic
        panic_header,
        /// Styles the message of a panic
        panic_message,
        /// Styles paths of a panic
        panic_file,
        /// Styles the line numbers of a panic
        panic_line_number,
        /// Styles the "N frames hidden" message
        hidden_frames,
    }
}

/// A representation of a Frame from a Backtrace or a SpanTrace
#[derive(Debug)]
#[non_exhaustive]
pub struct Frame {
    /// Frame index
    pub n: usize,
    /// frame symbol name
    pub name: Option<String>,
    /// source line number
    pub lineno: Option<u32>,
    /// source file path
    pub filename: Option<PathBuf>,
}

#[derive(Debug)]
struct StyledFrame<'a>(&'a Frame, Theme);

impl fmt::Display for StyledFrame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(frame, theme) = self;

        let is_dependency_code = frame.is_dependency_code();

        // Print frame index.
        write!(f, "{:>2}: ", frame.n)?;

        // Does the function have a hash suffix?
        // (dodging a dep on the regex crate here)
        let name = frame.name.as_deref().unwrap_or("<unknown>");
        let has_hash_suffix = name.len() > 19
            && &name[name.len() - 19..name.len() - 16] == "::h"
            && name[name.len() - 16..]
                .chars()
                .all(|x| x.is_ascii_hexdigit());

        let hash_suffix = if has_hash_suffix {
            &name[name.len() - 19..]
        } else {
            "<unknown>"
        };

        // Print function name.
        let name = if has_hash_suffix {
            &name[..name.len() - 19]
        } else {
            name
        };

        if is_dependency_code {
            write!(f, "{}", (name).style(theme.dependency_code))?;
        } else {
            write!(f, "{}", (name).style(theme.crate_code))?;
        }

        write!(f, "{}", (hash_suffix).style(theme.code_hash))?;

        let mut separated = f.header("\n");

        // Print source location, if known.
        let file = frame.filename.as_ref().map(|path| path.display());
        let file: &dyn fmt::Display = if let Some(ref filename) = file {
            filename
        } else {
            &"<unknown source file>"
        };
        let lineno = frame
            .lineno
            .map_or("<unknown line>".to_owned(), |x| x.to_string());
        write!(
            &mut separated.ready(),
            "    at {}:{}",
            file.style(theme.file),
            lineno.style(theme.line_number),
        )?;

        let v = if std::thread::panicking() {
            panic_verbosity()
        } else {
            lib_verbosity()
        };

        // Maybe print source.
        if v >= Verbosity::Full {
            write!(&mut separated.ready(), "{}", SourceSection(frame, *theme))?;
        }

        Ok(())
    }
}

struct SourceSection<'a>(&'a Frame, Theme);

impl fmt::Display for SourceSection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(frame, theme) = self;

        let (lineno, filename) = match (frame.lineno, frame.filename.as_ref()) {
            (Some(a), Some(b)) => (a, b),
            // Without a line number and file name, we can't sensibly proceed.
            _ => return Ok(()),
        };

        let file = match std::fs::File::open(filename) {
            Ok(file) => file,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            e @ Err(_) => e.unwrap(),
        };

        use std::fmt::Write;
        use std::io::BufRead;

        // Extract relevant lines.
        let reader = std::io::BufReader::new(file);
        let start_line = lineno - 2.min(lineno - 1);
        let surrounding_src = reader.lines().skip(start_line as usize - 1).take(5);
        let mut separated = f.header("\n");
        let mut f = separated.in_progress();
        for (line, cur_line_no) in surrounding_src.zip(start_line..) {
            let line = line.unwrap();
            if cur_line_no == lineno {
                write!(
                    &mut f,
                    "{:>8} {} {}",
                    cur_line_no.style(theme.active_line),
                    ">".style(theme.active_line),
                    line.style(theme.active_line),
                )?;
            } else {
                write!(&mut f, "{:>8} │ {}", cur_line_no, line)?;
            }
            f = separated.ready();
        }

        Ok(())
    }
}

impl Frame {
    fn is_dependency_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "std::",
            "core::",
            "backtrace::backtrace::",
            "_rust_begin_unwind",
            "color_traceback::",
            "__rust_",
            "___rust_",
            "__pthread",
            "_main",
            "main",
            "__scrt_common_main_seh",
            "BaseThreadInitThunk",
            "_start",
            "__libc_start_main",
            "start_thread",
        ];

        // Inspect name.
        if let Some(ref name) = self.name {
            if SYM_PREFIXES.iter().any(|x| name.starts_with(x)) {
                return true;
            }
        }

        const FILE_PREFIXES: &[&str] = &[
            "/rustc/",
            "src/libstd/",
            "src/libpanic_unwind/",
            "src/libtest/",
        ];

        // Inspect filename.
        if let Some(ref filename) = self.filename {
            let filename = filename.to_string_lossy();
            if FILE_PREFIXES.iter().any(|x| filename.starts_with(x))
                || filename.contains("/.cargo/registry/src/")
            {
                return true;
            }
        }

        false
    }

    /// Heuristically determine whether a frame is likely to be a post panic
    /// frame.
    ///
    /// Post panic frames are frames of a functions called after the actual panic
    /// is already in progress and don't contain any useful information for a
    /// reader of the backtrace.
    fn is_post_panic_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "_rust_begin_unwind",
            "rust_begin_unwind",
            "core::result::unwrap_failed",
            "core::option::expect_none_failed",
            "core::panicking::panic_fmt",
            "color_backtrace::create_panic_handler",
            "std::panicking::begin_panic",
            "begin_panic_fmt",
            "failure::backtrace::Backtrace::new",
            "backtrace::capture",
            "failure::error_message::err_msg",
            "<failure::error::Error as core::convert::From<F>>::from",
        ];

        match self.name.as_ref() {
            Some(name) => SYM_PREFIXES.iter().any(|x| name.starts_with(x)),
            None => false,
        }
    }

    /// Heuristically determine whether a frame is likely to be part of language
    /// runtime.
    fn is_runtime_init_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "std::rt::lang_start::",
            "test::run_test::run_test_inner::",
            "std::sys_common::backtrace::__rust_begin_short_backtrace",
        ];

        let (name, file) = match (self.name.as_ref(), self.filename.as_ref()) {
            (Some(name), Some(filename)) => (name, filename.to_string_lossy()),
            _ => return false,
        };

        if SYM_PREFIXES.iter().any(|x| name.starts_with(x)) {
            return true;
        }

        // For Linux, this is the best rule for skipping test init I found.
        if name == "{{closure}}" && file == "src/libtest/lib.rs" {
            return true;
        }

        false
    }
}

/// Builder for customizing the behavior of the global panic and error report hooks
pub struct HookBuilder {
    filters: Vec<Box<FilterCallback>>,
    capture_span_trace_by_default: bool,
    display_env_section: bool,
    #[cfg(feature = "track-caller")]
    display_location_section: bool,
    panic_section: Option<Box<dyn Display + Send + Sync + 'static>>,
    panic_message: Option<Box<dyn PanicMessage>>,
    theme: Theme,
    #[cfg(feature = "issue-url")]
    issue_url: Option<String>,
    #[cfg(feature = "issue-url")]
    issue_metadata: Vec<(String, Box<dyn Display + Send + Sync + 'static>)>,
    #[cfg(feature = "issue-url")]
    issue_filter: Arc<IssueFilterCallback>,
}

impl HookBuilder {
    /// Construct a HookBuilder
    ///
    /// # Details
    ///
    /// By default this function calls `add_default_filters()` and
    /// `capture_span_trace_by_default(true)`. To get a `HookBuilder` with all
    /// features disabled by default call `HookBuilder::blank()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use color_eyre::config::HookBuilder;
    ///
    /// HookBuilder::new()
    ///     .install()
    ///     .unwrap();
    /// ```
    pub fn new() -> Self {
        Self::blank()
            .add_default_filters()
            .capture_span_trace_by_default(true)
    }

    /// Construct a HookBuilder with minimal features enabled
    pub fn blank() -> Self {
        HookBuilder {
            filters: vec![],
            capture_span_trace_by_default: false,
            display_env_section: true,
            #[cfg(feature = "track-caller")]
            display_location_section: true,
            panic_section: None,
            panic_message: None,
            theme: Theme::dark(),
            #[cfg(feature = "issue-url")]
            issue_url: None,
            #[cfg(feature = "issue-url")]
            issue_metadata: vec![],
            #[cfg(feature = "issue-url")]
            issue_filter: Arc::new(|_| true),
        }
    }

    /// Set the global styles that `color_eyre` should use.
    ///
    /// **Tip:** You can test new styles by editing `examples/theme.rs` in the `color-eyre` repository.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Add a custom section to the panic hook that will be printed
    /// in the panic message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// color_eyre::config::HookBuilder::default()
    ///     .panic_section("consider reporting the bug at https://github.com/eyre-rs/eyre/issues")
    ///     .install()
    ///     .unwrap()
    /// ```
    pub fn panic_section<S: Display + Send + Sync + 'static>(mut self, section: S) -> Self {
        self.panic_section = Some(Box::new(section));
        self
    }

    /// Overrides the main error message printing section at the start of panic
    /// reports
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::{panic::Location, fmt};
    /// use color_eyre::section::PanicMessage;
    /// use owo_colors::OwoColorize;
    ///
    /// struct MyPanicMessage;
    ///
    /// color_eyre::config::HookBuilder::default()
    ///     .panic_message(MyPanicMessage)
    ///     .install()
    ///     .unwrap();
    ///
    /// impl PanicMessage for MyPanicMessage {
    ///     fn display(&self, pi: &std::panic::PanicInfo<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         writeln!(f, "{}", "The application panicked (crashed).".red())?;
    ///
    ///         // Print panic message.
    ///         let payload = pi
    ///             .payload()
    ///             .downcast_ref::<String>()
    ///             .map(String::as_str)
    ///             .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
    ///             .unwrap_or("<non string panic payload>");
    ///
    ///         write!(f, "Message:  ")?;
    ///         writeln!(f, "{}", payload.cyan())?;
    ///
    ///         // If known, print panic location.
    ///         write!(f, "Location: ")?;
    ///         if let Some(loc) = pi.location() {
    ///             write!(f, "{}", loc.file().purple())?;
    ///             write!(f, ":")?;
    ///             write!(f, "{}", loc.line().purple())?;
    ///
    ///             write!(f, "\n\nConsider reporting the bug at {}", custom_url(loc, payload))?;
    ///         } else {
    ///             write!(f, "<unknown>")?;
    ///         }
    ///
    ///         Ok(())
    ///     }
    /// }
    ///
    /// fn custom_url(location: &Location<'_>, message: &str) -> impl fmt::Display {
    ///     "todo"
    /// }
    /// ```
    pub fn panic_message<S: PanicMessage>(mut self, section: S) -> Self {
        self.panic_message = Some(Box::new(section));
        self
    }

    /// Set an upstream github repo and enable issue reporting url generation
    ///
    /// # Details
    ///
    /// Once enabled, color-eyre will generate urls that will create customized
    /// issues pre-populated with information about the associated error report.
    ///
    /// Additional information can be added to the metadata table in the
    /// generated urls by calling `add_issue_metadata` when configuring the
    /// HookBuilder.
    ///
    /// # Examples
    ///
    /// ```rust
    /// color_eyre::config::HookBuilder::default()
    ///     .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
    ///     .install()
    ///     .unwrap();
    /// ```
    #[cfg(feature = "issue-url")]
    #[cfg_attr(docsrs, doc(cfg(feature = "issue-url")))]
    pub fn issue_url<S: ToString>(mut self, url: S) -> Self {
        self.issue_url = Some(url.to_string());
        self
    }

    /// Add a new entry to the metadata table in generated github issue urls
    ///
    /// **Note**: this metadata will be ignored if no `issue_url` is set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// color_eyre::config::HookBuilder::default()
    ///     .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
    ///     .add_issue_metadata("version", env!("CARGO_PKG_VERSION"))
    ///     .install()
    ///     .unwrap();
    /// ```
    #[cfg(feature = "issue-url")]
    #[cfg_attr(docsrs, doc(cfg(feature = "issue-url")))]
    pub fn add_issue_metadata<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Display,
        V: Display + Send + Sync + 'static,
    {
        let pair = (key.to_string(), Box::new(value) as _);
        self.issue_metadata.push(pair);
        self
    }

    /// Configures a filter for disabling issue url generation for certain kinds of errors
    ///
    /// If the closure returns `true`, then the issue url will be generated.
    ///
    /// # Examples
    ///
    /// ```rust
    /// color_eyre::config::HookBuilder::default()
    ///     .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
    ///     .issue_filter(|kind| match kind {
    ///         color_eyre::ErrorKind::NonRecoverable(payload) => {
    ///             let payload = payload
    ///                 .downcast_ref::<String>()
    ///                 .map(String::as_str)
    ///                 .or_else(|| payload.downcast_ref::<&str>().cloned())
    ///                 .unwrap_or("<non string panic payload>");
    ///
    ///             !payload.contains("my irrelevant error message")
    ///         },
    ///         color_eyre::ErrorKind::Recoverable(error) => !error.is::<std::fmt::Error>(),
    ///     })
    ///     .install()
    ///     .unwrap();
    ///
    #[cfg(feature = "issue-url")]
    #[cfg_attr(docsrs, doc(cfg(feature = "issue-url")))]
    pub fn issue_filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(crate::ErrorKind<'_>) -> bool + Send + Sync + 'static,
    {
        self.issue_filter = Arc::new(predicate);
        self
    }

    /// Configures the default capture mode for `SpanTraces` in error reports and panics
    pub fn capture_span_trace_by_default(mut self, cond: bool) -> Self {
        self.capture_span_trace_by_default = cond;
        self
    }

    /// Configures the enviroment varible info section and whether or not it is displayed
    pub fn display_env_section(mut self, cond: bool) -> Self {
        self.display_env_section = cond;
        self
    }

    /// Configures the location info section and whether or not it is displayed.
    ///
    /// # Notes
    ///
    /// This will not disable the location section in a panic message.
    #[cfg(feature = "track-caller")]
    #[cfg_attr(docsrs, doc(cfg(feature = "track-caller")))]
    pub fn display_location_section(mut self, cond: bool) -> Self {
        self.display_location_section = cond;
        self
    }

    /// Add a custom filter to the set of frame filters
    ///
    /// # Examples
    ///
    /// ```rust
    /// color_eyre::config::HookBuilder::default()
    ///     .add_frame_filter(Box::new(|frames| {
    ///         let filters = &[
    ///             "uninteresting_function",
    ///         ];
    ///
    ///         frames.retain(|frame| {
    ///             !filters.iter().any(|f| {
    ///                 let name = if let Some(name) = frame.name.as_ref() {
    ///                     name.as_str()
    ///                 } else {
    ///                     return true;
    ///                 };
    ///
    ///                 name.starts_with(f)
    ///             })
    ///         });
    ///     }))
    ///     .install()
    ///     .unwrap();
    /// ```
    pub fn add_frame_filter(mut self, filter: Box<FilterCallback>) -> Self {
        self.filters.push(filter);
        self
    }

    /// Install the given Hook as the global error report hook
    pub fn install(self) -> Result<(), crate::eyre::Report> {
        let (panic_hook, eyre_hook) = self.try_into_hooks()?;
        eyre_hook.install()?;
        panic_hook.install();
        Ok(())
    }

    /// Add the default set of filters to this `HookBuilder`'s configuration
    pub fn add_default_filters(self) -> Self {
        self.add_frame_filter(Box::new(default_frame_filter))
            .add_frame_filter(Box::new(eyre_frame_filters))
    }

    /// Create a `PanicHook` and `EyreHook` from this `HookBuilder`.
    /// This can be used if you want to combine these handlers with other handlers.
    pub fn into_hooks(self) -> (PanicHook, EyreHook) {
        self.try_into_hooks().expect("into_hooks should only be called when no `color_spantrace` themes have previously been set")
    }

    /// Create a `PanicHook` and `EyreHook` from this `HookBuilder`.
    /// This can be used if you want to combine these handlers with other handlers.
    pub fn try_into_hooks(self) -> Result<(PanicHook, EyreHook), crate::eyre::Report> {
        let theme = self.theme;
        #[cfg(feature = "issue-url")]
        let metadata = Arc::new(self.issue_metadata);
        let panic_hook = PanicHook {
            filters: self.filters.into(),
            section: self.panic_section,
            #[cfg(feature = "capture-spantrace")]
            capture_span_trace_by_default: self.capture_span_trace_by_default,
            display_env_section: self.display_env_section,
            panic_message: self
                .panic_message
                .unwrap_or_else(|| Box::new(DefaultPanicMessage(theme))),
            theme,
            #[cfg(feature = "issue-url")]
            issue_url: self.issue_url.clone(),
            #[cfg(feature = "issue-url")]
            issue_metadata: metadata.clone(),
            #[cfg(feature = "issue-url")]
            issue_filter: self.issue_filter.clone(),
        };

        let eyre_hook = EyreHook {
            filters: panic_hook.filters.clone(),
            #[cfg(feature = "capture-spantrace")]
            capture_span_trace_by_default: self.capture_span_trace_by_default,
            display_env_section: self.display_env_section,
            #[cfg(feature = "track-caller")]
            display_location_section: self.display_location_section,
            theme,
            #[cfg(feature = "issue-url")]
            issue_url: self.issue_url,
            #[cfg(feature = "issue-url")]
            issue_metadata: metadata,
            #[cfg(feature = "issue-url")]
            issue_filter: self.issue_filter,
        };

        #[cfg(feature = "capture-spantrace")]
        eyre::WrapErr::wrap_err(color_spantrace::set_theme(self.theme.into()), "could not set the provided `Theme` via `color_spantrace::set_theme` globally as another was already set")?;

        Ok((panic_hook, eyre_hook))
    }
}

#[cfg(feature = "capture-spantrace")]
impl From<Theme> for color_spantrace::Theme {
    fn from(src: Theme) -> color_spantrace::Theme {
        color_spantrace::Theme::new()
            .file(src.file)
            .line_number(src.line_number)
            .target(src.spantrace_target)
            .fields(src.spantrace_fields)
            .active_line(src.active_line)
    }
}

#[allow(missing_docs)]
impl Default for HookBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn default_frame_filter(frames: &mut Vec<&Frame>) {
    let top_cutoff = frames
        .iter()
        .rposition(|x| x.is_post_panic_code())
        .map(|x| x + 2) // indices are 1 based
        .unwrap_or(0);

    let bottom_cutoff = frames
        .iter()
        .position(|x| x.is_runtime_init_code())
        .unwrap_or(frames.len());

    let rng = top_cutoff..=bottom_cutoff;
    frames.retain(|x| rng.contains(&x.n))
}

fn eyre_frame_filters(frames: &mut Vec<&Frame>) {
    let filters = &[
        "<color_eyre::Handler as eyre::EyreHandler>::default",
        "eyre::",
        "color_eyre::",
    ];

    frames.retain(|frame| {
        !filters.iter().any(|f| {
            let name = if let Some(name) = frame.name.as_ref() {
                name.as_str()
            } else {
                return true;
            };

            name.starts_with(f)
        })
    });
}

struct DefaultPanicMessage(Theme);

impl PanicMessage for DefaultPanicMessage {
    fn display(&self, pi: &std::panic::PanicInfo<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // XXX is my assumption correct that this function is guaranteed to only run after `color_eyre` was setup successfully (including setting `THEME`), and that therefore the following line will never panic? Otherwise, we could return `fmt::Error`, but if the above is true, I like `unwrap` + a comment why this never fails better
        let theme = &self.0;

        writeln!(
            f,
            "{}",
            "The application panicked (crashed).".style(theme.panic_header)
        )?;

        // Print panic message.
        let payload = pi
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
            .unwrap_or("<non string panic payload>");

        write!(f, "Message:  ")?;
        writeln!(f, "{}", payload.style(theme.panic_message))?;

        // If known, print panic location.
        write!(f, "Location: ")?;
        write!(f, "{}", crate::fmt::LocationSection(pi.location(), *theme))?;

        Ok(())
    }
}

/// A type representing an error report for a panic.
pub struct PanicReport<'a> {
    hook: &'a PanicHook,
    panic_info: &'a std::panic::PanicInfo<'a>,
    backtrace: Option<backtrace::Backtrace>,
    #[cfg(feature = "capture-spantrace")]
    span_trace: Option<tracing_error::SpanTrace>,
}

fn print_panic_info(report: &PanicReport<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    report.hook.panic_message.display(report.panic_info, f)?;

    let v = panic_verbosity();
    let capture_bt = v != Verbosity::Minimal;

    let mut separated = f.header("\n\n");

    if let Some(ref section) = report.hook.section {
        write!(&mut separated.ready(), "{}", section)?;
    }

    #[cfg(feature = "capture-spantrace")]
    {
        if let Some(span_trace) = report.span_trace.as_ref() {
            write!(
                &mut separated.ready(),
                "{}",
                crate::writers::FormattedSpanTrace(span_trace)
            )?;
        }
    }

    if let Some(bt) = report.backtrace.as_ref() {
        let fmted_bt = report.hook.format_backtrace(bt);
        write!(
            indented(&mut separated.ready()).with_format(Format::Uniform { indentation: "  " }),
            "{}",
            fmted_bt
        )?;
    }

    if report.hook.display_env_section {
        let env_section = EnvSection {
            bt_captured: &capture_bt,
            #[cfg(feature = "capture-spantrace")]
            span_trace: report.span_trace.as_ref(),
        };

        write!(&mut separated.ready(), "{}", env_section)?;
    }

    #[cfg(feature = "issue-url")]
    {
        let payload = report.panic_info.payload();

        if report.hook.issue_url.is_some()
            && (*report.hook.issue_filter)(crate::ErrorKind::NonRecoverable(payload))
        {
            let url = report.hook.issue_url.as_ref().unwrap();
            let payload = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().cloned())
                .unwrap_or("<non string panic payload>");

            let issue_section = crate::section::github::IssueSection::new(url, payload)
                .with_backtrace(report.backtrace.as_ref())
                .with_location(report.panic_info.location())
                .with_metadata(&report.hook.issue_metadata);

            #[cfg(feature = "capture-spantrace")]
            let issue_section = issue_section.with_span_trace(report.span_trace.as_ref());

            write!(&mut separated.ready(), "{}", issue_section)?;
        }
    }

    Ok(())
}

impl fmt::Display for PanicReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        print_panic_info(self, f)
    }
}

/// A panic reporting hook
pub struct PanicHook {
    filters: Arc<[Box<FilterCallback>]>,
    section: Option<Box<dyn Display + Send + Sync + 'static>>,
    panic_message: Box<dyn PanicMessage>,
    theme: Theme,
    #[cfg(feature = "capture-spantrace")]
    capture_span_trace_by_default: bool,
    display_env_section: bool,
    #[cfg(feature = "issue-url")]
    issue_url: Option<String>,
    #[cfg(feature = "issue-url")]
    issue_metadata: Arc<Vec<(String, Box<dyn Display + Send + Sync + 'static>)>>,
    #[cfg(feature = "issue-url")]
    issue_filter: Arc<IssueFilterCallback>,
}

impl PanicHook {
    pub(crate) fn format_backtrace<'a>(
        &'a self,
        trace: &'a backtrace::Backtrace,
    ) -> BacktraceFormatter<'a> {
        BacktraceFormatter {
            filters: &self.filters,
            inner: trace,
            theme: self.theme,
        }
    }

    #[cfg(feature = "capture-spantrace")]
    fn spantrace_capture_enabled(&self) -> bool {
        std::env::var("RUST_SPANTRACE")
            .map(|val| val != "0")
            .unwrap_or(self.capture_span_trace_by_default)
    }

    /// Install self as a global panic hook via `std::panic::set_hook`.
    pub fn install(self) {
        std::panic::set_hook(self.into_panic_hook());
    }

    /// Convert self into the type expected by `std::panic::set_hook`.
    pub fn into_panic_hook(
        self,
    ) -> Box<dyn Fn(&std::panic::PanicInfo<'_>) + Send + Sync + 'static> {
        Box::new(move |panic_info| {
            eprintln!("{}", self.panic_report(panic_info));
        })
    }

    /// Construct a panic reporter which prints it's panic report via the
    /// `Display` trait.
    pub fn panic_report<'a>(
        &'a self,
        panic_info: &'a std::panic::PanicInfo<'_>,
    ) -> PanicReport<'a> {
        let v = panic_verbosity();
        let capture_bt = v != Verbosity::Minimal;

        #[cfg(feature = "capture-spantrace")]
        let span_trace = if self.spantrace_capture_enabled() {
            Some(tracing_error::SpanTrace::capture())
        } else {
            None
        };

        let backtrace = if capture_bt {
            Some(backtrace::Backtrace::new())
        } else {
            None
        };

        PanicReport {
            panic_info,
            #[cfg(feature = "capture-spantrace")]
            span_trace,
            backtrace,
            hook: self,
        }
    }
}

/// An eyre reporting hook used to construct `EyreHandler`s
pub struct EyreHook {
    filters: Arc<[Box<FilterCallback>]>,
    #[cfg(feature = "capture-spantrace")]
    capture_span_trace_by_default: bool,
    display_env_section: bool,
    #[cfg(feature = "track-caller")]
    display_location_section: bool,
    theme: Theme,
    #[cfg(feature = "issue-url")]
    issue_url: Option<String>,
    #[cfg(feature = "issue-url")]
    issue_metadata: Arc<Vec<(String, Box<dyn Display + Send + Sync + 'static>)>>,
    #[cfg(feature = "issue-url")]
    issue_filter: Arc<IssueFilterCallback>,
}

type HookFunc = Box<
    dyn Fn(&(dyn std::error::Error + 'static)) -> Box<dyn eyre::EyreHandler>
        + Send
        + Sync
        + 'static,
>;

impl EyreHook {
    #[allow(unused_variables)]
    pub(crate) fn default(&self, error: &(dyn std::error::Error + 'static)) -> crate::Handler {
        let backtrace = if lib_verbosity() != Verbosity::Minimal {
            Some(backtrace::Backtrace::new())
        } else {
            None
        };

        #[cfg(feature = "capture-spantrace")]
        let span_trace = if self.spantrace_capture_enabled()
            && crate::handler::get_deepest_spantrace(error).is_none()
        {
            Some(tracing_error::SpanTrace::capture())
        } else {
            None
        };

        crate::Handler {
            filters: self.filters.clone(),
            backtrace,
            suppress_backtrace: false,
            #[cfg(feature = "capture-spantrace")]
            span_trace,
            sections: Vec::new(),
            display_env_section: self.display_env_section,
            #[cfg(feature = "track-caller")]
            display_location_section: self.display_location_section,
            #[cfg(feature = "issue-url")]
            issue_url: self.issue_url.clone(),
            #[cfg(feature = "issue-url")]
            issue_metadata: self.issue_metadata.clone(),
            #[cfg(feature = "issue-url")]
            issue_filter: self.issue_filter.clone(),
            theme: self.theme,
            #[cfg(feature = "track-caller")]
            location: None,
        }
    }

    #[cfg(feature = "capture-spantrace")]
    fn spantrace_capture_enabled(&self) -> bool {
        std::env::var("RUST_SPANTRACE")
            .map(|val| val != "0")
            .unwrap_or(self.capture_span_trace_by_default)
    }

    /// Installs self as the global eyre handling hook via `eyre::set_hook`
    pub fn install(self) -> Result<(), crate::eyre::InstallError> {
        crate::eyre::set_hook(self.into_eyre_hook())
    }

    /// Convert the self into the boxed type expected by `eyre::set_hook`.
    pub fn into_eyre_hook(self) -> HookFunc {
        Box::new(move |e| Box::new(self.default(e)))
    }
}

pub(crate) struct BacktraceFormatter<'a> {
    pub(crate) filters: &'a [Box<FilterCallback>],
    pub(crate) inner: &'a backtrace::Backtrace,
    pub(crate) theme: Theme,
}

impl fmt::Display for BacktraceFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:━^80}", " BACKTRACE ")?;

        // Collect frame info.
        let frames: Vec<_> = self
            .inner
            .frames()
            .iter()
            .flat_map(|frame| frame.symbols())
            .zip(1usize..)
            .map(|(sym, n)| Frame {
                name: sym.name().map(|x| x.to_string()),
                lineno: sym.lineno(),
                filename: sym.filename().map(|x| x.into()),
                n,
            })
            .collect();

        let mut filtered_frames = frames.iter().collect();
        match env::var("COLORBT_SHOW_HIDDEN").ok().as_deref() {
            Some("1") | Some("on") | Some("y") => (),
            _ => {
                for filter in self.filters {
                    filter(&mut filtered_frames);
                }
            }
        }

        if filtered_frames.is_empty() {
            // TODO: Would probably look better centered.
            return write!(f, "\n<empty backtrace>");
        }

        let mut separated = f.header("\n");

        // Don't let filters mess with the order.
        filtered_frames.sort_by_key(|x| x.n);

        let mut buf = String::new();

        macro_rules! print_hidden {
            ($n:expr) => {
                let n = $n;
                buf.clear();
                write!(
                    &mut buf,
                    "{decorator} {n} frame{plural} hidden {decorator}",
                    n = n,
                    plural = if n == 1 { "" } else { "s" },
                    decorator = "⋮",
                )
                .expect("writing to strings doesn't panic");
                write!(
                    &mut separated.ready(),
                    "{:^80}",
                    buf.style(self.theme.hidden_frames)
                )?;
            };
        }

        let mut last_n = 0;
        for frame in &filtered_frames {
            let frame_delta = frame.n - last_n - 1;
            if frame_delta != 0 {
                print_hidden!(frame_delta);
            }
            write!(&mut separated.ready(), "{}", StyledFrame(frame, self.theme))?;
            last_n = frame.n;
        }

        let last_filtered_n = filtered_frames.last().unwrap().n;
        let last_unfiltered_n = frames.last().unwrap().n;
        if last_filtered_n < last_unfiltered_n {
            print_hidden!(last_unfiltered_n - last_filtered_n);
        }

        Ok(())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Verbosity {
    Minimal,
    Medium,
    Full,
}

pub(crate) fn panic_verbosity() -> Verbosity {
    match env::var("RUST_BACKTRACE") {
        Ok(s) if s == "full" => Verbosity::Full,
        Ok(s) if s != "0" => Verbosity::Medium,
        _ => Verbosity::Minimal,
    }
}

pub(crate) fn lib_verbosity() -> Verbosity {
    match env::var("RUST_LIB_BACKTRACE").or_else(|_| env::var("RUST_BACKTRACE")) {
        Ok(s) if s == "full" => Verbosity::Full,
        Ok(s) if s != "0" => Verbosity::Medium,
        _ => Verbosity::Minimal,
    }
}

/// Callback for filtering a vector of `Frame`s
pub type FilterCallback = dyn Fn(&mut Vec<&Frame>) + Send + Sync + 'static;

/// Callback for filtering issue url generation in error reports
#[cfg(feature = "issue-url")]
#[cfg_attr(docsrs, doc(cfg(feature = "issue-url")))]
pub type IssueFilterCallback = dyn Fn(crate::ErrorKind<'_>) -> bool + Send + Sync + 'static;
