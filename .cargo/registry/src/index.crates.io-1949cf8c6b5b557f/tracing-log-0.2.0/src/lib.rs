//! Adapters for connecting unstructured log records from the `log` crate into
//! the `tracing` ecosystem.
//!
//! # Overview
//!
//! [`tracing`] is a framework for instrumenting Rust programs with context-aware,
//! structured, event-based diagnostic information. This crate provides
//! compatibility layers for using `tracing` alongside the logging facade provided
//! by the [`log`] crate.
//!
//! This crate provides:
//!
//! - [`AsTrace`] and [`AsLog`] traits for converting between `tracing` and `log` types.
//! - [`LogTracer`], a [`log::Log`] implementation that consumes [`log::Record`]s
//!   and outputs them as [`tracing::Event`].
//!
//! *Compiler support: [requires `rustc` 1.56+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! # Usage
//!
//! ## Convert log records to tracing `Event`s
//!
//! To convert [`log::Record`]s as [`tracing::Event`]s, set `LogTracer` as the default
//! logger by calling its [`init`] or [`init_with_filter`] methods.
//!
//! ```rust
//! # use std::error::Error;
//! use tracing_log::LogTracer;
//! use log;
//!
//! # fn main() -> Result<(), Box<Error>> {
//! LogTracer::init()?;
//!
//! // will be available for Subscribers as a tracing Event
//! log::trace!("an example trace log");
//! # Ok(())
//! # }
//! ```
//!
//! This conversion does not convert unstructured data in log records (such as
//! values passed as format arguments to the `log!` macro) to structured
//! `tracing` fields. However, it *does* attach these new events to to the
//! span that was currently executing when the record was logged. This is the
//! primary use-case for this library: making it possible to locate the log
//! records emitted by dependencies which use `log` within the context of a
//! trace.
//!
//! ## Convert tracing `Event`s to logs
//!
//! Enabling the ["log" and "log-always" feature flags][flags] on the `tracing`
//! crate will cause all `tracing` spans and events to emit `log::Record`s as
//! they occur.
//!
//! ## Caution: Mixing both conversions
//!
//! Note that `log::Logger` implementations that convert log records to trace events
//! should not be used with `Subscriber`s that convert trace events _back_ into
//! `log` records, as doing so will result in the event recursing between the subscriber
//! and the logger forever (or, in real life, probably overflowing the call stack).
//!
//! If the logging of trace events generated from log records produced by the
//! `log` crate is desired, either the `log` crate should not be used to
//! implement this logging, or an additional layer of filtering will be
//! required to avoid infinitely converting between `Event` and `log::Record`.
//!
//! ## Feature Flags
//!
//! * `std`: enables features that require the Rust standard library (on by default)
//! * `log-tracer`: enables the `LogTracer` type (on by default)
//! * `interest-cache`: makes it possible to configure an interest cache for
//!   logs emitted through the `log` crate (see [`Builder::with_interest_cache`]); requires `std`
//!
//! ## Supported Rust Versions
//!
//! Tracing is built against the latest stable release. The minimum supported
//! version is 1.56. The current Tracing version is not guaranteed to build on
//! Rust versions earlier than the minimum supported version.
//!
//! Tracing follows the same compiler support policies as the rest of the Tokio
//! project. The current stable Rust compiler and the three most recent minor
//! versions before it will always be supported. For example, if the current
//! stable compiler version is 1.69, the minimum supported version will not be
//! increased past 1.66, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
//!
//! [`init`]: LogTracer::init
//! [`init_with_filter`]: LogTracer::init_with_filter
//! [`tracing`]: https://crates.io/crates/tracing
//! [`tracing::Subscriber`]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
//! [`Subscriber`]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
//! [`tracing::Event`]: https://docs.rs/tracing/latest/tracing/struct.Event.html
//! [flags]: https://docs.rs/tracing/latest/tracing/#crate-feature-flags
//! [`Builder::with_interest_cache`]: log_tracer::Builder::with_interest_cache
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/logo-type.png",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg), deny(rustdoc::broken_intra_doc_links))]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
use once_cell::sync::Lazy;

