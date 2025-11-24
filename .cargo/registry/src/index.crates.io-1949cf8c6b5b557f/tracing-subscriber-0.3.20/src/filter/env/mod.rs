//! A `Layer` that enables or disables spans and events based on a set of
//! filtering directives.

// these are publicly re-exported, but the compiler doesn't realize
// that for some reason.
#[allow(unreachable_pub)]
pub use self::{builder::Builder, directive::Directive, field::BadName as BadFieldName};
mod builder;
mod directive;
mod field;

use crate::{
    filter::LevelFilter,
    layer::{Context, Layer},
    sync::RwLock,
};
use directive::ParseError;
use std::{cell::RefCell, collections::HashMap, env, error::Error, fmt, str::FromStr};
use thread_local::ThreadLocal;
use tracing_core::{
    callsite,
    field::Field,
    span,
    subscriber::{Interest, Subscriber},
    Metadata,
};

/// A [`Layer`] which filters spans and events based on a set of filter
/// directives.
///
/// `EnvFilter` implements both the [`Layer`](#impl-Layer<S>) and [`Filter`] traits, so it may
/// be used for both [global filtering][global] and [per-layer filtering][plf],
/// respectively. See [the documentation on filtering with `Layer`s][filtering]
/// for details.
///
/// The [`Targets`] type implements a similar form of filtering, but without the
/// ability to dynamically enable events based on the current span context, and
/// without filtering on field values. When these features are not required,
/// [`Targets`] provides a lighter-weight alternative to [`EnvFilter`].
///
/// # Directives
///
/// A filter consists of one or more comma-separated directives which match on [`Span`]s and [`Event`]s.
/// Each directive may have a corresponding maximum verbosity [`level`] which
/// enables (e.g., _selects for_) spans and events that match. Like `log`,
/// `tracing` considers less exclusive levels (like `trace` or `info`) to be more
/// verbose than more exclusive levels (like `error` or `warn`).
///
/// The directive syntax is similar to that of [`env_logger`]'s. At a high level, the syntax for directives
/// consists of several parts:
///
/// ```text
/// target[span{field=value}]=level
/// ```
///
/// Each component (`target`, `span`, `field`, `value`, and `level`) will be covered in turn.
///
/// - `target` matches the event or span's target. In general, this is the module path and/or crate name.
///   Examples of targets `h2`, `tokio::net`, or `tide::server`. For more information on targets,
///   please refer to [`Metadata`]'s documentation.
/// - `span` matches on the span's name. If a `span` directive is provided alongside a `target`,
///   the `span` directive will match on spans _within_ the `target`.
/// - `field` matches on [fields] within spans. Field names can also be supplied without a `value`
///   and will match on any [`Span`] or [`Event`] that has a field with that name.
///   For example: `[span{field=\"value\"}]=debug`, `[{field}]=trace`.
/// - `value` matches on the value of a span's field. If a value is a numeric literal or a bool,
///   it will match _only_ on that value. Otherwise, this filter matches the
///   [`std::fmt::Debug`] output from the value.
/// - `level` sets a maximum verbosity level accepted by this directive.
///
/// When a field value directive (`[{<FIELD NAME>=<FIELD_VALUE>}]=...`) matches a
/// value's [`std::fmt::Debug`] output (i.e., the field value in the directive
/// is not a `bool`, `i64`, `u64`, or `f64` literal), the matched pattern may be
/// interpreted as either a regular expression or as the precise expected
/// output of the field's [`std::fmt::Debug`] implementation. By default, these
/// filters are interpreted as regular expressions, but this can be disabled
/// using the [`Builder::with_regex`] builder method to use precise matching
/// instead.
///
/// When field value filters are interpreted as regular expressions, the
/// [`regex` crate's regular expression syntax][re-syntax] is supported.
///
/// **Note**: When filters are constructed from potentially untrusted inputs,
/// [disabling regular expression matching](Builder::with_regex) is strongly
/// recommended.
///
/// ## Usage Notes
///
/// - The portion of the directive which is included within the square brackets is `tracing`-specific.
/// - Any portion of the directive can be omitted.
///     - The sole exception are the `field` and `value` directives. If a `value` is provided,
///       a `field` must _also_ be provided. However, the converse does not hold, as fields can
///       be matched without a value.
/// - If only a level is provided, it will set the maximum level for all `Span`s and `Event`s
///   that are not enabled by other filters.
/// - A directive without a level will enable anything that it matches. This is equivalent to `=trace`.
/// - When a crate has a dash in its name, the default target for events will be the
///   crate's module path as it appears in Rust. This means every dash will be replaced
///   with an underscore.
/// - A dash in a target will only appear when being specified explicitly:
///   `tracing::info!(target: "target-name", ...);`
///
/// ## Example Syntax
///
/// - `tokio::net=info` will enable all spans or events that:
///    - have the `tokio::net` target,
///    - at the level `info` or above.
/// - `warn,tokio::net=info` will enable all spans and events that:
///    - are at the level `warn` or above, *or*
///    - have the `tokio::net` target at the level `info` or above.
/// - `my_crate[span_a]=trace` will enable all spans and events that:
///    - are within the `span_a` span or named `span_a` _if_ `span_a` has the target `my_crate`,
///    - at the level `trace` or above.
/// - `[span_b{name=\"bob\"}]` will enable all spans or event that:
///    - have _any_ target,
///    - are inside a span named `span_b`,
///    - which has a field named `name` with value `bob`,
///    - at _any_ level.
///
/// # Examples
///
/// Parsing an `EnvFilter` from the [default environment
/// variable](EnvFilter::from_default_env) (`RUST_LOG`):
///
/// ```
/// use tracing_subscriber::{EnvFilter, fmt, prelude::*};
///
/// tracing_subscriber::registry()
///     .with(fmt::layer())
///     .with(EnvFilter::from_default_env())
///     .init();
/// ```
///
/// Parsing an `EnvFilter` [from a user-provided environment
/// variable](EnvFilter::from_env):
///
/// ```
/// use tracing_subscriber::{EnvFilter, fmt, prelude::*};
///
/// tracing_subscriber::registry()
///     .with(fmt::layer())
///     .with(EnvFilter::from_env("MYAPP_LOG"))
///     .init();
/// ```
///
/// Using `EnvFilter` as a [per-layer filter][plf] to filter only a single
/// [`Layer`]:
///
/// ```
/// use tracing_subscriber::{EnvFilter, fmt, prelude::*};
///
/// // Parse an `EnvFilter` configuration from the `RUST_LOG`
/// // environment variable.
/// let filter = EnvFilter::from_default_env();
///
/// // Apply the filter to this layer *only*.
/// let filtered_layer = fmt::layer().with_filter(filter);
///
/// // Some other layer, whose output we don't want to filter.
/// let unfiltered_layer = // ...
///     # fmt::layer();
///
/// tracing_subscriber::registry()
///     .with(filtered_layer)
///     .with(unfiltered_layer)
///     .init();
/// ```
/// # Constructing `EnvFilter`s
///
/// An `EnvFilter` is be constructed by parsing a string containing one or more
/// directives. The [`EnvFilter::new`] constructor parses an `EnvFilter` from a
/// string, ignoring any invalid directives, while [`EnvFilter::try_new`]
/// returns an error if invalid directives are encountered. Similarly, the
/// [`EnvFilter::from_env`] and [`EnvFilter::try_from_env`] constructors parse
/// an `EnvFilter` from the value of the provided environment variable, with
/// lossy and strict validation, respectively.
///
/// A [builder](EnvFilter::builder) interface is available to set additional
/// configuration options prior to parsing an `EnvFilter`. See the [`Builder`
/// type's documentation](Builder) for details on the options that can be
/// configured using the builder.
///
/// [`Span`]: tracing_core::span
/// [fields]: tracing_core::Field
/// [`Event`]: tracing_core::Event
/// [`level`]: tracing_core::Level
/// [`Metadata`]: tracing_core::Metadata
/// [`Targets`]: crate::filter::Targets
/// [`env_logger`]: https://crates.io/crates/env_logger
/// [`Filter`]: #impl-Filter<S>
/// [global]: crate::layer#global-filtering
/// [plf]: crate::layer#per-layer-filtering
/// [filtering]: crate::layer#filtering-with-layers
/// [re-syntax]: https://docs.rs/regex/1.11.1/regex/#syntax
#[cfg_attr(docsrs, doc(cfg(all(feature = "env-filter", feature = "std"))))]
#[derive(Debug)]
pub struct EnvFilter {
    statics: directive::Statics,
    dynamics: directive::Dynamics,
    has_dynamics: bool,
    by_id: RwLock<HashMap<span::Id, directive::SpanMatcher>>,
    by_cs: RwLock<HashMap<callsite::Identifier, directive::CallsiteMatcher>>,
    scope: ThreadLocal<RefCell<Vec<LevelFilter>>>,
    regex: bool,
}

