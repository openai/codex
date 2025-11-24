//! Error reporting

#![cfg_attr(not(feature = "error-context"), allow(dead_code))]
#![cfg_attr(not(feature = "error-context"), allow(unused_imports))]
#![cfg_attr(not(feature = "error-context"), allow(unused_variables))]
#![cfg_attr(not(feature = "error-context"), allow(unused_mut))]
#![cfg_attr(not(feature = "error-context"), allow(clippy::let_and_return))]

// Std
use std::{
    borrow::Cow,
    convert::From,
    error,
    fmt::{self, Debug, Display, Formatter},
    io,
    result::Result as StdResult,
};

// Internal
use crate::builder::StyledStr;
use crate::builder::Styles;
use crate::output::fmt::Colorizer;
use crate::output::fmt::Stream;
use crate::parser::features::suggestions;
use crate::util::FlatMap;
use crate::util::{color::ColorChoice, SUCCESS_CODE, USAGE_CODE};
use crate::Command;

#[cfg(feature = "error-context")]
mod context;
mod format;
mod kind;

pub use format::ErrorFormatter;
pub use format::KindFormatter;
pub use kind::ErrorKind;

#[cfg(feature = "error-context")]
pub use context::ContextKind;
#[cfg(feature = "error-context")]
pub use context::ContextValue;
#[cfg(feature = "error-context")]
pub use format::RichFormatter;

#[cfg(not(feature = "error-context"))]
pub use KindFormatter as DefaultFormatter;
#[cfg(feature = "error-context")]
pub use RichFormatter as DefaultFormatter;

/// Short hand for [`Result`] type
///
/// [`Result`]: std::result::Result
pub type Result<T, E = Error> = StdResult<T, E>;

/// Command Line Argument Parser Error
///
/// See [`Command::error`] to create an error.
///
/// [`Command::error`]: crate::Command::error
pub struct Error<F: ErrorFormatter = DefaultFormatter> {
    inner: Box<ErrorInner>,
    phantom: std::marker::PhantomData<F>,
}

#[derive(Debug)]
struct ErrorInner {
    kind: ErrorKind,
    #[cfg(feature = "error-context")]
    context: FlatMap<ContextKind, ContextValue>,
    message: Option<Message>,
    source: Option<Box<dyn error::Error + Send + Sync>>,
    help_flag: Option<Cow<'static, str>>,
    styles: Styles,
    color_when: ColorChoice,
    color_help_when: ColorChoice,
    backtrace: Option<Backtrace>,
}

impl<F: ErrorFormatter> Error<F> {
    /// Create an unformatted error
    ///
    /// This is for you need to pass the error up to
    /// a place that has access to the `Command` at which point you can call [`Error::format`].
    ///
    /// Prefer [`Command::error`] for generating errors.
    ///
    /// [`Command::error`]: crate::Command::error
    pub fn raw(kind: ErrorKind, message: impl Display) -> Self {
        Self::new(kind).set_message(message.to_string())
    }

    /// Format the existing message with the Command's context
    #[must_use]
    pub fn format(mut self, cmd: &mut Command) -> Self {
        cmd._build_self(false);
        let usage = cmd.render_usage_();
        if let Some(message) = self.inner.message.as_mut() {
            message.format(cmd, usage);
        }
        self.with_cmd(cmd)
    }