use std::{fmt, io};

use tracing_core::{
    callsite::{self, Callsite},
    dispatcher,
    field::{self, Field, Visit},
    identify_callsite,
    metadata::{Kind, Level},
    subscriber, Event, Metadata,
};

#[cfg(feature = "log-tracer")]
#[cfg_attr(docsrs, doc(cfg(feature = "log-tracer")))]
pub mod log_tracer;

#[cfg(feature = "log-tracer")]
#[cfg_attr(docsrs, doc(cfg(feature = "log-tracer")))]
#[doc(inline)]
pub use self::log_tracer::LogTracer;

pub use log;

#[cfg(all(feature = "interest-cache", feature = "log-tracer", feature = "std"))]
mod interest_cache;

#[cfg(all(feature = "interest-cache", feature = "log-tracer", feature = "std"))]
#[cfg_attr(
    docsrs,
    doc(cfg(all(feature = "interest-cache", feature = "log-tracer", feature = "std")))
)]
pub use crate::interest_cache::InterestCacheConfig;

/// Format a log record as a trace event in the current span.
pub fn format_trace(record: &log::Record<'_>) -> io::Result<()> {
    dispatch_record(record);
    Ok(())
}

// XXX(eliza): this is factored out so that we don't have to deal with the pub
// function `format_trace`'s `Result` return type...maybe we should get rid of
// that in 0.2...
pub(crate) fn dispatch_record(record: &log::Record<'_>) {
    dispatcher::get_default(|dispatch| {
        let filter_meta = record.as_trace();
        if !dispatch.enabled(&filter_meta) {
            return;
        }

        let (_, keys, meta) = loglevel_to_cs(record.level());

        let log_module = record.module_path();
        let log_file = record.file();
        let log_line = record.line();

        let module = log_module.as_ref().map(|s| s as &dyn field::Value);
        let file = log_file.as_ref().map(|s| s as &dyn field::Value);
        let line = log_line.as_ref().map(|s| s as &dyn field::Value);

        dispatch.event(&Event::new(
            meta,
            &meta.fields().value_set(&[
                (&keys.message, Some(record.args() as &dyn field::Value)),
                (&keys.target, Some(&record.target())),
                (&keys.module, module),
                (&keys.file, file),
                (&keys.line, line),
            ]),
        ));
    });
}

/// Trait implemented for `tracing` types that can be converted to a `log`
/// equivalent.
pub trait AsLog: crate::sealed::Sealed {
    /// The `log` type that this type can be converted into.
    type Log;
    /// Returns the `log` equivalent of `self`.
    fn as_log(&self) -> Self::Log;
}

/// Trait implemented for `log` types that can be converted to a `tracing`
/// equivalent.
pub trait AsTrace: crate::sealed::Sealed {
    /// The `tracing` type that this type can be converted into.
    type Trace;
    /// Returns the `tracing` equivalent of `self`.
    fn as_trace(&self) -> Self::Trace;
}

impl<'a> crate::sealed::Sealed for Metadata<'a> {}

impl<'a> AsLog for Metadata<'a> {
    type Log = log::Metadata<'a>;
    fn as_log(&self) -> Self::Log {
        log::Metadata::builder()
            .level(self.level().as_log())
            .target(self.target())
            .build()
    }
}
impl<'a> crate::sealed::Sealed for log::Metadata<'a> {}

impl<'a> AsTrace for log::Metadata<'a> {
    type Trace = Metadata<'a>;
    fn as_trace(&self) -> Self::Trace {
        let cs_id = identify_callsite!(loglevel_to_cs(self.level()).0);
        Metadata::new(
            "log record",
            self.target(),
            self.level().as_trace(),
            None,
            None,
            None,
            field::FieldSet::new(FIELD_NAMES, cs_id),
            Kind::EVENT,
        )
    }
}