/// Creates an [`EnvFilter`] with the same directives as `self`.
///
/// This does *not* clone any of the dynamic state that [`EnvFilter`] acquires while attached to a
/// subscriber.
impl Clone for EnvFilter {
    fn clone(&self) -> EnvFilter {
        EnvFilter {
            statics: self.statics.clone(),
            dynamics: self.dynamics.clone(),
            has_dynamics: self.has_dynamics,
            by_id: RwLock::default(),
            by_cs: RwLock::default(),
            scope: ThreadLocal::new(),
            regex: self.regex,
        }
    }
}

type FieldMap<T> = HashMap<Field, T>;

/// Indicates that an error occurred while parsing a `EnvFilter` from an
/// environment variable.
#[cfg_attr(docsrs, doc(cfg(all(feature = "env-filter", feature = "std"))))]
#[derive(Debug)]
pub struct FromEnvError {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    Parse(ParseError),
    Env(env::VarError),
}

impl EnvFilter {
    /// `RUST_LOG` is the default environment variable used by
    /// [`EnvFilter::from_default_env`] and [`EnvFilter::try_from_default_env`].
    ///
    /// [`EnvFilter::from_default_env`]: EnvFilter::from_default_env()
    /// [`EnvFilter::try_from_default_env`]: EnvFilter::try_from_default_env()
    pub const DEFAULT_ENV: &'static str = "RUST_LOG";

