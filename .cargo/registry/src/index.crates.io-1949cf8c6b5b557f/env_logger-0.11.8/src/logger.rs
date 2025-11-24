use std::{borrow::Cow, cell::RefCell, env, io};

use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

use crate::fmt;
use crate::fmt::writer::{self, Writer};
use crate::fmt::{FormatFn, Formatter};

/// The default name for the environment variable to read filters from.
pub const DEFAULT_FILTER_ENV: &str = "RUST_LOG";

/// The default name for the environment variable to read style preferences from.
pub const DEFAULT_WRITE_STYLE_ENV: &str = "RUST_LOG_STYLE";

/// `Builder` acts as builder for initializing a `Logger`.
///
/// It can be used to customize the log format, change the environment variable used
/// to provide the logging directives and also set the default log level filter.
///
/// # Examples
///
/// ```
/// # use std::io::Write;
/// use env_logger::Builder;
/// use log::{LevelFilter, error, info};
///
/// let mut builder = Builder::from_default_env();
///
/// builder
///     .format(|buf, record| writeln!(buf, "{} - {}", record.level(), record.args()))
///     .filter(None, LevelFilter::Info)
///     .init();
///
/// error!("error message");
/// info!("info message");
/// ```
#[derive(Default)]
pub struct Builder {
    filter: env_filter::Builder,
    writer: writer::Builder,
    format: fmt::Builder,
    built: bool,
}

impl Builder {
    /// Initializes the log builder with defaults.
    ///
    /// **NOTE:** This method won't read from any environment variables.
    /// Use the [`filter`] and [`write_style`] methods to configure the builder
    /// or use [`from_env`] or [`from_default_env`] instead.
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// use log::LevelFilter;
    /// use env_logger::{Builder, WriteStyle};
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder
    ///     .filter(None, LevelFilter::Info)
    ///     .write_style(WriteStyle::Always)
    ///     .init();
    /// ```
    ///
    /// [`filter`]: #method.filter
    /// [`write_style`]: #method.write_style
    /// [`from_env`]: #method.from_env
    /// [`from_default_env`]: #method.from_default_env
    pub fn new() -> Builder {
        Default::default()
    }

    /// Initializes the log builder from the environment.
    ///
    /// The variables used to read configuration from can be tweaked before
    /// passing in.
    ///
    /// # Examples
    ///
    /// Initialise a logger reading the log filter from an environment variable
    /// called `MY_LOG`:
    ///
    /// ```
    /// use env_logger::Builder;
    ///
    /// let mut builder = Builder::from_env("MY_LOG");
    /// builder.init();
    /// ```
    ///
    /// Initialise a logger using the `MY_LOG` variable for filtering and
    /// `MY_LOG_STYLE` for whether or not to write styles:
    ///
    /// ```
    /// use env_logger::{Builder, Env};
    ///
    /// let env = Env::new().filter("MY_LOG").write_style("MY_LOG_STYLE");
    ///
    /// let mut builder = Builder::from_env(env);
    /// builder.init();
    /// ```
    pub fn from_env<'a, E>(env: E) -> Self
    where
        E: Into<Env<'a>>,
    {
        let mut builder = Builder::new();
        builder.parse_env(env);
        builder
    }

    /// Applies the configuration from the environment.
    ///
    /// This function allows a builder to be configured with default parameters,
    /// to be then overridden by the environment.
    ///
    /// # Examples
    ///
    /// Initialise a logger with filter level `Off`, then override the log
    /// filter from an environment variable called `MY_LOG`:
    ///
    /// ```
    /// use log::LevelFilter;
    /// use env_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter_level(LevelFilter::Off);
    /// builder.parse_env("MY_LOG");
    /// builder.init();
    /// ```
    ///
    /// Initialise a logger with filter level `Off`, then use the `MY_LOG`
    /// variable to override filtering and `MY_LOG_STYLE` to override  whether
    /// or not to write styles:
    ///
    /// ```
    /// use log::LevelFilter;
    /// use env_logger::{Builder, Env};
    ///
    /// let env = Env::new().filter("MY_LOG").write_style("MY_LOG_STYLE");
    ///
    /// let mut builder = Builder::new();
    /// builder.filter_level(LevelFilter::Off);
    /// builder.parse_env(env);
    /// builder.init();
    /// ```
    pub fn parse_env<'a, E>(&mut self, env: E) -> &mut Self
    where
        E: Into<Env<'a>>,
    {
        let env = env.into();

        if let Some(s) = env.get_filter() {
            self.parse_filters(&s);
        }

        if let Some(s) = env.get_write_style() {
            self.parse_write_style(&s);
        }

        self
    }