struct Fields {
    message: field::Field,
    target: field::Field,
    module: field::Field,
    file: field::Field,
    line: field::Field,
}

static FIELD_NAMES: &[&str] = &[
    "message",
    "log.target",
    "log.module_path",
    "log.file",
    "log.line",
];

impl Fields {
    fn new(cs: &'static dyn Callsite) -> Self {
        let fieldset = cs.metadata().fields();
        let message = fieldset.field("message").unwrap();
        let target = fieldset.field("log.target").unwrap();
        let module = fieldset.field("log.module_path").unwrap();
        let file = fieldset.field("log.file").unwrap();
        let line = fieldset.field("log.line").unwrap();
        Fields {
            message,
            target,
            module,
            file,
            line,
        }
    }
}

macro_rules! log_cs {
    ($level:expr, $cs:ident, $meta:ident, $ty:ident) => {
        struct $ty;
        static $cs: $ty = $ty;
        static $meta: Metadata<'static> = Metadata::new(
            "log event",
            "log",
            $level,
            ::core::option::Option::None,
            ::core::option::Option::None,
            ::core::option::Option::None,
            field::FieldSet::new(FIELD_NAMES, identify_callsite!(&$cs)),
            Kind::EVENT,
        );

        impl callsite::Callsite for $ty {
            fn set_interest(&self, _: subscriber::Interest) {}
            fn metadata(&self) -> &'static Metadata<'static> {
                &$meta
            }
        }
    };
}

log_cs!(
    tracing_core::Level::TRACE,
    TRACE_CS,
    TRACE_META,
    TraceCallsite
);
log_cs!(
    tracing_core::Level::DEBUG,
    DEBUG_CS,
    DEBUG_META,
    DebugCallsite
);
log_cs!(tracing_core::Level::INFO, INFO_CS, INFO_META, InfoCallsite);
log_cs!(tracing_core::Level::WARN, WARN_CS, WARN_META, WarnCallsite);
log_cs!(
    tracing_core::Level::ERROR,
    ERROR_CS,
    ERROR_META,
    ErrorCallsite
);

static TRACE_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&TRACE_CS));
static DEBUG_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&DEBUG_CS));
static INFO_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&INFO_CS));
static WARN_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&WARN_CS));
static ERROR_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&ERROR_CS));

fn level_to_cs(level: Level) -> (&'static dyn Callsite, &'static Fields) {
    match level {
        Level::TRACE => (&TRACE_CS, &*TRACE_FIELDS),
        Level::DEBUG => (&DEBUG_CS, &*DEBUG_FIELDS),
        Level::INFO => (&INFO_CS, &*INFO_FIELDS),
        Level::WARN => (&WARN_CS, &*WARN_FIELDS),
        Level::ERROR => (&ERROR_CS, &*ERROR_FIELDS),
    }
}

fn loglevel_to_cs(
    level: log::Level,
) -> (
    &'static dyn Callsite,
    &'static Fields,
    &'static Metadata<'static>,
) {
    match level {
        log::Level::Trace => (&TRACE_CS, &*TRACE_FIELDS, &TRACE_META),
        log::Level::Debug => (&DEBUG_CS, &*DEBUG_FIELDS, &DEBUG_META),
        log::Level::Info => (&INFO_CS, &*INFO_FIELDS, &INFO_META),
        log::Level::Warn => (&WARN_CS, &*WARN_FIELDS, &WARN_META),
        log::Level::Error => (&ERROR_CS, &*ERROR_FIELDS, &ERROR_META),
    }
}

impl<'a> crate::sealed::Sealed for log::Record<'a> {}