    // === constructors, etc ===

    /// Returns a [builder] that can be used to configure a new [`EnvFilter`]
    /// instance.
    ///
    /// The [`Builder`] type is used to set additional configurations, such as
    /// [whether regular expressions are enabled](Builder::with_regex) or [the
    /// default directive](Builder::with_default_directive) before parsing an
    /// [`EnvFilter`] from a string or environment variable.
    ///
    /// [builder]: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Returns a new `EnvFilter` from the value of the `RUST_LOG` environment
    /// variable, ignoring any invalid filter directives.
    ///
    /// If the environment variable is empty or not set, or if it contains only
    /// invalid directives, a default directive enabling the [`ERROR`] level is
    /// added.
    ///
    /// To set additional configuration options prior to parsing the filter, use
    /// the [`Builder`] type instead.
    ///
    /// This function is equivalent to the following:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// # fn docs() -> EnvFilter {
    /// EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::ERROR.into())
    ///     .from_env_lossy()
    /// # }
    /// ```
    ///
    /// [`ERROR`]: tracing::Level::ERROR
    pub fn from_default_env() -> Self {
        Self::builder()
            .with_default_directive(LevelFilter::ERROR.into())
            .from_env_lossy()
    }

    /// Returns a new `EnvFilter` from the value of the given environment
    /// variable, ignoring any invalid filter directives.
    ///
    /// If the environment variable is empty or not set, or if it contains only
    /// invalid directives, a default directive enabling the [`ERROR`] level is
    /// added.
    ///
    /// To set additional configuration options prior to parsing the filter, use
    /// the [`Builder`] type instead.
    ///
    /// This function is equivalent to the following:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// # fn docs() -> EnvFilter {
    /// # let env = "";
    /// EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::ERROR.into())
    ///     .with_env_var(env)
    ///     .from_env_lossy()
    /// # }
    /// ```
    ///
    /// [`ERROR`]: tracing::Level::ERROR
    pub fn from_env<A: AsRef<str>>(env: A) -> Self {
        Self::builder()
            .with_default_directive(LevelFilter::ERROR.into())
            .with_env_var(env.as_ref())
            .from_env_lossy()
    }