    /// Initializes the log builder from the environment using default variable names.
    ///
    /// This method is a convenient way to call `from_env(Env::default())` without
    /// having to use the `Env` type explicitly. The builder will use the
    /// [default environment variables].
    ///
    /// # Examples
    ///
    /// Initialise a logger using the default environment variables:
    ///
    /// ```
    /// use env_logger::Builder;
    ///
    /// let mut builder = Builder::from_default_env();
    /// builder.init();
    /// ```
    ///
    /// [default environment variables]: struct.Env.html#default-environment-variables
    pub fn from_default_env() -> Self {
        Self::from_env(Env::default())
    }

    /// Applies the configuration from the environment using default variable names.
    ///
    /// This method is a convenient way to call `parse_env(Env::default())` without
    /// having to use the `Env` type explicitly. The builder will use the
    /// [default environment variables].
    ///
    /// # Examples
    ///
    /// Initialise a logger with filter level `Off`, then configure it using the
    /// default environment variables:
    ///
    /// ```
    /// use log::LevelFilter;
    /// use env_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.filter_level(LevelFilter::Off);
    /// builder.parse_default_env();
    /// builder.init();
    /// ```
    ///
    /// [default environment variables]: struct.Env.html#default-environment-variables
    pub fn parse_default_env(&mut self) -> &mut Self {
        self.parse_env(Env::default())
    }