impl<'a> AsTrace for log::Record<'a> {
    type Trace = Metadata<'a>;
    fn as_trace(&self) -> Self::Trace {
        let cs_id = identify_callsite!(loglevel_to_cs(self.level()).0);
        Metadata::new(
            "log record",
            self.target(),
            self.level().as_trace(),
            self.file(),
            self.line(),
            self.module_path(),
            field::FieldSet::new(FIELD_NAMES, cs_id),
            Kind::EVENT,
        )
    }
}

impl crate::sealed::Sealed for tracing_core::Level {}

impl AsLog for tracing_core::Level {
    type Log = log::Level;
    fn as_log(&self) -> log::Level {
        match *self {
            tracing_core::Level::ERROR => log::Level::Error,
            tracing_core::Level::WARN => log::Level::Warn,
            tracing_core::Level::INFO => log::Level::Info,
            tracing_core::Level::DEBUG => log::Level::Debug,
            tracing_core::Level::TRACE => log::Level::Trace,
        }
    }
}

impl crate::sealed::Sealed for log::Level {}

impl AsTrace for log::Level {
    type Trace = tracing_core::Level;
    #[inline]
    fn as_trace(&self) -> tracing_core::Level {
        match self {
            log::Level::Error => tracing_core::Level::ERROR,
            log::Level::Warn => tracing_core::Level::WARN,
            log::Level::Info => tracing_core::Level::INFO,
            log::Level::Debug => tracing_core::Level::DEBUG,
            log::Level::Trace => tracing_core::Level::TRACE,
        }
    }
}

impl crate::sealed::Sealed for log::LevelFilter {}

impl AsTrace for log::LevelFilter {
    type Trace = tracing_core::LevelFilter;
    #[inline]
    fn as_trace(&self) -> tracing_core::LevelFilter {
        match self {
            log::LevelFilter::Off => tracing_core::LevelFilter::OFF,
            log::LevelFilter::Error => tracing_core::LevelFilter::ERROR,
            log::LevelFilter::Warn => tracing_core::LevelFilter::WARN,
            log::LevelFilter::Info => tracing_core::LevelFilter::INFO,
            log::LevelFilter::Debug => tracing_core::LevelFilter::DEBUG,
            log::LevelFilter::Trace => tracing_core::LevelFilter::TRACE,
        }
    }
}

impl crate::sealed::Sealed for tracing_core::LevelFilter {}

impl AsLog for tracing_core::LevelFilter {
    type Log = log::LevelFilter;
    #[inline]
    fn as_log(&self) -> Self::Log {
        match *self {
            tracing_core::LevelFilter::OFF => log::LevelFilter::Off,
            tracing_core::LevelFilter::ERROR => log::LevelFilter::Error,
            tracing_core::LevelFilter::WARN => log::LevelFilter::Warn,
            tracing_core::LevelFilter::INFO => log::LevelFilter::Info,
            tracing_core::LevelFilter::DEBUG => log::LevelFilter::Debug,
            tracing_core::LevelFilter::TRACE => log::LevelFilter::Trace,
        }
    }
}
/// Extends log `Event`s to provide complete `Metadata`.
///
/// In `tracing-log`, an `Event` produced by a log (through [`AsTrace`]) has an hard coded
/// "log" target and no `file`, `line`, or `module_path` attributes. This happens because `Event`
/// requires its `Metadata` to be `'static`, while [`log::Record`]s provide them with a generic
/// lifetime.
///
/// However, these values are stored in the `Event`'s fields and
/// the [`normalized_metadata`] method allows to build a new `Metadata`
/// that only lives as long as its source `Event`, but provides complete
/// data.
///
/// It can typically be used by `Subscriber`s when processing an `Event`,
/// to allow accessing its complete metadata in a consistent way,
/// regardless of the source of its source.
///
/// [`normalized_metadata`]: NormalizeEvent#normalized_metadata
pub trait NormalizeEvent<'a>: crate::sealed::Sealed {
    /// If this `Event` comes from a `log`, this method provides a new
    /// normalized `Metadata` which has all available attributes
    /// from the original log, including `file`, `line`, `module_path`
    /// and `target`.
    /// Returns `None` is the `Event` is not issued from a `log`.
    fn normalized_metadata(&'a self) -> Option<Metadata<'a>>;
    /// Returns whether this `Event` represents a log (from the `log` crate)
    fn is_log(&self) -> bool;
}