    /// Returns a new `EnvFilter` from the directives in the given string,
    /// ignoring any that are invalid.
    ///
    /// If the string is empty or contains only invalid directives, a default
    /// directive enabling the [`ERROR`] level is added.
    ///
    /// To set additional configuration options prior to parsing the filter, use
    /// the [`Builder`] type instead.
    ///
    /// This function is equivalent to the following:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// # fn docs() -> EnvFilter {
    /// # let directives = "";
    /// EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::ERROR.into())
    ///     .parse_lossy(directives)
    /// # }
    /// ```
    ///
    /// [`ERROR`]: tracing::Level::ERROR
    pub fn new<S: AsRef<str>>(directives: S) -> Self {
        Self::builder()
            .with_default_directive(LevelFilter::ERROR.into())
            .parse_lossy(directives)
    }

    /// Returns a new `EnvFilter` from the directives in the given string,
    /// or an error if any are invalid.
    ///
    /// If the string is empty, a default directive enabling the [`ERROR`] level
    /// is added.
    ///
    /// To set additional configuration options prior to parsing the filter, use
    /// the [`Builder`] type instead.
    ///
    /// This function is equivalent to the following:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// # fn docs() -> Result<EnvFilter, tracing_subscriber::filter::ParseError> {
    /// # let directives = "";
    /// EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::ERROR.into())
    ///     .parse(directives)
    /// # }
    /// ```
    ///
    /// [`ERROR`]: tracing::Level::ERROR
    pub fn try_new<S: AsRef<str>>(dirs: S) -> Result<Self, directive::ParseError> {
        Self::builder().parse(dirs)
    }

    /// Returns a new `EnvFilter` from the value of the `RUST_LOG` environment
    /// variable, or an error if the environment variable is unset or contains
    /// any invalid filter directives.
    ///
    /// To set additional configuration options prior to parsing the filter, use
    /// the [`Builder`] type instead.
    ///
    /// This function is equivalent to the following:
    ///
    /// ```rust
    /// use tracing_subscriber::EnvFilter;
    ///
    /// # fn docs() -> Result<EnvFilter, tracing_subscriber::filter::FromEnvError> {
    /// EnvFilter::builder().try_from_env()
    /// # }
    /// ```
    pub fn try_from_default_env() -> Result<Self, FromEnvError> {
        Self::builder().try_from_env()
    }

    /// Returns a new `EnvFilter` from the value of the given environment
    /// variable, or an error if the environment variable is unset or contains
    /// any invalid filter directives.
    ///
    /// To set additional configuration options prior to parsing the filter, use
    /// the [`Builder`] type instead.
    ///
    /// This function is equivalent to the following:
    ///
    /// ```rust
    /// use tracing_subscriber::EnvFilter;
    ///
    /// # fn docs() -> Result<EnvFilter, tracing_subscriber::filter::FromEnvError> {
    /// # let env = "";
    /// EnvFilter::builder().with_env_var(env).try_from_env()
    /// # }
    /// ```
    pub fn try_from_env<A: AsRef<str>>(env: A) -> Result<Self, FromEnvError> {
        Self::builder().with_env_var(env.as_ref()).try_from_env()
    }

    /// Add a filtering directive to this `EnvFilter`.
    ///
    /// The added directive will be used in addition to any previously set
    /// directives, either added using this method or provided when the filter
    /// is constructed.
    ///
    /// Filters may be created from [`LevelFilter`] or [`Level`], which will
    /// enable all traces at or below a certain verbosity level, or
    /// parsed from a string specifying a directive.
    ///
    /// If a filter directive is inserted that matches exactly the same spans
    /// and events as a previous filter, but sets a different level for those
    /// spans and events, the previous directive is overwritten.
    ///
    /// [`LevelFilter`]: super::LevelFilter
    /// [`Level`]: tracing_core::Level
    ///
    /// # Examples
    ///
    /// From [`LevelFilter`]:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    /// let mut filter = EnvFilter::from_default_env()
    ///     .add_directive(LevelFilter::INFO.into());
    /// ```
    ///
    /// Or from [`Level`]:
    ///
    /// ```rust
    /// # use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    /// # use tracing::Level;
    /// let mut filter = EnvFilter::from_default_env()
    ///     .add_directive(Level::INFO.into());
    /// ```
    ///
    /// Parsed from a string:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, Directive};
    ///
    /// # fn try_mk_filter() -> Result<(), Box<dyn ::std::error::Error>> {
    /// let mut filter = EnvFilter::try_from_default_env()?
    ///     .add_directive("my_crate::module=trace".parse()?)
    ///     .add_directive("my_crate::my_other_module::something=info".parse()?);
    /// # Ok(())
    /// # }
    /// ```
    /// In the above example, substitute `my_crate`, `module`, etc. with the
    /// name your target crate/module is imported with. This might be
    /// different from the package name in Cargo.toml (`-` is replaced by `_`).
    /// Example, if the package name in your Cargo.toml is `MY-FANCY-LIB`, then
    /// the corresponding Rust identifier would be `MY_FANCY_LIB`:
    pub fn add_directive(mut self, mut directive: Directive) -> Self {
        if !self.regex {
            directive.deregexify();
        }
        if let Some(stat) = directive.to_static() {
            self.statics.add(stat)
        } else {
            self.has_dynamics = true;
            self.dynamics.add(directive);
        }
        self
    }