    /// Sets the format function for formatting the log output.
    ///
    /// This function is called on each record logged and should format the
    /// log record and output it to the given [`Formatter`].
    ///
    /// The format function is expected to output the string directly to the
    /// `Formatter` so that implementations can use the [`std::fmt`] macros
    /// to format and output without intermediate heap allocations. The default
    /// `env_logger` formatter takes advantage of this.
    ///
    /// When the `color` feature is enabled, styling via ANSI escape codes is supported and the
    /// output will automatically respect [`Builder::write_style`].
    ///
    /// # Examples
    ///
    /// Use a custom format to write only the log message:
    ///
    /// ```
    /// use std::io::Write;
    /// use env_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.format(|buf, record| writeln!(buf, "{}", record.args()));
    /// ```
    ///
    /// [`Formatter`]: fmt/struct.Formatter.html
    /// [`String`]: https://doc.rust-lang.org/stable/std/string/struct.String.html
    /// [`std::fmt`]: https://doc.rust-lang.org/std/fmt/index.html
    pub fn format<F>(&mut self, format: F) -> &mut Self
    where
        F: Fn(&mut Formatter, &Record<'_>) -> io::Result<()> + Sync + Send + 'static,
    {
        self.format.custom_format = Some(Box::new(format));
        self
    }

    /// Use the default format.
    ///
    /// This method will clear any custom format set on the builder.
    pub fn default_format(&mut self) -> &mut Self {
        self.format = Default::default();
        self
    }

    /// Whether or not to write the level in the default format.
    pub fn format_level(&mut self, write: bool) -> &mut Self {
        self.format.default_format.level(write);
        self
    }

    /// Whether or not to write the source file path in the default format.
    pub fn format_file(&mut self, write: bool) -> &mut Self {
        self.format.default_format.file(write);
        self
    }

    /// Whether or not to write the source line number path in the default format.
    ///
    /// Only has effect if `format_file` is also enabled
    pub fn format_line_number(&mut self, write: bool) -> &mut Self {
        self.format.default_format.line_number(write);
        self
    }

    /// Whether or not to write the source path and line number
    ///
    /// Equivalent to calling both `format_file` and `format_line_number`
    /// with `true`
    pub fn format_source_path(&mut self, write: bool) -> &mut Self {
        self.format_file(write).format_line_number(write);
        self
    }

    /// Whether or not to write the module path in the default format.
    pub fn format_module_path(&mut self, write: bool) -> &mut Self {
        self.format.default_format.module_path(write);
        self
    }

    /// Whether or not to write the target in the default format.
    pub fn format_target(&mut self, write: bool) -> &mut Self {
        self.format.default_format.target(write);
        self
    }

    /// Configures the amount of spaces to use to indent multiline log records.
    /// A value of `None` disables any kind of indentation.
    pub fn format_indent(&mut self, indent: Option<usize>) -> &mut Self {
        self.format.default_format.indent(indent);
        self
    }

    /// Configures if timestamp should be included and in what precision.
    pub fn format_timestamp(&mut self, timestamp: Option<fmt::TimestampPrecision>) -> &mut Self {
        self.format.default_format.timestamp(timestamp);
        self
    }

    /// Configures the timestamp to use second precision.
    pub fn format_timestamp_secs(&mut self) -> &mut Self {
        self.format_timestamp(Some(fmt::TimestampPrecision::Seconds))
    }

    /// Configures the timestamp to use millisecond precision.
    pub fn format_timestamp_millis(&mut self) -> &mut Self {
        self.format_timestamp(Some(fmt::TimestampPrecision::Millis))
    }

    /// Configures the timestamp to use microsecond precision.
    pub fn format_timestamp_micros(&mut self) -> &mut Self {
        self.format_timestamp(Some(fmt::TimestampPrecision::Micros))
    }

    /// Configures the timestamp to use nanosecond precision.
    pub fn format_timestamp_nanos(&mut self) -> &mut Self {
        self.format_timestamp(Some(fmt::TimestampPrecision::Nanos))
    }

    /// Configures the end of line suffix.
    pub fn format_suffix(&mut self, suffix: &'static str) -> &mut Self {
        self.format.default_format.suffix(suffix);
        self
    }

    /// Set the format for structured key/value pairs in the log record
    ///
    /// With the default format, this function is called for each record and should format
    /// the structured key-value pairs as returned by [`log::Record::key_values`].
    ///
    /// The format function is expected to output the string directly to the `Formatter` so that
    /// implementations can use the [`std::fmt`] macros, similar to the main format function.
    ///
    /// The default format uses a space to separate each key-value pair, with an "=" between
    /// the key and value.
    #[cfg(feature = "kv")]
    pub fn format_key_values<F>(&mut self, format: F) -> &mut Self
    where
        F: Fn(&mut Formatter, &dyn log::kv::Source) -> io::Result<()> + Sync + Send + 'static,
    {
        self.format.default_format.key_values(format);
        self
    }

    /// Adds a directive to the filter for a specific module.
    ///
    /// # Examples
    ///
    /// Only include messages for info and above for logs in `path::to::module`:
    ///
    /// ```
    /// use env_logger::Builder;
    /// use log::LevelFilter;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter_module("path::to::module", LevelFilter::Info);
    /// ```
    pub fn filter_module(&mut self, module: &str, level: LevelFilter) -> &mut Self {
        self.filter.filter_module(module, level);
        self
    }

    /// Adds a directive to the filter for all modules.
    ///
    /// # Examples
    ///
    /// Only include messages for info and above for logs globally:
    ///
    /// ```
    /// use env_logger::Builder;
    /// use log::LevelFilter;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter_level(LevelFilter::Info);
    /// ```
    pub fn filter_level(&mut self, level: LevelFilter) -> &mut Self {
        self.filter.filter_level(level);
        self
    }

    /// Adds filters to the logger.
    ///
    /// The given module (if any) will log at most the specified level provided.
    /// If no module is provided then the filter will apply to all log messages.
    ///
    /// # Examples
    ///
    /// Only include messages for info and above for logs in `path::to::module`:
    ///
    /// ```
    /// use env_logger::Builder;
    /// use log::LevelFilter;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter(Some("path::to::module"), LevelFilter::Info);
    /// ```
    pub fn filter(&mut self, module: Option<&str>, level: LevelFilter) -> &mut Self {
        self.filter.filter(module, level);
        self
    }

    /// Parses the directives string in the same form as the `RUST_LOG`
    /// environment variable.
    ///
    /// See the module documentation for more details.
    pub fn parse_filters(&mut self, filters: &str) -> &mut Self {
        self.filter.parse(filters);
        self
    }

    /// Sets the target for the log output.
    ///
    /// Env logger can log to either stdout, stderr or a custom pipe. The default is stderr.
    ///
    /// The custom pipe can be used to send the log messages to a custom sink (for example a file).
    /// Do note that direct writes to a file can become a bottleneck due to IO operation times.
    ///
    /// # Examples
    ///
    /// Write log message to `stdout`:
    ///
    /// ```
    /// use env_logger::{Builder, Target};
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.target(Target::Stdout);
    /// ```
    pub fn target(&mut self, target: fmt::Target) -> &mut Self {
        self.writer.target(target);
        self
    }

    /// Sets whether or not styles will be written.
    ///
    /// This can be useful in environments that don't support control characters
    /// for setting colors.
    ///
    /// # Examples
    ///
    /// Never attempt to write styles:
    ///
    /// ```
    /// use env_logger::{Builder, WriteStyle};
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.write_style(WriteStyle::Never);
    /// ```
    pub fn write_style(&mut self, write_style: fmt::WriteStyle) -> &mut Self {
        self.writer.write_style(write_style);
        self
    }

    /// Parses whether or not to write styles in the same form as the `RUST_LOG_STYLE`
    /// environment variable.
    ///
    /// See the module documentation for more details.
    pub fn parse_write_style(&mut self, write_style: &str) -> &mut Self {
        self.writer.parse_write_style(write_style);
        self
    }

    /// Sets whether or not the logger will be used in unit tests.
    ///
    /// If `is_test` is `true` then the logger will allow the testing framework to
    /// capture log records rather than printing them to the terminal directly.
    pub fn is_test(&mut self, is_test: bool) -> &mut Self {
        self.writer.is_test(is_test);
        self
    }

    /// Initializes the global logger with the built env logger.
    ///
    /// This should be called early in the execution of a Rust program. Any log
    /// events that occur before initialization will be ignored.
    ///
    /// # Errors
    ///
    /// This function will fail if it is called more than once, or if another
    /// library has already initialized a global logger.
    pub fn try_init(&mut self) -> Result<(), SetLoggerError> {
        let logger = self.build();

        let max_level = logger.filter();
        let r = log::set_boxed_logger(Box::new(logger));

        if r.is_ok() {
            log::set_max_level(max_level);
        }

        r
    }

    /// Initializes the global logger with the built env logger.
    ///
    /// This should be called early in the execution of a Rust program. Any log
    /// events that occur before initialization will be ignored.
    ///
    /// # Panics
    ///
    /// This function will panic if it is called more than once, or if another
    /// library has already initialized a global logger.
    pub fn init(&mut self) {
        self.try_init()
            .expect("Builder::init should not be called after logger initialized");
    }

    /// Build an env logger.
    ///
    /// The returned logger implements the `Log` trait and can be installed manually
    /// or nested within another logger.
    pub fn build(&mut self) -> Logger {
        assert!(!self.built, "attempt to re-use consumed builder");
        self.built = true;

        Logger {
            writer: self.writer.build(),
            filter: self.filter.build(),
            format: self.format.build(),
        }
    }
}

impl std::fmt::Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.built {
            f.debug_struct("Logger").field("built", &true).finish()
        } else {
            f.debug_struct("Logger")
                .field("filter", &self.filter)
                .field("writer", &self.writer)
                .finish()
        }
    }
}