impl<'a> crate::sealed::Sealed for Event<'a> {}

impl<'a> NormalizeEvent<'a> for Event<'a> {
    fn normalized_metadata(&'a self) -> Option<Metadata<'a>> {
        let original = self.metadata();
        if self.is_log() {
            let mut fields = LogVisitor::new_for(self, level_to_cs(*original.level()).1);
            self.record(&mut fields);

            Some(Metadata::new(
                "log event",
                fields.target.unwrap_or("log"),
                *original.level(),
                fields.file,
                fields.line.map(|l| l as u32),
                fields.module_path,
                field::FieldSet::new(&["message"], original.callsite()),
                Kind::EVENT,
            ))
        } else {
            None
        }
    }

    fn is_log(&self) -> bool {
        self.metadata().callsite() == identify_callsite!(level_to_cs(*self.metadata().level()).0)
    }
}

struct LogVisitor<'a> {
    target: Option<&'a str>,
    module_path: Option<&'a str>,
    file: Option<&'a str>,
    line: Option<u64>,
    fields: &'static Fields,
}

impl<'a> LogVisitor<'a> {
    // We don't actually _use_ the provided event argument; it is simply to
    // ensure that the `LogVisitor` does not outlive the event whose fields it
    // is visiting, so that the reference casts in `record_str` are safe.
    fn new_for(_event: &'a Event<'a>, fields: &'static Fields) -> Self {
        Self {
            target: None,
            module_path: None,
            file: None,
            line: None,
            fields,
        }
    }
}

impl<'a> Visit for LogVisitor<'a> {
    fn record_debug(&mut self, _field: &Field, _value: &dyn fmt::Debug) {}

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field == &self.fields.line {
            self.line = Some(value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        unsafe {
            // The `Visit` API erases the string slice's lifetime. However, we
            // know it is part of the `Event` struct with a lifetime of `'a`. If
            // (and only if!) this `LogVisitor` was constructed with the same
            // lifetime parameter `'a` as the event in question, it's safe to
            // cast these string slices to the `'a` lifetime.
            if field == &self.fields.file {
                self.file = Some(&*(value as *const _));
            } else if field == &self.fields.target {
                self.target = Some(&*(value as *const _));
            } else if field == &self.fields.module {
                self.module_path = Some(&*(value as *const _));
            }
        }
    }
}

mod sealed {
    pub trait Sealed {}
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_callsite(level: log::Level) {
        let record = log::Record::builder()
            .args(format_args!("Error!"))
            .level(level)
            .target("myApp")
            .file(Some("server.rs"))
            .line(Some(144))
            .module_path(Some("server"))
            .build();

        let meta = record.as_trace();
        let (cs, _keys, _) = loglevel_to_cs(record.level());
        let cs_meta = cs.metadata();
        assert_eq!(
            meta.callsite(),
            cs_meta.callsite(),
            "actual: {:#?}\nexpected: {:#?}",
            meta,
            cs_meta
        );
        assert_eq!(meta.level(), &level.as_trace());
    }

    #[test]
    fn error_callsite_is_correct() {
        test_callsite(log::Level::Error);
    }

    #[test]
    fn warn_callsite_is_correct() {
        test_callsite(log::Level::Warn);
    }

    #[test]
    fn info_callsite_is_correct() {
        test_callsite(log::Level::Info);
    }

    #[test]
    fn debug_callsite_is_correct() {
        test_callsite(log::Level::Debug);
    }

    #[test]
    fn trace_callsite_is_correct() {
        test_callsite(log::Level::Trace);
    }
}