    // === filtering methods ===

    /// Returns `true` if this `EnvFilter` would enable the provided `metadata`
    /// in the current context.
    ///
    /// This is equivalent to calling the [`Layer::enabled`] or
    /// [`Filter::enabled`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope.
    pub fn enabled<S>(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        let level = metadata.level();

        // is it possible for a dynamic filter directive to enable this event?
        // if not, we can avoid the thread local access + iterating over the
        // spans in the current scope.
        if self.has_dynamics && self.dynamics.max_level >= *level {
            if metadata.is_span() {
                // If the metadata is a span, see if we care about its callsite.
                let enabled_by_cs = self
                    .by_cs
                    .read()
                    .ok()
                    .map(|by_cs| by_cs.contains_key(&metadata.callsite()))
                    .unwrap_or(false);
                if enabled_by_cs {
                    return true;
                }
            }

            let enabled_by_scope = {
                let scope = self.scope.get_or_default().borrow();
                for filter in &*scope {
                    if filter >= level {
                        return true;
                    }
                }
                false
            };
            if enabled_by_scope {
                return true;
            }
        }

        // is it possible for a static filter directive to enable this event?
        if self.statics.max_level >= *level {
            // Otherwise, fall back to checking if the callsite is
            // statically enabled.
            return self.statics.enabled(metadata);
        }

        false
    }

    /// Returns an optional hint of the highest [verbosity level][level] that
    /// this `EnvFilter` will enable.
    ///
    /// This is equivalent to calling the [`Layer::max_level_hint`] or
    /// [`Filter::max_level_hint`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope.
    ///
    /// [level]: tracing_core::metadata::Level
    pub fn max_level_hint(&self) -> Option<LevelFilter> {
        if self.dynamics.has_value_filters() {
            // If we perform any filtering on span field *values*, we will
            // enable *all* spans, because their field values are not known
            // until recording.
            return Some(LevelFilter::TRACE);
        }
        std::cmp::max(
            self.statics.max_level.into(),
            self.dynamics.max_level.into(),
        )
    }

    /// Informs the filter that a new span was created.
    ///
    /// This is equivalent to calling the [`Layer::on_new_span`] or
    /// [`Filter::on_new_span`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope.
    pub fn on_new_span<S>(&self, attrs: &span::Attributes<'_>, id: &span::Id, _: Context<'_, S>) {
        let by_cs = try_lock!(self.by_cs.read());
        if let Some(cs) = by_cs.get(&attrs.metadata().callsite()) {
            let span = cs.to_span_match(attrs);
            try_lock!(self.by_id.write()).insert(id.clone(), span);
        }
    }