/// The env logger.
///
/// This struct implements the `Log` trait from the [`log` crate][log-crate-url],
/// which allows it to act as a logger.
///
/// The [`init()`], [`try_init()`], [`Builder::init()`] and [`Builder::try_init()`]
/// methods will each construct a `Logger` and immediately initialize it as the
/// default global logger.
///
/// If you'd instead need access to the constructed `Logger`, you can use
/// the associated [`Builder`] and install it with the
/// [`log` crate][log-crate-url] directly.
///
/// [log-crate-url]: https://docs.rs/log
/// [`init()`]: fn.init.html
/// [`try_init()`]: fn.try_init.html
/// [`Builder::init()`]: struct.Builder.html#method.init
/// [`Builder::try_init()`]: struct.Builder.html#method.try_init
/// [`Builder`]: struct.Builder.html
pub struct Logger {
    writer: Writer,
    filter: env_filter::Filter,
    format: FormatFn,
}

impl Logger {
    /// Creates the logger from the environment.
    ///
    /// The variables used to read configuration from can be tweaked before
    /// passing in.
    ///
    /// # Examples
    ///
    /// Create a logger reading the log filter from an environment variable
    /// called `MY_LOG`:
    ///
    /// ```
    /// use env_logger::Logger;
    ///
    /// let logger = Logger::from_env("MY_LOG");
    /// ```
    ///
    /// Create a logger using the `MY_LOG` variable for filtering and
    /// `MY_LOG_STYLE` for whether or not to write styles:
    ///
    /// ```
    /// use env_logger::{Logger, Env};
    ///
    /// let env = Env::new().filter_or("MY_LOG", "info").write_style_or("MY_LOG_STYLE", "always");
    ///
    /// let logger = Logger::from_env(env);
    /// ```
    pub fn from_env<'a, E>(env: E) -> Self
    where
        E: Into<Env<'a>>,
    {
        Builder::from_env(env).build()
    }

