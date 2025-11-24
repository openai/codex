//! An adapter for converting [`log`] records into `tracing` `Event`s.
//!
//! This module provides the [`LogTracer`] type which implements `log`'s [logger
//! interface] by recording log records as `tracing` `Event`s. This is intended for
//! use in conjunction with a `tracing` `Subscriber` to consume events from
//! dependencies that emit [`log`] records within a trace context.
//!
//! # Usage
//!
//! To create and initialize a `LogTracer` with the default configurations, use:
//!
//! * [`init`] if you want to convert all logs, regardless of log level,
//!   allowing the tracing `Subscriber` to perform any filtering
//! * [`init_with_filter`] to convert all logs up to a specified log level
//!
//! In addition, a [builder] is available for cases where more advanced
//! configuration is required. In particular, the builder can be used to [ignore
//! log records][ignore] emitted by particular crates. This is useful in cases
//! such as when a crate emits both `tracing` diagnostics _and_ log records by
//! default.
//!
//! [logger interface]: log::Log
//! [`init`]: LogTracer.html#method.init
//! [`init_with_filter`]: LogTracer.html#method.init_with_filter
//! [builder]: LogTracer::builder()
//! [ignore]: Builder::ignore_crate()
use crate::AsTrace;
pub use log::SetLoggerError;
use tracing_core::dispatcher;

/// A simple "logger" that converts all log records into `tracing` `Event`s.
#[derive(Debug)]
pub struct LogTracer {
    ignore_crates: Box<[String]>,
}

/// Configures a new `LogTracer`.
#[derive(Debug)]
pub struct Builder {
    ignore_crates: Vec<String>,
    filter: log::LevelFilter,
    #[cfg(all(feature = "interest-cache", feature = "std"))]
    interest_cache_config: Option<crate::InterestCacheConfig>,
}

// ===== impl LogTracer =====

impl LogTracer {
    /// Returns a builder that allows customizing a `LogTracer` and setting it
    /// the default logger.
    ///
    /// For example:
    /// ```rust
    /// # use std::error::Error;
    /// use tracing_log::LogTracer;
    /// use log;
    ///
    /// # fn main() -> Result<(), Box<Error>> {
    /// LogTracer::builder()
    ///     .ignore_crate("foo") // suppose the `foo` crate is using `tracing`'s log feature
    ///     .with_max_level(log::LevelFilter::Info)
    ///     .init()?;
    ///
    /// // will be available for Subscribers as a tracing Event
    /// log::info!("an example info log");
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Creates a new `LogTracer` that can then be used as a logger for the `log` crate.
    ///
    /// It is generally simpler to use the [`init`] or [`init_with_filter`] methods
    /// which will create the `LogTracer` and set it as the global logger.
    ///
    /// Logger setup without the initialization methods can be done with:
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use tracing_log::LogTracer;
    /// use log;
    ///
    /// # fn main() -> Result<(), Box<Error>> {
    /// let logger = LogTracer::new();
    /// log::set_boxed_logger(Box::new(logger))?;
    /// log::set_max_level(log::LevelFilter::Trace);
    ///
    /// // will be available for Subscribers as a tracing Event
    /// log::trace!("an example trace log");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`init`]: LogTracer::init()
    /// [`init_with_filter`]: .#method.init_with_filter
    pub fn new() -> Self {
        Self {
            ignore_crates: Vec::new().into_boxed_slice(),
        }
    }

    /// Sets up `LogTracer` as global logger for the `log` crate,
    /// with the given level as max level filter.
    ///
    /// Setting a global logger can only be done once.
    ///
    /// The [`builder`] function can be used to customize the `LogTracer` before
    /// initializing it.
    ///
    /// [`builder`]: LogTracer::builder()
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn init_with_filter(level: log::LevelFilter) -> Result<(), SetLoggerError> {
        Self::builder().with_max_level(level).init()
    }

    /// Sets a `LogTracer` as the global logger for the `log` crate.
    ///
    /// Setting a global logger can only be done once.
    ///
    /// ```rust
    /// # use std::error::Error;
    /// use tracing_log::LogTracer;
    /// use log;
    ///
    /// # fn main() -> Result<(), Box<Error>> {
    /// LogTracer::init()?;
    ///
    /// // will be available for Subscribers as a tracing Event
    /// log::trace!("an example trace log");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// This will forward all logs to `tracing` and lets the current `Subscriber`
    /// determine if they are enabled.
    ///
    /// The [`builder`] function can be used to customize the `LogTracer` before
    /// initializing it.
    ///
    /// If you know in advance you want to filter some log levels,
    /// use [`builder`] or [`init_with_filter`] instead.
    ///
    /// [`init_with_filter`]: LogTracer::init_with_filter()
    /// [`builder`]: LogTracer::builder()
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn init() -> Result<(), SetLoggerError> {
        Self::builder().init()
    }
}