    /// Informs the filter that the span with the provided `id` was entered.
    ///
    /// This is equivalent to calling the [`Layer::on_enter`] or
    /// [`Filter::on_enter`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope.
    pub fn on_enter<S>(&self, id: &span::Id, _: Context<'_, S>) {
        // XXX: This is where _we_ could push IDs to the stack instead, and use
        // that to allow changing the filter while a span is already entered.
        // But that might be much less efficient...
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            self.scope.get_or_default().borrow_mut().push(span.level());
        }
    }

    /// Informs the filter that the span with the provided `id` was exited.
    ///
    /// This is equivalent to calling the [`Layer::on_exit`] or
    /// [`Filter::on_exit`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope.
    pub fn on_exit<S>(&self, id: &span::Id, _: Context<'_, S>) {
        if self.cares_about_span(id) {
            self.scope.get_or_default().borrow_mut().pop();
        }
    }

    /// Informs the filter that the span with the provided `id` was closed.
    ///
    /// This is equivalent to calling the [`Layer::on_close`] or
    /// [`Filter::on_close`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope.
    pub fn on_close<S>(&self, id: span::Id, _: Context<'_, S>) {
        // If we don't need to acquire a write lock, avoid doing so.
        if !self.cares_about_span(&id) {
            return;
        }

        let mut spans = try_lock!(self.by_id.write());
        spans.remove(&id);
    }

    /// Informs the filter that the span with the provided `id` recorded the
    /// provided field `values`.
    ///
    /// This is equivalent to calling the [`Layer::on_record`] or
    /// [`Filter::on_record`] methods on `EnvFilter`'s implementations of those
    /// traits, but it does not require the trait to be in scope
    pub fn on_record<S>(&self, id: &span::Id, values: &span::Record<'_>, _: Context<'_, S>) {
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            span.record_update(values);
        }
    }

    fn cares_about_span(&self, span: &span::Id) -> bool {
        let spans = try_lock!(self.by_id.read(), else return false);
        spans.contains_key(span)
    }

    fn base_interest(&self) -> Interest {
        if self.has_dynamics {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }

    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.has_dynamics && metadata.is_span() {
            // If this metadata describes a span, first, check if there is a
            // dynamic filter that should be constructed for it. If so, it
            // should always be enabled, since it influences filtering.
            if let Some(matcher) = self.dynamics.matcher(metadata) {
                let mut by_cs = try_lock!(self.by_cs.write(), else return self.base_interest());
                by_cs.insert(metadata.callsite(), matcher);
                return Interest::always();
            }
        }

        // Otherwise, check if any of our static filters enable this metadata.
        if self.statics.enabled(metadata) {
            Interest::always()
        } else {
            self.base_interest()
        }
    }
}

impl<S: Subscriber> Layer<S> for EnvFilter {
    #[inline]
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        EnvFilter::register_callsite(self, metadata)
    }

    #[inline]
    fn max_level_hint(&self) -> Option<LevelFilter> {
        EnvFilter::max_level_hint(self)
    }

    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
        self.enabled(metadata, ctx)
    }

    #[inline]
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        self.on_new_span(attrs, id, ctx)
    }

    #[inline]
    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        self.on_record(id, values, ctx);
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        self.on_enter(id, ctx);
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        self.on_exit(id, ctx);
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        self.on_close(id, ctx);
    }
}

feature! {
    #![all(feature = "registry", feature = "std")]
    use crate::layer::Filter;

    impl<S> Filter<S> for EnvFilter {
        #[inline]
        fn enabled(&self, meta: &Metadata<'_>, ctx: &Context<'_, S>) -> bool {
            self.enabled(meta, ctx.clone())
        }

        #[inline]
        fn callsite_enabled(&self, meta: &'static Metadata<'static>) -> Interest {
            self.register_callsite(meta)
        }

        #[inline]
        fn max_level_hint(&self) -> Option<LevelFilter> {
            EnvFilter::max_level_hint(self)
        }

        #[inline]
        fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
            self.on_new_span(attrs, id, ctx)
        }

        #[inline]
        fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
            self.on_record(id, values, ctx);
        }

        #[inline]
        fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
            self.on_enter(id, ctx);
        }

        #[inline]
        fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
            self.on_exit(id, ctx);
        }

        #[inline]
        fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
            self.on_close(id, ctx);
        }
    }
}

impl FromStr for EnvFilter {
    type Err = directive::ParseError;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        Self::try_new(spec)
    }
}

impl<S> From<S> for EnvFilter
where
    S: AsRef<str>,
{
    fn from(s: S) -> Self {
        Self::new(s)
    }
}

impl Default for EnvFilter {
    fn default() -> Self {
        Builder::default().from_directives(std::iter::empty())
    }
}

impl fmt::Display for EnvFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut statics = self.statics.iter();
        let wrote_statics = if let Some(next) = statics.next() {
            fmt::Display::fmt(next, f)?;
            for directive in statics {
                write!(f, ",{}", directive)?;
            }
            true
        } else {
            false
        };

        let mut dynamics = self.dynamics.iter();
        if let Some(next) = dynamics.next() {
            if wrote_statics {
                f.write_str(",")?;
            }
            fmt::Display::fmt(next, f)?;
            for directive in dynamics {
                write!(f, ",{}", directive)?;
            }
        }
        Ok(())
    }
}