    /// Create an error with a pre-defined message
    ///
    /// See also
    /// - [`Error::insert`]
    /// - [`Error::with_cmd`]
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "error-context")] {
    /// # use clap_builder as clap;
    /// # use clap::error::ErrorKind;
    /// # use clap::error::ContextKind;
    /// # use clap::error::ContextValue;
    ///
    /// let cmd = clap::Command::new("prog");
    ///
    /// let mut err = clap::Error::new(ErrorKind::ValueValidation)
    ///     .with_cmd(&cmd);
    /// err.insert(ContextKind::InvalidArg, ContextValue::String("--foo".to_owned()));
    /// err.insert(ContextKind::InvalidValue, ContextValue::String("bar".to_owned()));
    ///
    /// err.print();
    /// # }
    /// ```
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            inner: Box::new(ErrorInner {
                kind,
                #[cfg(feature = "error-context")]
                context: FlatMap::new(),
                message: None,
                source: None,
                help_flag: None,
                styles: Styles::plain(),
                color_when: ColorChoice::Never,
                color_help_when: ColorChoice::Never,
                backtrace: Backtrace::new(),
            }),
            phantom: Default::default(),
        }
    }

    /// Apply [`Command`]'s formatting to the error
    ///
    /// Generally, this is used with [`Error::new`]
    pub fn with_cmd(self, cmd: &Command) -> Self {
        self.set_styles(cmd.get_styles().clone())
            .set_color(cmd.get_color())
            .set_colored_help(cmd.color_help())
            .set_help_flag(format::get_help_flag(cmd))
    }

    /// Apply an alternative formatter to the error
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// # use clap::Arg;
    /// # use clap::error::KindFormatter;
    /// let cmd = Command::new("foo")
    ///     .arg(Arg::new("input").required(true));
    /// let matches = cmd
    ///     .try_get_matches_from(["foo", "input.txt"])
    ///     .map_err(|e| e.apply::<KindFormatter>())
    ///     .unwrap_or_else(|e| e.exit());
    /// ```
    pub fn apply<EF: ErrorFormatter>(self) -> Error<EF> {
        Error {
            inner: self.inner,
            phantom: Default::default(),
        }
    }

    /// Type of error for programmatic processing
    pub fn kind(&self) -> ErrorKind {
        self.inner.kind
    }

    /// Additional information to further qualify the error
    #[cfg(feature = "error-context")]
    pub fn context(&self) -> impl Iterator<Item = (ContextKind, &ContextValue)> {
        self.inner.context.iter().map(|(k, v)| (*k, v))
    }

    /// Lookup a piece of context
    #[inline(never)]
    #[cfg(feature = "error-context")]
    pub fn get(&self, kind: ContextKind) -> Option<&ContextValue> {
        self.inner.context.get(&kind)
    }

    /// Insert a piece of context
    ///
    /// If this `ContextKind` is already present, its value is replaced and the old value is returned.
    #[inline(never)]
    #[cfg(feature = "error-context")]
    pub fn insert(&mut self, kind: ContextKind, value: ContextValue) -> Option<ContextValue> {
        self.inner.context.insert(kind, value)
    }

    /// Remove a piece of context, return the old value if any
    ///
    /// The context is currently implemented in a vector, so `remove` takes
    /// linear time.
    #[inline(never)]
    #[cfg(feature = "error-context")]
    pub fn remove(&mut self, kind: ContextKind) -> Option<ContextValue> {
        self.inner.context.remove(&kind)
    }

    /// Should the message be written to `stdout` or not?
    #[inline]
    pub fn use_stderr(&self) -> bool {
        self.stream() == Stream::Stderr
    }

    pub(crate) fn stream(&self) -> Stream {
        match self.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => Stream::Stdout,
            _ => Stream::Stderr,
        }
    }

    /// Returns the exit code that `.exit` will exit the process with.
    ///
    /// When the error's kind would print to `stderr` this returns `2`,
    /// else it returns `0`.
    pub fn exit_code(&self) -> i32 {
        if self.use_stderr() {
            USAGE_CODE
        } else {
            SUCCESS_CODE
        }
    }

    /// Prints the error and exits.
    ///
    /// Depending on the error kind, this either prints to `stderr` and exits with a status of `2`
    /// or prints to `stdout` and exits with a status of `0`.
    pub fn exit(&self) -> ! {
        // Swallow broken pipe errors
        let _ = self.print();
        std::process::exit(self.exit_code());
    }

    /// Prints formatted and colored error to `stdout` or `stderr` according to its error kind
    ///
    /// # Example
    /// ```no_run
    /// # use clap_builder as clap;
    /// use clap::Command;
    ///
    /// match Command::new("Command").try_get_matches() {
    ///     Ok(matches) => {
    ///         // do_something
    ///     },
    ///     Err(err) => {
    ///         err.print().expect("Error writing Error");
    ///         // do_something
    ///     },
    /// };
    /// ```
    pub fn print(&self) -> io::Result<()> {
        let style = self.formatted();
        let color_when = if matches!(
            self.kind(),
            ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        ) {
            self.inner.color_help_when
        } else {
            self.inner.color_when
        };
        let c = Colorizer::new(self.stream(), color_when).with_content(style.into_owned());
        c.print()
    }

    /// Render the error message to a [`StyledStr`].
    ///
    /// # Example
    /// ```no_run
    /// # use clap_builder as clap;
    /// use clap::Command;
    ///
    /// match Command::new("Command").try_get_matches() {
    ///     Ok(matches) => {
    ///         // do_something
    ///     },
    ///     Err(err) => {
    ///         let err = err.render();
    ///         println!("{err}");
    ///         // do_something
    ///     },
    /// };
    /// ```
    pub fn render(&self) -> StyledStr {
        self.formatted().into_owned()
    }

    #[inline(never)]
    fn for_app(kind: ErrorKind, cmd: &Command, styled: StyledStr) -> Self {
        Self::new(kind).set_message(styled).with_cmd(cmd)
    }

    pub(crate) fn set_message(mut self, message: impl Into<Message>) -> Self {
        self.inner.message = Some(message.into());
        self
    }

    pub(crate) fn set_source(mut self, source: Box<dyn error::Error + Send + Sync>) -> Self {
        self.inner.source = Some(source);
        self
    }

    pub(crate) fn set_styles(mut self, styles: Styles) -> Self {
        self.inner.styles = styles;
        self
    }

    pub(crate) fn set_color(mut self, color_when: ColorChoice) -> Self {
        self.inner.color_when = color_when;
        self
    }

    pub(crate) fn set_colored_help(mut self, color_help_when: ColorChoice) -> Self {
        self.inner.color_help_when = color_help_when;
        self
    }

    pub(crate) fn set_help_flag(mut self, help_flag: Option<Cow<'static, str>>) -> Self {
        self.inner.help_flag = help_flag;
        self
    }

    /// Does not verify if `ContextKind` is already present
    #[inline(never)]
    #[cfg(feature = "error-context")]
    pub(crate) fn insert_context_unchecked(
        mut self,
        kind: ContextKind,
        value: ContextValue,
    ) -> Self {
        self.inner.context.insert_unchecked(kind, value);
        self
    }

    /// Does not verify if `ContextKind` is already present
    #[inline(never)]
    #[cfg(feature = "error-context")]
    pub(crate) fn extend_context_unchecked<const N: usize>(
        mut self,
        context: [(ContextKind, ContextValue); N],
    ) -> Self {
        self.inner.context.extend_unchecked(context);
        self
    }

    pub(crate) fn display_help(cmd: &Command, styled: StyledStr) -> Self {
        Self::for_app(ErrorKind::DisplayHelp, cmd, styled)
    }

    pub(crate) fn display_help_error(cmd: &Command, styled: StyledStr) -> Self {
        Self::for_app(
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
            cmd,
            styled,
        )
    }

    pub(crate) fn display_version(cmd: &Command, styled: StyledStr) -> Self {
        Self::for_app(ErrorKind::DisplayVersion, cmd, styled)
    }

    pub(crate) fn argument_conflict(
        cmd: &Command,
        arg: String,
        mut others: Vec<String>,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::ArgumentConflict).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            let others = match others.len() {
                0 => ContextValue::None,
                1 => ContextValue::String(others.pop().unwrap()),
                _ => ContextValue::Strings(others),
            };
            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (ContextKind::PriorArg, others),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn subcommand_conflict(
        cmd: &Command,
        sub: String,
        mut others: Vec<String>,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::ArgumentConflict).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            let others = match others.len() {
                0 => ContextValue::None,
                1 => ContextValue::String(others.pop().unwrap()),
                _ => ContextValue::Strings(others),
            };
            err = err.extend_context_unchecked([
                (ContextKind::InvalidSubcommand, ContextValue::String(sub)),
                (ContextKind::PriorArg, others),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn empty_value(cmd: &Command, good_vals: &[String], arg: String) -> Self {
        Self::invalid_value(cmd, "".to_owned(), good_vals, arg)
    }

    pub(crate) fn no_equals(cmd: &Command, arg: String, usage: Option<StyledStr>) -> Self {
        let mut err = Self::new(ErrorKind::NoEquals).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err
                .extend_context_unchecked([(ContextKind::InvalidArg, ContextValue::String(arg))]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn invalid_value(
        cmd: &Command,
        bad_val: String,
        good_vals: &[String],
        arg: String,
    ) -> Self {
        let suggestion = suggestions::did_you_mean(&bad_val, good_vals.iter()).pop();
        let mut err = Self::new(ErrorKind::InvalidValue).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (ContextKind::InvalidValue, ContextValue::String(bad_val)),
                (
                    ContextKind::ValidValue,
                    ContextValue::Strings(good_vals.iter().map(|s| (*s).clone()).collect()),
                ),
            ]);
            if let Some(suggestion) = suggestion {
                err = err.insert_context_unchecked(
                    ContextKind::SuggestedValue,
                    ContextValue::String(suggestion),
                );
            }
        }

        err
    }

    pub(crate) fn invalid_subcommand(
        cmd: &Command,
        subcmd: String,
        did_you_mean: Vec<String>,
        name: String,
        suggested_trailing_arg: bool,
        usage: Option<StyledStr>,
    ) -> Self {
        use std::fmt::Write as _;
        let styles = cmd.get_styles();
        let invalid = &styles.get_invalid();
        let valid = &styles.get_valid();
        let mut err = Self::new(ErrorKind::InvalidSubcommand).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            let mut suggestions = vec![];
            if suggested_trailing_arg {
                let mut styled_suggestion = StyledStr::new();
                let _ = write!(
                    styled_suggestion,
                    "to pass '{invalid}{subcmd}{invalid:#}' as a value, use '{valid}{name} -- {subcmd}{valid:#}'",
                );
                suggestions.push(styled_suggestion);
            }

            err = err.extend_context_unchecked([
                (ContextKind::InvalidSubcommand, ContextValue::String(subcmd)),
                (
                    ContextKind::SuggestedSubcommand,
                    ContextValue::Strings(did_you_mean),
                ),
                (
                    ContextKind::Suggested,
                    ContextValue::StyledStrs(suggestions),
                ),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn unrecognized_subcommand(
        cmd: &Command,
        subcmd: String,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::InvalidSubcommand).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([(
                ContextKind::InvalidSubcommand,
                ContextValue::String(subcmd),
            )]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn missing_required_argument(
        cmd: &Command,
        required: Vec<String>,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::MissingRequiredArgument).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([(
                ContextKind::InvalidArg,
                ContextValue::Strings(required),
            )]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn missing_subcommand(
        cmd: &Command,
        parent: String,
        available: Vec<String>,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::MissingSubcommand).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([
                (ContextKind::InvalidSubcommand, ContextValue::String(parent)),
                (
                    ContextKind::ValidSubcommand,
                    ContextValue::Strings(available),
                ),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn invalid_utf8(cmd: &Command, usage: Option<StyledStr>) -> Self {
        let mut err = Self::new(ErrorKind::InvalidUtf8).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn too_many_values(
        cmd: &Command,
        val: String,
        arg: String,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::TooManyValues).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (ContextKind::InvalidValue, ContextValue::String(val)),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn too_few_values(
        cmd: &Command,
        arg: String,
        min_vals: usize,
        curr_vals: usize,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::TooFewValues).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (
                    ContextKind::MinValues,
                    ContextValue::Number(min_vals as isize),
                ),
                (
                    ContextKind::ActualNumValues,
                    ContextValue::Number(curr_vals as isize),
                ),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn value_validation(
        arg: String,
        val: String,
        err: Box<dyn error::Error + Send + Sync>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::ValueValidation).set_source(err);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (ContextKind::InvalidValue, ContextValue::String(val)),
            ]);
        }

        err
    }

    pub(crate) fn wrong_number_of_values(
        cmd: &Command,
        arg: String,
        num_vals: usize,
        curr_vals: usize,
        usage: Option<StyledStr>,
    ) -> Self {
        let mut err = Self::new(ErrorKind::WrongNumberOfValues).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (
                    ContextKind::ExpectedNumValues,
                    ContextValue::Number(num_vals as isize),
                ),
                (
                    ContextKind::ActualNumValues,
                    ContextValue::Number(curr_vals as isize),
                ),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    pub(crate) fn unknown_argument(
        cmd: &Command,
        arg: String,
        did_you_mean: Option<(String, Option<String>)>,
        suggested_trailing_arg: bool,
        usage: Option<StyledStr>,
    ) -> Self {
        use std::fmt::Write as _;
        let styles = cmd.get_styles();
        let invalid = &styles.get_invalid();
        let valid = &styles.get_valid();
        let mut err = Self::new(ErrorKind::UnknownArgument).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            let mut suggestions = vec![];
            if suggested_trailing_arg {
                let mut styled_suggestion = StyledStr::new();
                let _ = write!(
                    styled_suggestion,
                    "to pass '{invalid}{arg}{invalid:#}' as a value, use '{valid}-- {arg}{valid:#}'",
                );
                suggestions.push(styled_suggestion);
            }

            err = err
                .extend_context_unchecked([(ContextKind::InvalidArg, ContextValue::String(arg))]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
            match did_you_mean {
                Some((flag, Some(sub))) => {
                    let mut styled_suggestion = StyledStr::new();
                    let _ = write!(styled_suggestion, "'{valid}{sub} {flag}{valid:#}' exists",);
                    suggestions.push(styled_suggestion);
                }
                Some((flag, None)) => {
                    err = err.insert_context_unchecked(
                        ContextKind::SuggestedArg,
                        ContextValue::String(flag),
                    );
                }
                None => {}
            }
            if !suggestions.is_empty() {
                err = err.insert_context_unchecked(
                    ContextKind::Suggested,
                    ContextValue::StyledStrs(suggestions),
                );
            }
        }

        err
    }

    pub(crate) fn unnecessary_double_dash(
        cmd: &Command,
        arg: String,
        usage: Option<StyledStr>,
    ) -> Self {
        use std::fmt::Write as _;
        let styles = cmd.get_styles();
        let invalid = &styles.get_invalid();
        let valid = &styles.get_valid();
        let mut err = Self::new(ErrorKind::UnknownArgument).with_cmd(cmd);

        #[cfg(feature = "error-context")]
        {
            let mut styled_suggestion = StyledStr::new();
            let _ = write!(
                styled_suggestion,
                "subcommand '{valid}{arg}{valid:#}' exists; to use it, remove the '{invalid}--{invalid:#}' before it",
            );

            err = err.extend_context_unchecked([
                (ContextKind::InvalidArg, ContextValue::String(arg)),
                (
                    ContextKind::Suggested,
                    ContextValue::StyledStrs(vec![styled_suggestion]),
                ),
            ]);
            if let Some(usage) = usage {
                err = err
                    .insert_context_unchecked(ContextKind::Usage, ContextValue::StyledStr(usage));
            }
        }

        err
    }

    fn formatted(&self) -> Cow<'_, StyledStr> {
        if let Some(message) = self.inner.message.as_ref() {
            message.formatted(&self.inner.styles)
        } else {
            let styled = F::format_error(self);
            Cow::Owned(styled)
        }
    }
}

impl<F: ErrorFormatter> From<io::Error> for Error<F> {
    fn from(e: io::Error) -> Self {
        Error::raw(ErrorKind::Io, e)
    }
}

impl<F: ErrorFormatter> From<fmt::Error> for Error<F> {
    fn from(e: fmt::Error) -> Self {
        Error::raw(ErrorKind::Format, e)
    }
}

impl<F: ErrorFormatter> Debug for Error<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        self.inner.fmt(f)
    }
}

impl<F: ErrorFormatter> error::Error for Error<F> {
    #[allow(trivial_casts)]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.inner.source.as_ref().map(|e| e.as_ref() as _)
    }
}

impl<F: ErrorFormatter> Display for Error<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Assuming `self.message` already has a trailing newline, from `try_help` or similar
        ok!(write!(f, "{}", self.formatted()));
        if let Some(backtrace) = self.inner.backtrace.as_ref() {
            ok!(writeln!(f));
            ok!(writeln!(f, "Backtrace:"));
            ok!(writeln!(f, "{backtrace}"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Raw(String),
    Formatted(StyledStr),
}

impl Message {
    fn format(&mut self, cmd: &Command, usage: Option<StyledStr>) {
        match self {
            Message::Raw(s) => {
                let mut message = String::new();
                std::mem::swap(s, &mut message);

                let styled = format::format_error_message(
                    &message,
                    cmd.get_styles(),
                    Some(cmd),
                    usage.as_ref(),
                );

                *self = Self::Formatted(styled);
            }
            Message::Formatted(_) => {}
        }
    }

    fn formatted(&self, styles: &Styles) -> Cow<'_, StyledStr> {
        match self {
            Message::Raw(s) => {
                let styled = format::format_error_message(s, styles, None, None);

                Cow::Owned(styled)
            }
            Message::Formatted(s) => Cow::Borrowed(s),
        }
    }
}

impl From<String> for Message {
    fn from(inner: String) -> Self {
        Self::Raw(inner)
    }
}

impl From<StyledStr> for Message {
    fn from(inner: StyledStr) -> Self {
        Self::Formatted(inner)
    }
}

#[cfg(feature = "debug")]
#[derive(Debug)]
struct Backtrace(backtrace::Backtrace);

#[cfg(feature = "debug")]
impl Backtrace {
    fn new() -> Option<Self> {
        Some(Self(backtrace::Backtrace::new()))
    }
}

#[cfg(feature = "debug")]
impl Display for Backtrace {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // `backtrace::Backtrace` uses `Debug` instead of `Display`
        write!(f, "{:?}", self.0)
    }
}

#[cfg(not(feature = "debug"))]
#[derive(Debug)]
struct Backtrace;

#[cfg(not(feature = "debug"))]
impl Backtrace {
    fn new() -> Option<Self> {
        None
    }
}

#[cfg(not(feature = "debug"))]
impl Display for Backtrace {
    fn fmt(&self, _: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[test]
fn check_auto_traits() {
    static_assertions::assert_impl_all!(Error: Send, Sync, Unpin);
}