impl Default for LogTracer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "interest-cache", feature = "std"))]
use crate::interest_cache::try_cache as try_cache_interest;

#[cfg(not(all(feature = "interest-cache", feature = "std")))]
fn try_cache_interest(_: &log::Metadata<'_>, callback: impl FnOnce() -> bool) -> bool {
    callback()
}

impl log::Log for LogTracer {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // First, check the log record against the current max level enabled by
        // the current `tracing` subscriber.
        if metadata.level().as_trace() > tracing_core::LevelFilter::current() {
            // If the log record's level is above that, disable it.
            return false;
        }

        // Okay, it wasn't disabled by the max level — do we have any specific
        // modules to ignore?
        if !self.ignore_crates.is_empty() {
            // If we are ignoring certain module paths, ensure that the metadata
            // does not start with one of those paths.
            let target = metadata.target();
            for ignored in &self.ignore_crates[..] {
                if target.starts_with(ignored) {
                    return false;
                }
            }
        }

        try_cache_interest(metadata, || {
            // Finally, check if the current `tracing` dispatcher cares about this.
            dispatcher::get_default(|dispatch| dispatch.enabled(&metadata.as_trace()))
        })
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            crate::dispatch_record(record);
        }
    }

    fn flush(&self) {}
}

// ===== impl Builder =====

impl Builder {
    /// Returns a new `Builder` to construct a [`LogTracer`].
    ///
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a global maximum level for `log` records.
    ///
    /// Log records whose level is more verbose than the provided level will be
    /// disabled.
    ///
    /// By default, all `log` records will be enabled.
    pub fn with_max_level(self, filter: impl Into<log::LevelFilter>) -> Self {
        let filter = filter.into();
        Self { filter, ..self }
    }

    /// Configures the `LogTracer` to ignore all log records whose target
    /// starts with the given string.
    ///
    /// This should be used when a crate enables the `tracing/log` feature to
    /// emit log records for tracing events. Otherwise, those events will be
    /// recorded twice.
    pub fn ignore_crate(mut self, name: impl Into<String>) -> Self {
        self.ignore_crates.push(name.into());
        self
    }

    /// Configures the `LogTracer` to ignore all log records whose target
    /// starts with any of the given the given strings.
    ///
    /// This should be used when a crate enables the `tracing/log` feature to
    /// emit log records for tracing events. Otherwise, those events will be
    /// recorded twice.
    pub fn ignore_all<I>(self, crates: impl IntoIterator<Item = I>) -> Self
    where
        I: Into<String>,
    {
        crates.into_iter().fold(self, Self::ignore_crate)
    }

    /// Configures the `LogTracer` to either disable or enable the interest cache.
    ///
    /// When enabled, a per-thread LRU cache will be used to cache whenever the logger
    /// is interested in a given [level] + [target] pair for records generated through
    /// the `log` crate.
    ///
    /// When no `trace!` logs are enabled the logger is able to cheaply filter
    /// them out just by comparing their log level to the globally specified
    /// maximum, and immediately reject them. When *any* other `trace!` log is
    /// enabled (even one which doesn't actually exist!) the logger has to run
    /// its full filtering machinery on each and every `trace!` log, which can
    /// potentially be very expensive.
    ///
    /// Enabling this cache is useful in such situations to improve performance.
    ///
    /// You most likely do not want to enabled this if you have registered any dynamic
    /// filters on your logger and you want them to be run every time.
    ///
    /// This is disabled by default.
    ///
    /// [level]: log::Metadata::level
    /// [target]: log::Metadata::target
    #[cfg(all(feature = "interest-cache", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "interest-cache", feature = "std"))))]
    pub fn with_interest_cache(mut self, config: crate::InterestCacheConfig) -> Self {
        self.interest_cache_config = Some(config);
        self
    }

    /// Constructs a new `LogTracer` with the provided configuration and sets it
    /// as the default logger.
    ///
    /// Setting a global logger can only be done once.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[allow(unused_mut)]
    pub fn init(mut self) -> Result<(), SetLoggerError> {
        #[cfg(all(feature = "interest-cache", feature = "std"))]
        crate::interest_cache::configure(self.interest_cache_config.take());

        let ignore_crates = self.ignore_crates.into_boxed_slice();
        let logger = Box::new(LogTracer { ignore_crates });
        log::set_boxed_logger(logger)?;
        log::set_max_level(self.filter);
        Ok(())
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            ignore_crates: Vec::new(),
            filter: log::LevelFilter::max(),
            #[cfg(all(feature = "interest-cache", feature = "std"))]
            interest_cache_config: None,
        }
    }
}