// ===== impl FromEnvError =====

impl From<directive::ParseError> for FromEnvError {
    fn from(p: directive::ParseError) -> Self {
        Self {
            kind: ErrorKind::Parse(p),
        }
    }
}

impl From<env::VarError> for FromEnvError {
    fn from(v: env::VarError) -> Self {
        Self {
            kind: ErrorKind::Env(v),
        }
    }
}

impl fmt::Display for FromEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Parse(ref p) => p.fmt(f),
            ErrorKind::Env(ref e) => e.fmt(f),
        }
    }
}

impl Error for FromEnvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.kind {
            ErrorKind::Parse(ref p) => Some(p),
            ErrorKind::Env(ref e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_core::field::FieldSet;
    use tracing_core::*;

    struct NoSubscriber;
    impl Subscriber for NoSubscriber {
        #[inline]
        fn register_callsite(&self, _: &'static Metadata<'static>) -> subscriber::Interest {
            subscriber::Interest::always()
        }
        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(0xDEAD)
        }
        fn event(&self, _event: &Event<'_>) {}
        fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}
        fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

        #[inline]
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }
        fn enter(&self, _span: &span::Id) {}
        fn exit(&self, _span: &span::Id) {}
    }

    struct Cs;
    impl Callsite for Cs {
        fn set_interest(&self, _interest: Interest) {}
        fn metadata(&self) -> &Metadata<'_> {
            unimplemented!()
        }
    }

    #[test]
    fn callsite_enabled_no_span_directive() {
        let filter = EnvFilter::new("app=debug").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&[], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_never());
    }

    #[test]
    fn callsite_off() {
        let filter = EnvFilter::new("app=off").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::ERROR,
            None,
            None,
            None,
            FieldSet::new(&[], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_never());
    }

    #[test]
    fn callsite_enabled_includes_span_directive() {
        let filter = EnvFilter::new("app[mySpan]=debug").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&[], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_always());
    }

    #[test]
    fn callsite_enabled_includes_span_directive_field() {
        let filter =
            EnvFilter::new("app[mySpan{field=\"value\"}]=debug").with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&["field"], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_always());
    }

    #[test]
    fn callsite_enabled_includes_span_directive_multiple_fields() {
        let filter = EnvFilter::new("app[mySpan{field=\"value\",field2=2}]=debug")
            .with_subscriber(NoSubscriber);
        static META: &Metadata<'static> = &Metadata::new(
            "mySpan",
            "app",
            Level::TRACE,
            None,
            None,
            None,
            FieldSet::new(&["field"], identify_callsite!(&Cs)),
            Kind::SPAN,
        );

        let interest = filter.register_callsite(META);
        assert!(interest.is_never());
    }

    #[test]
    fn roundtrip() {
        let f1: EnvFilter =
            "[span1{foo=1}]=error,[span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug"
                .parse()
                .unwrap();
        let f2: EnvFilter = format!("{}", f1).parse().unwrap();
        assert_eq!(f1.statics, f2.statics);
        assert_eq!(f1.dynamics, f2.dynamics);
    }

    #[test]
    fn size_of_filters() {
        fn print_sz(s: &str) {
            let filter = s.parse::<EnvFilter>().expect("filter should parse");
            println!(
                "size_of_val({:?})\n -> {}B",
                s,
                std::mem::size_of_val(&filter)
            );
        }

        print_sz("info");

        print_sz("foo=debug");

        print_sz(
            "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
            crate2=debug,crate3=trace,crate3::mod2::mod1=off",
        );

        print_sz("[span1{foo=1}]=error,[span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug");

        print_sz(
            "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
            crate2=debug,crate3=trace,crate3::mod2::mod1=off,[span1{foo=1}]=error,\
            [span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug",
        );
    }

    #[test]
    fn parse_empty_string() {
        // There is no corresponding test for [`Builder::parse_lossy`] as failed
        // parsing does not produce any observable side effects. If this test fails
        // check that [`Builder::parse_lossy`] is behaving correctly as well.
        assert!(EnvFilter::builder().parse("").is_ok());
    }
}