    /// Creates the logger from the environment using default variable names.
    ///
    /// This method is a convenient way to call `from_env(Env::default())` without
    /// having to use the `Env` type explicitly. The logger will use the
    /// [default environment variables].
    ///
    /// # Examples
    ///
    /// Creates a logger using the default environment variables:
    ///
    /// ```
    /// use env_logger::Logger;
    ///
    /// let logger = Logger::from_default_env();
    /// ```
    ///
    /// [default environment variables]: struct.Env.html#default-environment-variables
    pub fn from_default_env() -> Self {
        Builder::from_default_env().build()
    }

    /// Returns the maximum `LevelFilter` that this env logger instance is
    /// configured to output.
    pub fn filter(&self) -> LevelFilter {
        self.filter.filter()
    }

    /// Checks if this record matches the configured filter.
    pub fn matches(&self, record: &Record<'_>) -> bool {
        self.filter.matches(record)
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if self.matches(record) {
            // Log records are written to a thread-local buffer before being printed
            // to the terminal. We clear these buffers afterwards, but they aren't shrunk
            // so will always at least have capacity for the largest log record formatted
            // on that thread.
            //
            // If multiple `Logger`s are used by the same threads then the thread-local
            // formatter might have different color support. If this is the case the
            // formatter and its buffer are discarded and recreated.

            thread_local! {
                static FORMATTER: RefCell<Option<Formatter>> = const { RefCell::new(None) };
            }

            let print = |formatter: &mut Formatter, record: &Record<'_>| {
                let _ = self
                    .format
                    .format(formatter, record)
                    .and_then(|_| formatter.print(&self.writer));

                // Always clear the buffer afterwards
                formatter.clear();
            };

            let printed = FORMATTER
                .try_with(|tl_buf| {
                    if let Ok(mut tl_buf) = tl_buf.try_borrow_mut() {
                        // There are no active borrows of the buffer
                        if let Some(ref mut formatter) = *tl_buf {
                            // We have a previously set formatter

                            // Check the buffer style. If it's different from the logger's
                            // style then drop the buffer and recreate it.
                            if formatter.write_style() != self.writer.write_style() {
                                *formatter = Formatter::new(&self.writer);
                            }

                            print(formatter, record);
                        } else {
                            // We don't have a previously set formatter
                            let mut formatter = Formatter::new(&self.writer);
                            print(&mut formatter, record);

                            *tl_buf = Some(formatter);
                        }
                    } else {
                        // There's already an active borrow of the buffer (due to re-entrancy)
                        print(&mut Formatter::new(&self.writer), record);
                    }
                })
                .is_ok();

            if !printed {
                // The thread-local storage was not available (because its
                // destructor has already run). Create a new single-use
                // Formatter on the stack for this call.
                print(&mut Formatter::new(&self.writer), record);
            }
        }
    }

    fn flush(&self) {}
}

impl std::fmt::Debug for Logger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logger")
            .field("filter", &self.filter)
            .finish()
    }
}

/// Set of environment variables to configure from.
///
/// # Default environment variables
///
/// By default, the `Env` will read the following environment variables:
///
/// - `RUST_LOG`: the level filter
/// - `RUST_LOG_STYLE`: whether or not to print styles with records.
///
/// These sources can be configured using the builder methods on `Env`.
#[derive(Debug)]
pub struct Env<'a> {
    filter: Var<'a>,
    write_style: Var<'a>,
}

impl<'a> Env<'a> {
    /// Get a default set of environment variables.
    pub fn new() -> Self {
        Self::default()
    }

    /// Specify an environment variable to read the filter from.
    pub fn filter<E>(mut self, filter_env: E) -> Self
    where
        E: Into<Cow<'a, str>>,
    {
        self.filter = Var::new(filter_env);

        self
    }

    /// Specify an environment variable to read the filter from.
    ///
    /// If the variable is not set, the default value will be used.
    pub fn filter_or<E, V>(mut self, filter_env: E, default: V) -> Self
    where
        E: Into<Cow<'a, str>>,
        V: Into<Cow<'a, str>>,
    {
        self.filter = Var::new_with_default(filter_env, default);

        self
    }

    /// Use the default environment variable to read the filter from.
    ///
    /// If the variable is not set, the default value will be used.
    pub fn default_filter_or<V>(mut self, default: V) -> Self
    where
        V: Into<Cow<'a, str>>,
    {
        self.filter = Var::new_with_default(DEFAULT_FILTER_ENV, default);

        self
    }

    fn get_filter(&self) -> Option<String> {
        self.filter.get()
    }

    /// Specify an environment variable to read the style from.
    pub fn write_style<E>(mut self, write_style_env: E) -> Self
    where
        E: Into<Cow<'a, str>>,
    {
        self.write_style = Var::new(write_style_env);

        self
    }

    /// Specify an environment variable to read the style from.
    ///
    /// If the variable is not set, the default value will be used.
    pub fn write_style_or<E, V>(mut self, write_style_env: E, default: V) -> Self
    where
        E: Into<Cow<'a, str>>,
        V: Into<Cow<'a, str>>,
    {
        self.write_style = Var::new_with_default(write_style_env, default);

        self
    }

    /// Use the default environment variable to read the style from.
    ///
    /// If the variable is not set, the default value will be used.
    pub fn default_write_style_or<V>(mut self, default: V) -> Self
    where
        V: Into<Cow<'a, str>>,
    {
        self.write_style = Var::new_with_default(DEFAULT_WRITE_STYLE_ENV, default);

        self
    }

    fn get_write_style(&self) -> Option<String> {
        self.write_style.get()
    }
}

impl<'a, T> From<T> for Env<'a>
where
    T: Into<Cow<'a, str>>,
{
    fn from(filter_env: T) -> Self {
        Env::default().filter(filter_env.into())
    }
}

impl Default for Env<'_> {
    fn default() -> Self {
        Env {
            filter: Var::new(DEFAULT_FILTER_ENV),
            write_style: Var::new(DEFAULT_WRITE_STYLE_ENV),
        }
    }
}

#[derive(Debug)]
struct Var<'a> {
    name: Cow<'a, str>,
    default: Option<Cow<'a, str>>,
}

impl<'a> Var<'a> {
    fn new<E>(name: E) -> Self
    where
        E: Into<Cow<'a, str>>,
    {
        Var {
            name: name.into(),
            default: None,
        }
    }

    fn new_with_default<E, V>(name: E, default: V) -> Self
    where
        E: Into<Cow<'a, str>>,
        V: Into<Cow<'a, str>>,
    {
        Var {
            name: name.into(),
            default: Some(default.into()),
        }
    }

    fn get(&self) -> Option<String> {
        env::var(&*self.name)
            .ok()
            .or_else(|| self.default.clone().map(|v| v.into_owned()))
    }
}

/// Attempts to initialize the global logger with an env logger.
///
/// This should be called early in the execution of a Rust program. Any log
/// events that occur before initialization will be ignored.
///
/// # Errors
///
/// This function will fail if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn try_init() -> Result<(), SetLoggerError> {
    try_init_from_env(Env::default())
}

/// Initializes the global logger with an env logger.
///
/// This should be called early in the execution of a Rust program. Any log
/// events that occur before initialization will be ignored.
///
/// # Panics
///
/// This function will panic if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn init() {
    try_init().expect("env_logger::init should not be called after logger initialized");
}

/// Attempts to initialize the global logger with an env logger from the given
/// environment variables.
///
/// This should be called early in the execution of a Rust program. Any log
/// events that occur before initialization will be ignored.
///
/// # Examples
///
/// Initialise a logger using the `MY_LOG` environment variable for filters
/// and `MY_LOG_STYLE` for writing colors:
///
/// ```
/// use env_logger::{Builder, Env};
///
/// # fn run() -> Result<(), Box<dyn ::std::error::Error>> {
/// let env = Env::new().filter("MY_LOG").write_style("MY_LOG_STYLE");
///
/// env_logger::try_init_from_env(env)?;
///
/// Ok(())
/// # }
/// # run().unwrap();
/// ```
///
/// # Errors
///
/// This function will fail if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn try_init_from_env<'a, E>(env: E) -> Result<(), SetLoggerError>
where
    E: Into<Env<'a>>,
{
    let mut builder = Builder::from_env(env);

    builder.try_init()
}

/// Initializes the global logger with an env logger from the given environment
/// variables.
///
/// This should be called early in the execution of a Rust program. Any log
/// events that occur before initialization will be ignored.
///
/// # Examples
///
/// Initialise a logger using the `MY_LOG` environment variable for filters
/// and `MY_LOG_STYLE` for writing colors:
///
/// ```
/// use env_logger::{Builder, Env};
///
/// let env = Env::new().filter("MY_LOG").write_style("MY_LOG_STYLE");
///
/// env_logger::init_from_env(env);
/// ```
///
/// # Panics
///
/// This function will panic if it is called more than once, or if another
/// library has already initialized a global logger.
pub fn init_from_env<'a, E>(env: E)
where
    E: Into<Env<'a>>,
{
    try_init_from_env(env)
        .expect("env_logger::init_from_env should not be called after logger initialized");
}

/// Create a new builder with the default environment variables.
///
/// The builder can be configured before being initialized.
/// This is a convenient way of calling [`Builder::from_default_env`].
///
/// [`Builder::from_default_env`]: struct.Builder.html#method.from_default_env
pub fn builder() -> Builder {
    Builder::from_default_env()
}

/// Create a builder from the given environment variables.
///
/// The builder can be configured before being initialized.
#[deprecated(
    since = "0.8.0",
    note = "Prefer `env_logger::Builder::from_env()` instead."
)]
pub fn from_env<'a, E>(env: E) -> Builder
where
    E: Into<Env<'a>>,
{
    Builder::from_env(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_get_filter_reads_from_var_if_set() {
        env::set_var("env_get_filter_reads_from_var_if_set", "from var");

        let env = Env::new().filter_or("env_get_filter_reads_from_var_if_set", "from default");

        assert_eq!(Some("from var".to_owned()), env.get_filter());
    }

    #[test]
    fn env_get_filter_reads_from_default_if_var_not_set() {
        env::remove_var("env_get_filter_reads_from_default_if_var_not_set");

        let env = Env::new().filter_or(
            "env_get_filter_reads_from_default_if_var_not_set",
            "from default",
        );

        assert_eq!(Some("from default".to_owned()), env.get_filter());
    }

    #[test]
    fn env_get_write_style_reads_from_var_if_set() {
        env::set_var("env_get_write_style_reads_from_var_if_set", "from var");

        let env =
            Env::new().write_style_or("env_get_write_style_reads_from_var_if_set", "from default");

        assert_eq!(Some("from var".to_owned()), env.get_write_style());
    }

    #[test]
    fn env_get_write_style_reads_from_default_if_var_not_set() {
        env::remove_var("env_get_write_style_reads_from_default_if_var_not_set");

        let env = Env::new().write_style_or(
            "env_get_write_style_reads_from_default_if_var_not_set",
            "from default",
        );

        assert_eq!(Some("from default".to_owned()), env.get_write_style());
    }

    #[test]
    fn builder_parse_env_overrides_existing_filters() {
        env::set_var(
            "builder_parse_default_env_overrides_existing_filters",
            "debug",
        );
        let env = Env::new().filter("builder_parse_default_env_overrides_existing_filters");

        let mut builder = Builder::new();
        builder.filter_level(LevelFilter::Trace);
        // Overrides global level to debug
        builder.parse_env(env);

        assert_eq!(builder.filter.build().filter(), LevelFilter::Debug);
    }
}
