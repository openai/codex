//! The [`Layer`] trait, a composable abstraction for building [`Subscriber`]s.
//!
//! The [`Subscriber`] trait in `tracing-core` represents the _complete_ set of
//! functionality required to consume `tracing` instrumentation. This means that
//! a single `Subscriber` instance is a self-contained implementation of a
//! complete strategy for collecting traces; but it _also_ means that the
//! `Subscriber` trait cannot easily be composed with other `Subscriber`s.
//!
//! In particular, [`Subscriber`]s are responsible for generating [span IDs] and
//! assigning them to spans. Since these IDs must uniquely identify a span
//! within the context of the current trace, this means that there may only be
//! a single `Subscriber` for a given thread at any point in time &mdash;
//! otherwise, there would be no authoritative source of span IDs.
//!
//! On the other hand, the majority of the [`Subscriber`] trait's functionality
//! is composable: any number of subscribers may _observe_ events, span entry
//! and exit, and so on, provided that there is a single authoritative source of
//! span IDs. The [`Layer`] trait represents this composable subset of the
//! [`Subscriber`] behavior; it can _observe_ events and spans, but does not
//! assign IDs.
//!
//! # Composing Layers
//!
//! Since a [`Layer`] does not implement a complete strategy for collecting
//! traces, it must be composed with a `Subscriber` in order to be used. The
//! [`Layer`] trait is generic over a type parameter (called `S` in the trait
//! definition), representing the types of `Subscriber` they can be composed
//! with. Thus, a [`Layer`] may be implemented that will only compose with a
//! particular `Subscriber` implementation, or additional trait bounds may be
//! added to constrain what types implementing `Subscriber` a `Layer` can wrap.
//!
//! `Layer`s may be added to a `Subscriber` by using the [`SubscriberExt::with`]
//! method, which is provided by `tracing-subscriber`'s [prelude]. This method
//! returns a [`Layered`] struct that implements `Subscriber` by composing the
//! `Layer` with the `Subscriber`.
//!
//! For example:
//! ```rust
//! use tracing_subscriber::Layer;
//! use tracing_subscriber::prelude::*;
//! use tracing::Subscriber;
//!
//! pub struct MyLayer {
//!     // ...
//! }
//!
//! impl<S: Subscriber> Layer<S> for MyLayer {
//!     // ...
//! }
//!
//! pub struct MySubscriber {
//!     // ...
//! }
//!
//! # use tracing_core::{span::{Id, Attributes, Record}, Metadata, Event};
//! impl Subscriber for MySubscriber {
//!     // ...
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(1) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! }
//! # impl MyLayer {
//! # fn new() -> Self { Self {} }
//! # }
//! # impl MySubscriber {
//! # fn new() -> Self { Self { }}
//! # }
//!
//! let subscriber = MySubscriber::new()
//!     .with(MyLayer::new());
//!
//! tracing::subscriber::set_global_default(subscriber);
//! ```
//!
//! Multiple `Layer`s may be composed in the same manner:
//! ```rust
//! # use tracing_subscriber::{Layer, layer::SubscriberExt};
//! # use tracing::Subscriber;
//! pub struct MyOtherLayer {
//!     // ...
//! }
//!
//! impl<S: Subscriber> Layer<S> for MyOtherLayer {
//!     // ...
//! }
//!
//! pub struct MyThirdLayer {
//!     // ...
//! }
//!
//! impl<S: Subscriber> Layer<S> for MyThirdLayer {
//!     // ...
//! }
//! # pub struct MyLayer {}
//! # impl<S: Subscriber> Layer<S> for MyLayer {}
//! # pub struct MySubscriber { }
//! # use tracing_core::{span::{Id, Attributes, Record}, Metadata, Event};
//! # impl Subscriber for MySubscriber {
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(1) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! }
//! # impl MyLayer {
//! # fn new() -> Self { Self {} }
//! # }
//! # impl MyOtherLayer {
//! # fn new() -> Self { Self {} }
//! # }
//! # impl MyThirdLayer {
//! # fn new() -> Self { Self {} }
//! # }
//! # impl MySubscriber {
//! # fn new() -> Self { Self { }}
//! # }
//!
//! let subscriber = MySubscriber::new()
//!     .with(MyLayer::new())
//!     .with(MyOtherLayer::new())
//!     .with(MyThirdLayer::new());
//!
//! tracing::subscriber::set_global_default(subscriber);
//! ```
//!
//! The [`Layer::with_subscriber`] constructs the [`Layered`] type from a
//! [`Layer`] and [`Subscriber`], and is called by [`SubscriberExt::with`]. In
//! general, it is more idiomatic to use [`SubscriberExt::with`], and treat
//! [`Layer::with_subscriber`] as an implementation detail, as `with_subscriber`
//! calls must be nested, leading to less clear code for the reader.
//!
//! ## Runtime Configuration With `Layer`s
//!
//! In some cases, a particular [`Layer`] may be enabled or disabled based on
//! runtime configuration. This can introduce challenges, because the type of a
//! layered [`Subscriber`] depends on which layers are added to it: if an `if`
//! or `match` expression adds some [`Layer`] implementation in one branch,
//! and other layers in another, the [`Subscriber`] values returned by those
//! branches will have different types. For example, the following _will not_
//! work:
//!
//! ```compile_fail
//! # fn docs() -> Result<(), Box<dyn std::error::Error + 'static>> {
//! # struct Config {
//! #    is_prod: bool,
//! #    path: &'static str,
//! # }
//! # let cfg = Config { is_prod: false, path: "debug.log" };
//! use std::fs::File;
//! use tracing_subscriber::{Registry, prelude::*};
//!
//! let stdout_log = tracing_subscriber::fmt::layer().pretty();
//! let subscriber = Registry::default().with(stdout_log);
//!
//! // The compile error will occur here because the if and else
//! // branches have different (and therefore incompatible) types.
//! let subscriber = if cfg.is_prod {
//!     let file = File::create(cfg.path)?;
//!     let layer = tracing_subscriber::fmt::layer()
//!         .json()
//!         .with_writer(Arc::new(file));
//!     layer.with(subscriber)
//! } else {
//!     layer
//! };
//!
//! tracing::subscriber::set_global_default(subscriber)
//!     .expect("Unable to set global subscriber");
//! # Ok(()) }
//! ```
//!
//! However, a [`Layer`] wrapped in an [`Option`] [also implements the `Layer`
//! trait][option-impl]. This allows individual layers to be enabled or disabled at
//! runtime while always producing a [`Subscriber`] of the same type. For
//! example:
//!
//! ```
//! # fn docs() -> Result<(), Box<dyn std::error::Error + 'static>> {
//! # struct Config {
//! #    is_prod: bool,
//! #    path: &'static str,
//! # }
//! # let cfg = Config { is_prod: false, path: "debug.log" };
//! use std::fs::File;
//! use tracing_subscriber::{Registry, prelude::*};
//!
//! let stdout_log = tracing_subscriber::fmt::layer().pretty();
//! let subscriber = Registry::default().with(stdout_log);
//!
//! // if `cfg.is_prod` is true, also log JSON-formatted logs to a file.
//! let json_log = if cfg.is_prod {
//!     let file = File::create(cfg.path)?;
//!     let json_log = tracing_subscriber::fmt::layer()
//!         .json()
//!         .with_writer(file);
//!     Some(json_log)
//! } else {
//!     None
//! };
//!
//! // If `cfg.is_prod` is false, then `json` will be `None`, and this layer
//! // will do nothing. However, the subscriber will still have the same type
//! // regardless of whether the `Option`'s value is `None` or `Some`.
//! let subscriber = subscriber.with(json_log);
//!
//! tracing::subscriber::set_global_default(subscriber)
//!    .expect("Unable to set global subscriber");
//! # Ok(()) }
//! ```
//!
//! If a [`Layer`] may be one of several different types, note that [`Box<dyn
//! Layer<S> + Send + Sync>` implements `Layer`][box-impl].
//! This may be used to erase the type of a [`Layer`].
//!
//! For example, a function that configures a [`Layer`] to log to one of
//! several outputs might return a `Box<dyn Layer<S> + Send + Sync + 'static>`:
//! ```
//! use tracing_subscriber::{
//!     Layer,
//!     registry::LookupSpan,
//!     prelude::*,
//! };
//! use std::{path::PathBuf, fs::File, io};
//!
//! /// Configures whether logs are emitted to a file, to stdout, or to stderr.
//! pub enum LogConfig {
//!     File(PathBuf),
//!     Stdout,
//!     Stderr,
//! }
//!
//! impl LogConfig {
//!     pub fn layer<S>(self) -> Box<dyn Layer<S> + Send + Sync + 'static>
//!     where
//!         S: tracing_core::Subscriber,
//!         for<'a> S: LookupSpan<'a>,
//!     {
//!         // Shared configuration regardless of where logs are output to.
//!         let fmt = tracing_subscriber::fmt::layer()
//!             .with_target(true)
//!             .with_thread_names(true);
//!
//!         // Configure the writer based on the desired log target:
//!         match self {
//!             LogConfig::File(path) => {
//!                 let file = File::create(path).expect("failed to create log file");
//!                 Box::new(fmt.with_writer(file))
//!             },
//!             LogConfig::Stdout => Box::new(fmt.with_writer(io::stdout)),
//!             LogConfig::Stderr => Box::new(fmt.with_writer(io::stderr)),
//!         }
//!     }
//! }
//!
//! let config = LogConfig::Stdout;
//! tracing_subscriber::registry()
//!     .with(config.layer())
//!     .init();
//! ```
//!
//! The [`Layer::boxed`] method is provided to make boxing a `Layer`
//! more convenient, but [`Box::new`] may be used as well.
//!
//! When the number of `Layer`s varies at runtime, note that a
//! [`Vec<L> where L: Layer` also implements `Layer`][vec-impl]. This
//! can be used to add a variable number of `Layer`s to a `Subscriber`:
//!
//! ```
//! use tracing_subscriber::{Layer, prelude::*};
//! struct MyLayer {
//!     // ...
//! }
//! # impl MyLayer { fn new() -> Self { Self {} }}
//!
//! impl<S: tracing_core::Subscriber> Layer<S> for MyLayer {
//!     // ...
//! }
//!
//! /// Returns how many layers we need
//! fn how_many_layers() -> usize {
//!     // ...
//!     # 3
//! }
//!
//! // Create a variable-length `Vec` of layers
//! let mut layers = Vec::new();
//! for _ in 0..how_many_layers() {
//!     layers.push(MyLayer::new());
//! }
//!
//! tracing_subscriber::registry()
//!     .with(layers)
//!     .init();
//! ```
//!
//! If a variable number of `Layer` is needed and those `Layer`s have
//! different types, a `Vec` of [boxed `Layer` trait objects][box-impl] may
//! be used. For example:
//!
//! ```
//! use tracing_subscriber::{filter::LevelFilter, Layer, prelude::*};
//! use std::fs::File;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! struct Config {
//!     enable_log_file: bool,
//!     enable_stdout: bool,
//!     enable_stderr: bool,
//!     // ...
//! }
//! # impl Config {
//! #    fn from_config_file()-> Result<Self, Box<dyn std::error::Error>> {
//! #         // don't enable the log file so that the example doesn't actually create it
//! #         Ok(Self { enable_log_file: false, enable_stdout: true, enable_stderr: true })
//! #    }
//! # }
//!
//! let cfg = Config::from_config_file()?;
//!
//! // Based on our dynamically loaded config file, create any number of layers:
//! let mut layers = Vec::new();
//!
//! if cfg.enable_log_file {
//!     let file = File::create("myapp.log")?;
//!     let layer = tracing_subscriber::fmt::layer()
//!         .with_thread_names(true)
//!         .with_target(true)
//!         .json()
//!         .with_writer(file)
//!         // Box the layer as a type-erased trait object, so that it can
//!         // be pushed to the `Vec`.
//!         .boxed();
//!     layers.push(layer);
//! }
//!
//! if cfg.enable_stdout {
//!     let layer = tracing_subscriber::fmt::layer()
//!         .pretty()
//!         .with_filter(LevelFilter::INFO)
//!         // Box the layer as a type-erased trait object, so that it can
//!         // be pushed to the `Vec`.
//!         .boxed();
//!     layers.push(layer);
//! }
//!
//! if cfg.enable_stdout {
//!     let layer = tracing_subscriber::fmt::layer()
//!         .with_target(false)
//!         .with_filter(LevelFilter::WARN)
//!         // Box the layer as a type-erased trait object, so that it can
//!         // be pushed to the `Vec`.
//!         .boxed();
//!     layers.push(layer);
//! }
//!
//! tracing_subscriber::registry()
//!     .with(layers)
//!     .init();
//!# Ok(()) }
//! ```
//!
//! Finally, if the number of layers _changes_ at runtime, a `Vec` of
//! subscribers can be used alongside the [`reload`](crate::reload) module to
//! add or remove subscribers dynamically at runtime.
//!
//! [option-impl]: Layer#impl-Layer<S>-for-Option<L>
//! [box-impl]: Layer#impl-Layer%3CS%3E-for-Box%3Cdyn%20Layer%3CS%3E%20+%20Send%20+%20Sync%3E
//! [vec-impl]: Layer#impl-Layer<S>-for-Vec<L>
//! [prelude]: crate::prelude
//!
//! # Recording Traces
//!
//! The [`Layer`] trait defines a set of methods for consuming notifications from
//! tracing instrumentation, which are generally equivalent to the similarly
//! named methods on [`Subscriber`]. Unlike [`Subscriber`], the methods on
//! `Layer` are additionally passed a [`Context`] type, which exposes additional
//! information provided by the wrapped subscriber (such as [the current span])
//! to the layer.
//!
//! # Filtering with `Layer`s
//!
//! As well as strategies for handling trace events, the `Layer` trait may also
//! be used to represent composable _filters_. This allows the determination of
//! what spans and events should be recorded to be decoupled from _how_ they are
//! recorded: a filtering layer can be applied to other layers or
//! subscribers. `Layer`s can be used to implement _global filtering_, where a
//! `Layer` provides a filtering strategy for the entire subscriber.
//! Additionally, individual recording `Layer`s or sets of `Layer`s may be
//! combined with _per-layer filters_ that control what spans and events are
//! recorded by those layers.
//!
//! ## Global Filtering
//!
//! A `Layer` that implements a filtering strategy should override the
//! [`register_callsite`] and/or [`enabled`] methods. It may also choose to implement
//! methods such as [`on_enter`], if it wishes to filter trace events based on
//! the current span context.
//!
//! Note that the [`Layer::register_callsite`] and [`Layer::enabled`] methods
//! determine whether a span or event is enabled *globally*. Thus, they should
//! **not** be used to indicate whether an individual layer wishes to record a
//! particular span or event. Instead, if a layer is only interested in a subset
//! of trace data, but does *not* wish to disable other spans and events for the
//! rest of the layer stack should ignore those spans and events in its
//! notification methods.
//!
//! The filtering methods on a stack of `Layer`s are evaluated in a top-down
//! order, starting with the outermost `Layer` and ending with the wrapped
//! [`Subscriber`]. If any layer returns `false` from its [`enabled`] method, or
//! [`Interest::never()`] from its [`register_callsite`] method, filter
//! evaluation will short-circuit and the span or event will be disabled.
//!
//! ### Enabling Interest
//!
//! Whenever an tracing event (or span) is emitted, it goes through a number of
//! steps to determine how and how much it should be processed. The earlier an
//! event is disabled, the less work has to be done to process the event, so
//! `Layer`s that implement filtering should attempt to disable unwanted
//! events as early as possible. In order, each event checks:
//!
//! - [`register_callsite`], once per callsite (roughly: once per time that
//!   `event!` or `span!` is written in the source code; this is cached at the
//!   callsite). See [`Subscriber::register_callsite`] and
//!   [`tracing_core::callsite`] for a summary of how this behaves.
//! - [`enabled`], once per emitted event (roughly: once per time that `event!`
//!   or `span!` is *executed*), and only if `register_callsite` registers an
//!   [`Interest::sometimes`]. This is the main customization point to globally
//!   filter events based on their [`Metadata`]. If an event can be disabled
//!   based only on [`Metadata`], it should be, as this allows the construction
//!   of the actual `Event`/`Span` to be skipped.
//! - For events only (and not spans), [`event_enabled`] is called just before
//!   processing the event. This gives layers one last chance to say that
//!   an event should be filtered out, now that the event's fields are known.
//!
//! ## Per-Layer Filtering
//!
//! **Note**: per-layer filtering APIs currently require the [`"registry"` crate
//! feature flag][feat] to be enabled.
//!
//! Sometimes, it may be desirable for one `Layer` to record a particular subset
//! of spans and events, while a different subset of spans and events are
//! recorded by other `Layer`s. For example:
//!
//! - A layer that records metrics may wish to observe only events including
//!   particular tracked values, while a logging layer ignores those events.
//! - If recording a distributed trace is expensive, it might be desirable to
//!   only send spans with `INFO` and lower verbosity to the distributed tracing
//!   system, while logging more verbose spans to a file.
//! - Spans and events with a particular target might be recorded differently
//!   from others, such as by generating an HTTP access log from a span that
//!   tracks the lifetime of an HTTP request.
//!
//! The [`Filter`] trait is used to control what spans and events are
//! observed by an individual `Layer`, while still allowing other `Layer`s to
//! potentially record them. The [`Layer::with_filter`] method combines a
//! `Layer` with a [`Filter`], returning a [`Filtered`] layer.
//!
//! This crate's [`filter`] module provides a number of types which implement
//! the [`Filter`] trait, such as [`LevelFilter`], [`Targets`], and
//! [`FilterFn`]. These [`Filter`]s provide ready-made implementations of
//! common forms of filtering. For custom filtering policies, the [`FilterFn`]
//! and [`DynFilterFn`] types allow implementing a [`Filter`] with a closure or
//! function pointer. In addition, when more control is required, the [`Filter`]
//! trait may also be implemented for user-defined types.
//!
//! [`Option<Filter>`] also implements [`Filter`], which allows for an optional
//! filter. [`None`] filters out _nothing_ (that is, allows everything through). For
//! example:
//!
//! ```rust
//! # use tracing_subscriber::{filter::filter_fn, Layer};
//! # use tracing_core::{Metadata, subscriber::Subscriber};
//! # struct MyLayer<S>(std::marker::PhantomData<S>);
//! # impl<S> MyLayer<S> { fn new() -> Self { Self(std::marker::PhantomData)} }
//! # impl<S: Subscriber> Layer<S> for MyLayer<S> {}
//! # fn my_filter(_: &str) -> impl Fn(&Metadata) -> bool { |_| true  }
//! fn setup_tracing<S: Subscriber>(filter_config: Option<&str>) {
//!     let layer = MyLayer::<S>::new()
//!         .with_filter(filter_config.map(|config| filter_fn(my_filter(config))));
//! //...
//! }
//! ```
//!
//! <pre class="compile_fail" style="white-space:normal;font:inherit;">
//!     <strong>Warning</strong>: Currently, the <a href="../struct.Registry.html">
//!     <code>Registry</code></a> type defined in this crate is the only root
//!     <code>Subscriber</code> capable of supporting <code>Layer</code>s with
//!     per-layer filters. In the future, new APIs will be added to allow other
//!     root <code>Subscriber</code>s to support per-layer filters.
//! </pre>
//!
//! For example, to generate an HTTP access log based on spans with
//! the `http_access` target, while logging other spans and events to
//! standard out, a [`Filter`] can be added to the access log layer:
//!
//! ```
//! use tracing_subscriber::{filter, prelude::*};
//!
//! // Generates an HTTP access log.
//! let access_log = // ...
//!     # filter::LevelFilter::INFO;
//!
//! // Add a filter to the access log layer so that it only observes
//! // spans and events with the `http_access` target.
//! let access_log = access_log.with_filter(filter::filter_fn(|metadata| {
//!     // Returns `true` if and only if the span or event's target is
//!     // "http_access".
//!     metadata.target() == "http_access"
//! }));
//!
//! // A general-purpose logging layer.
//! let fmt_layer = tracing_subscriber::fmt::layer();
//!
//! // Build a subscriber that combines the access log and stdout log
//! // layers.
//! tracing_subscriber::registry()
//!     .with(fmt_layer)
//!     .with(access_log)
//!     .init();
//! ```
//!
//! Multiple layers can have their own, separate per-layer filters. A span or
//! event will be recorded if it is enabled by _any_ per-layer filter, but it
//! will be skipped by the layers whose filters did not enable it. Building on
//! the previous example:
//!
//! ```
//! use tracing_subscriber::{filter::{filter_fn, LevelFilter}, prelude::*};
//!
//! let access_log = // ...
//!     # LevelFilter::INFO;
//! let fmt_layer = tracing_subscriber::fmt::layer();
//!
//! tracing_subscriber::registry()
//!     // Add the filter for the "http_access" target to the access
//!     // log layer, like before.
//!     .with(access_log.with_filter(filter_fn(|metadata| {
//!         metadata.target() == "http_access"
//!     })))
//!     // Add a filter for spans and events with the INFO level
//!     // and below to the logging layer.
//!     .with(fmt_layer.with_filter(LevelFilter::INFO))
//!     .init();
//!
//! // Neither layer will observe this event
//! tracing::debug!(does_anyone_care = false, "a tree fell in the forest");
//!
//! // This event will be observed by the logging layer, but not
//! // by the access log layer.
//! tracing::warn!(dose_roentgen = %3.8, "not great, but not terrible");
//!
//! // This event will be observed only by the access log layer.
//! tracing::trace!(target: "http_access", "HTTP request started");
//!
//! // Both layers will observe this event.
//! tracing::error!(target: "http_access", "HTTP request failed with a very bad error!");
//! ```
//!
//! A per-layer filter can be applied to multiple [`Layer`]s at a time, by
//! combining them into a [`Layered`] layer using [`Layer::and_then`], and then
//! calling [`Layer::with_filter`] on the resulting [`Layered`] layer.
//!
//! Consider the following:
//! - `layer_a` and `layer_b`, which should only receive spans and events at
//!   the [`INFO`] [level] and above.
//! - A third layer, `layer_c`, which should receive spans and events at
//!   the [`DEBUG`] [level] as well.
//!
//! The layers and filters would be composed thusly:
//!
//! ```
//! use tracing_subscriber::{filter::LevelFilter, prelude::*};
//!
//! let layer_a = // ...
//! # LevelFilter::INFO;
//! let layer_b =  // ...
//! # LevelFilter::INFO;
//! let layer_c =  // ...
//! # LevelFilter::INFO;
//!
//! let info_layers = layer_a
//!     // Combine `layer_a` and `layer_b` into a `Layered` layer:
//!     .and_then(layer_b)
//!     // ...and then add an `INFO` `LevelFilter` to that layer:
//!     .with_filter(LevelFilter::INFO);
//!
//! tracing_subscriber::registry()
//!     // Add `layer_c` with a `DEBUG` filter.
//!     .with(layer_c.with_filter(LevelFilter::DEBUG))
//!     .with(info_layers)
//!     .init();
//!```
//!
//! If a [`Filtered`] [`Layer`] is combined with another [`Layer`]
//! [`Layer::and_then`], and a filter is added to the [`Layered`] layer, that
//! layer will be filtered by *both* the inner filter and the outer filter.
//! Only spans and events that are enabled by *both* filters will be
//! observed by that layer. This can be used to implement complex filtering
//! trees.
//!
//! As an example, consider the following constraints:
//! - Suppose that a particular [target] is used to indicate events that
//!   should be counted as part of a metrics system, which should be only
//!   observed by a layer that collects metrics.
//! - A log of high-priority events ([`INFO`] and above) should be logged
//!   to stdout, while more verbose events should be logged to a debugging log file.
//! - Metrics-focused events should *not* be included in either log output.
//!
//! In that case, it is possible to apply a filter to both logging layers to
//! exclude the metrics events, while additionally adding a [`LevelFilter`]
//! to the stdout log:
//!
//! ```
//! # // wrap this in a function so we don't actually create `debug.log` when
//! # // running the doctests..
//! # fn docs() -> Result<(), Box<dyn std::error::Error + 'static>> {
//! use tracing_subscriber::{filter, prelude::*};
//! use std::{fs::File, sync::Arc};
//!
//! // A layer that logs events to stdout using the human-readable "pretty"
//! // format.
//! let stdout_log = tracing_subscriber::fmt::layer()
//!     .pretty();
//!
//! // A layer that logs events to a file.
//! let file = File::create("debug.log")?;
//! let debug_log = tracing_subscriber::fmt::layer()
//!     .with_writer(Arc::new(file));
//!
//! // A layer that collects metrics using specific events.
//! let metrics_layer = /* ... */ filter::LevelFilter::INFO;
//!
//! tracing_subscriber::registry()
//!     .with(
//!         stdout_log
//!             // Add an `INFO` filter to the stdout logging layer
//!             .with_filter(filter::LevelFilter::INFO)
//!             // Combine the filtered `stdout_log` layer with the
//!             // `debug_log` layer, producing a new `Layered` layer.
//!             .and_then(debug_log)
//!             // Add a filter to *both* layers that rejects spans and
//!             // events whose targets start with `metrics`.
//!             .with_filter(filter::filter_fn(|metadata| {
//!                 !metadata.target().starts_with("metrics")
//!             }))
//!     )
//!     .with(
//!         // Add a filter to the metrics label that *only* enables
//!         // events whose targets start with `metrics`.
//!         metrics_layer.with_filter(filter::filter_fn(|metadata| {
//!             metadata.target().starts_with("metrics")
//!         }))
//!     )
//!     .init();
//!
//! // This event will *only* be recorded by the metrics layer.
//! tracing::info!(target: "metrics::cool_stuff_count", value = 42);
//!
//! // This event will only be seen by the debug log file layer:
//! tracing::debug!("this is a message, and part of a system of messages");
//!
//! // This event will be seen by both the stdout log layer *and*
//! // the debug log file layer, but not by the metrics layer.
//! tracing::warn!("the message is a warning about danger!");
//! # Ok(()) }
//! ```
//!
//! [`Subscriber`]: tracing_core::subscriber::Subscriber
//! [span IDs]: tracing_core::span::Id
//! [the current span]: Context::current_span
//! [`register_callsite`]: Layer::register_callsite
//! [`enabled`]: Layer::enabled
//! [`event_enabled`]: Layer::event_enabled
//! [`on_enter`]: Layer::on_enter
//! [`Layer::register_callsite`]: Layer::register_callsite
//! [`Layer::enabled`]: Layer::enabled
//! [`Interest::never()`]: tracing_core::subscriber::Interest::never()
//! [`Filtered`]: crate::filter::Filtered
//! [`filter`]: crate::filter
//! [`Targets`]: crate::filter::Targets
//! [`FilterFn`]: crate::filter::FilterFn
//! [`DynFilterFn`]: crate::filter::DynFilterFn
//! [level]: tracing_core::Level
//! [`INFO`]: tracing_core::Level::INFO
//! [`DEBUG`]: tracing_core::Level::DEBUG
//! [target]: tracing_core::Metadata::target
//! [`LevelFilter`]: crate::filter::LevelFilter
//! [feat]: crate#feature-flags
use crate::filter;

use tracing_core::{
    metadata::Metadata,
    span,
    subscriber::{Interest, Subscriber},
    Dispatch, Event, LevelFilter,
};

use core::any::TypeId;

feature! {
    #![feature = "alloc"]
    use alloc::boxed::Box;
    use core::ops::{Deref, DerefMut};
}

mod context;
mod layered;
pub use self::{context::*, layered::*};

// The `tests` module is `pub(crate)` because it contains test utilities used by
// other modules.
#[cfg(test)]
pub(crate) mod tests;

/// A composable handler for `tracing` events.
///
/// A `Layer` implements a behavior for recording or collecting traces that can
/// be composed together with other `Layer`s to build a [`Subscriber`]. See the
/// [module-level documentation](crate::layer) for details.
///
/// [`Subscriber`]: tracing_core::Subscriber
#[cfg_attr(docsrs, doc(notable_trait))]
pub trait Layer<S>
where
    S: Subscriber,
    Self: 'static,
{
    /// Performs late initialization when installing this layer as a
    /// [`Subscriber`].
    ///
    /// ## Avoiding Memory Leaks
    ///
    /// `Layer`s should not store the [`Dispatch`] pointing to the [`Subscriber`]
    /// that they are a part of. Because the `Dispatch` owns the `Subscriber`,
    /// storing the `Dispatch` within the `Subscriber` will create a reference
    /// count cycle, preventing the `Dispatch` from ever being dropped.
    ///
    /// Instead, when it is necessary to store a cyclical reference to the
    /// `Dispatch` within a `Layer`, use [`Dispatch::downgrade`] to convert a
    /// `Dispatch` into a [`WeakDispatch`]. This type is analogous to
    /// [`std::sync::Weak`], and does not create a reference count cycle. A
    /// [`WeakDispatch`] can be stored within a subscriber without causing a
    /// memory leak, and can be [upgraded] into a `Dispatch` temporarily when
    /// the `Dispatch` must be accessed by the subscriber.
    ///
    /// [`WeakDispatch`]: tracing_core::dispatcher::WeakDispatch
    /// [upgraded]: tracing_core::dispatcher::WeakDispatch::upgrade
    /// [`Subscriber`]: tracing_core::Subscriber
    fn on_register_dispatch(&self, subscriber: &Dispatch) {
        let _ = subscriber;
    }

    /// Performs late initialization when attaching a `Layer` to a
    /// [`Subscriber`].
    ///
    /// This is a callback that is called when the `Layer` is added to a
    /// [`Subscriber`] (e.g. in [`Layer::with_subscriber`] and
    /// [`SubscriberExt::with`]). Since this can only occur before the
    /// [`Subscriber`] has been set as the default, both the `Layer` and
    /// [`Subscriber`] are passed to this method _mutably_. This gives the
    /// `Layer` the opportunity to set any of its own fields with values
    /// received by method calls on the [`Subscriber`].
    ///
    /// For example, [`Filtered`] layers implement `on_layer` to call the
    /// [`Subscriber`]'s [`register_filter`] method, and store the returned
    /// [`FilterId`] as a field.
    ///
    /// **Note** In most cases, `Layer` implementations will not need to
    /// implement this method. However, in cases where a type implementing
    /// `Layer` wraps one or more other types that implement `Layer`, like the
    /// [`Layered`] and [`Filtered`] types in this crate, that type MUST ensure
    /// that the inner `Layer`s' `on_layer` methods are called. Otherwise,
    /// functionality that relies on `on_layer`, such as [per-layer filtering],
    /// may not work correctly.
    ///
    /// [`Filtered`]: crate::filter::Filtered
    /// [`register_filter`]: crate::registry::LookupSpan::register_filter
    /// [per-layer filtering]: #per-layer-filtering
    /// [`FilterId`]: crate::filter::FilterId
    fn on_layer(&mut self, subscriber: &mut S) {
        let _ = subscriber;
    }

    /// Registers a new callsite with this layer, returning whether or not
    /// the layer is interested in being notified about the callsite, similarly
    /// to [`Subscriber::register_callsite`].
    ///
    /// By default, this returns [`Interest::always()`] if [`self.enabled`] returns
    /// true, or [`Interest::never()`] if it returns false.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    /// <strong>Note</strong>: This method (and <a href="#method.enabled">
    /// <code>Layer::enabled</code></a>) determine whether a span or event is
    /// globally enabled, <em>not</em> whether the individual layer will be
    /// notified about that span or event. This is intended to be used
    /// by layers that implement filtering for the entire stack. Layers which do
    /// not wish to be notified about certain spans or events but do not wish to
    /// globally disable them should ignore those spans or events in their
    /// <a href="#method.on_event"><code>on_event</code></a>,
    /// <a href="#method.on_enter"><code>on_enter</code></a>,
    /// <a href="#method.on_exit"><code>on_exit</code></a>, and other notification
    /// methods.
    /// </pre>
    ///
    /// See [the trait-level documentation] for more information on filtering
    /// with `Layer`s.
    ///
    /// Layers may also implement this method to perform any behaviour that
    /// should be run once per callsite. If the layer wishes to use
    /// `register_callsite` for per-callsite behaviour, but does not want to
    /// globally enable or disable those callsites, it should always return
    /// [`Interest::always()`].
    ///
    /// [`Interest`]: tracing_core::Interest
    /// [`Subscriber::register_callsite`]: tracing_core::Subscriber::register_callsite()
    /// [`Interest::never()`]: tracing_core::subscriber::Interest::never()
    /// [`Interest::always()`]: tracing_core::subscriber::Interest::always()
    /// [`self.enabled`]: Layer::enabled()
    /// [`Layer::enabled`]: Layer::enabled()
    /// [`on_event`]: Layer::on_event()
    /// [`on_enter`]: Layer::on_enter()
    /// [`on_exit`]: Layer::on_exit()
    /// [the trait-level documentation]: #filtering-with-layers
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.enabled(metadata, Context::none()) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    /// Returns `true` if this layer is interested in a span or event with the
    /// given `metadata` in the current [`Context`], similarly to
    /// [`Subscriber::enabled`].
    ///
    /// By default, this always returns `true`, allowing the wrapped subscriber
    /// to choose to disable the span.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    /// <strong>Note</strong>: This method (and <a href="#method.register_callsite">
    /// <code>Layer::register_callsite</code></a>) determine whether a span or event is
    /// globally enabled, <em>not</em> whether the individual layer will be
    /// notified about that span or event. This is intended to be used
    /// by layers that implement filtering for the entire stack. Layers which do
    /// not wish to be notified about certain spans or events but do not wish to
    /// globally disable them should ignore those spans or events in their
    /// <a href="#method.on_event"><code>on_event</code></a>,
    /// <a href="#method.on_enter"><code>on_enter</code></a>,
    /// <a href="#method.on_exit"><code>on_exit</code></a>, and other notification
    /// methods.
    /// </pre>
    ///
    ///
    /// See [the trait-level documentation] for more information on filtering
    /// with `Layer`s.
    ///
    /// [`Interest`]: tracing_core::Interest
    /// [`Subscriber::enabled`]: tracing_core::Subscriber::enabled()
    /// [`Layer::register_callsite`]: Layer::register_callsite()
    /// [`on_event`]: Layer::on_event()
    /// [`on_enter`]: Layer::on_enter()
    /// [`on_exit`]: Layer::on_exit()
    /// [the trait-level documentation]: #filtering-with-layers
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
        let _ = (metadata, ctx);
        true
    }

    /// Notifies this layer that a new span was constructed with the given
    /// `Attributes` and `Id`.
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let _ = (attrs, id, ctx);
    }

    // TODO(eliza): do we want this to be a public API? If we end up moving
    // filtering layers to a separate trait, we may no longer want `Layer`s to
    // be able to participate in max level hinting...
    #[doc(hidden)]
    fn max_level_hint(&self) -> Option<LevelFilter> {
        None
    }

    /// Notifies this layer that a span with the given `Id` recorded the given
    /// `values`.
    // Note: it's unclear to me why we'd need the current span in `record` (the
    // only thing the `Context` type currently provides), but passing it in anyway
    // seems like a good future-proofing measure as it may grow other methods later...
    fn on_record(&self, _span: &span::Id, _values: &span::Record<'_>, _ctx: Context<'_, S>) {}

    /// Notifies this layer that a span with the ID `span` recorded that it
    /// follows from the span with the ID `follows`.
    // Note: it's unclear to me why we'd need the current span in `record` (the
    // only thing the `Context` type currently provides), but passing it in anyway
    // seems like a good future-proofing measure as it may grow other methods later...
    fn on_follows_from(&self, _span: &span::Id, _follows: &span::Id, _ctx: Context<'_, S>) {}

    /// Called before [`on_event`], to determine if `on_event` should be called.
    ///
    /// <div class="example-wrap" style="display:inline-block">
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///
    /// **Note**: This method determines whether an event is globally enabled,
    /// *not* whether the individual `Layer` will be notified about the
    /// event. This is intended to be used by `Layer`s that implement
    /// filtering for the entire stack. `Layer`s which do not wish to be
    /// notified about certain events but do not wish to globally disable them
    /// should ignore those events in their [on_event][Self::on_event].
    ///
    /// </pre></div>
    ///
    /// See [the trait-level documentation] for more information on filtering
    /// with `Layer`s.
    ///
    /// [`on_event`]: Self::on_event
    /// [`Interest`]: tracing_core::Interest
    /// [the trait-level documentation]: #filtering-with-layers
    #[inline] // collapse this to a constant please mrs optimizer
    fn event_enabled(&self, _event: &Event<'_>, _ctx: Context<'_, S>) -> bool {
        true
    }

    /// Notifies this layer that an event has occurred.
    fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, S>) {}

    /// Notifies this layer that a span with the given ID was entered.
    fn on_enter(&self, _id: &span::Id, _ctx: Context<'_, S>) {}

    /// Notifies this layer that the span with the given ID was exited.
    fn on_exit(&self, _id: &span::Id, _ctx: Context<'_, S>) {}

    /// Notifies this layer that the span with the given ID has been closed.
    fn on_close(&self, _id: span::Id, _ctx: Context<'_, S>) {}

    /// Notifies this layer that a span ID has been cloned, and that the
    /// subscriber returned a different ID.
    fn on_id_change(&self, _old: &span::Id, _new: &span::Id, _ctx: Context<'_, S>) {}

    /// Composes this layer around the given `Layer`, returning a `Layered`
    /// struct implementing `Layer`.
    ///
    /// The returned `Layer` will call the methods on this `Layer` and then
    /// those of the new `Layer`, before calling the methods on the subscriber
    /// it wraps. For example:
    ///
    /// ```rust
    /// # use tracing_subscriber::layer::Layer;
    /// # use tracing_core::Subscriber;
    /// pub struct FooLayer {
    ///     // ...
    /// }
    ///
    /// pub struct BarLayer {
    ///     // ...
    /// }
    ///
    /// pub struct MySubscriber {
    ///     // ...
    /// }
    ///
    /// impl<S: Subscriber> Layer<S> for FooLayer {
    ///     // ...
    /// }
    ///
    /// impl<S: Subscriber> Layer<S> for BarLayer {
    ///     // ...
    /// }
    ///
    /// # impl FooLayer {
    /// # fn new() -> Self { Self {} }
    /// # }
    /// # impl BarLayer {
    /// # fn new() -> Self { Self { }}
    /// # }
    /// # impl MySubscriber {
    /// # fn new() -> Self { Self { }}
    /// # }
    /// # use tracing_core::{span::{Id, Attributes, Record}, Metadata, Event};
    /// # impl tracing_core::Subscriber for MySubscriber {
    /// #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(1) }
    /// #   fn record(&self, _: &Id, _: &Record) {}
    /// #   fn event(&self, _: &Event) {}
    /// #   fn record_follows_from(&self, _: &Id, _: &Id) {}
    /// #   fn enabled(&self, _: &Metadata) -> bool { false }
    /// #   fn enter(&self, _: &Id) {}
    /// #   fn exit(&self, _: &Id) {}
    /// # }
    /// let subscriber = FooLayer::new()
    ///     .and_then(BarLayer::new())
    ///     .with_subscriber(MySubscriber::new());
    /// ```
    ///
    /// Multiple layers may be composed in this manner:
    ///
    /// ```rust
    /// # use tracing_subscriber::layer::Layer;
    /// # use tracing_core::Subscriber;
    /// # pub struct FooLayer {}
    /// # pub struct BarLayer {}
    /// # pub struct MySubscriber {}
    /// # impl<S: Subscriber> Layer<S> for FooLayer {}
    /// # impl<S: Subscriber> Layer<S> for BarLayer {}
    /// # impl FooLayer {
    /// # fn new() -> Self { Self {} }
    /// # }
    /// # impl BarLayer {
    /// # fn new() -> Self { Self { }}
    /// # }
    /// # impl MySubscriber {
    /// # fn new() -> Self { Self { }}
    /// # }
    /// # use tracing_core::{span::{Id, Attributes, Record}, Metadata, Event};
    /// # impl tracing_core::Subscriber for MySubscriber {
    /// #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(1) }
    /// #   fn record(&self, _: &Id, _: &Record) {}
    /// #   fn event(&self, _: &Event) {}
    /// #   fn record_follows_from(&self, _: &Id, _: &Id) {}
    /// #   fn enabled(&self, _: &Metadata) -> bool { false }
    /// #   fn enter(&self, _: &Id) {}
    /// #   fn exit(&self, _: &Id) {}
    /// # }
    /// pub struct BazLayer {
    ///     // ...
    /// }
    ///
    /// impl<S: Subscriber> Layer<S> for BazLayer {
    ///     // ...
    /// }
    /// # impl BazLayer { fn new() -> Self { BazLayer {} } }
    ///
    /// let subscriber = FooLayer::new()
    ///     .and_then(BarLayer::new())
    ///     .and_then(BazLayer::new())
    ///     .with_subscriber(MySubscriber::new());
    /// ```
    fn and_then<L>(self, layer: L) -> Layered<L, Self, S>
    where
        L: Layer<S>,
        Self: Sized,
    {
        let inner_has_layer_filter = filter::layer_has_plf(&self);
        Layered::new(layer, self, inner_has_layer_filter)
    }

    /// Composes this `Layer` with the given [`Subscriber`], returning a
    /// `Layered` struct that implements [`Subscriber`].
    ///
    /// The returned `Layered` subscriber will call the methods on this `Layer`
    /// and then those of the wrapped subscriber.
    ///
    /// For example:
    /// ```rust
    /// # use tracing_subscriber::layer::Layer;
    /// # use tracing_core::Subscriber;
    /// pub struct FooLayer {
    ///     // ...
    /// }
    ///
    /// pub struct MySubscriber {
    ///     // ...
    /// }
    ///
    /// impl<S: Subscriber> Layer<S> for FooLayer {
    ///     // ...
    /// }
    ///
    /// # impl FooLayer {
    /// # fn new() -> Self { Self {} }
    /// # }
    /// # impl MySubscriber {
    /// # fn new() -> Self { Self { }}
    /// # }
    /// # use tracing_core::{span::{Id, Attributes, Record}, Metadata};
    /// # impl tracing_core::Subscriber for MySubscriber {
    /// #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
    /// #   fn record(&self, _: &Id, _: &Record) {}
    /// #   fn event(&self, _: &tracing_core::Event) {}
    /// #   fn record_follows_from(&self, _: &Id, _: &Id) {}
    /// #   fn enabled(&self, _: &Metadata) -> bool { false }
    /// #   fn enter(&self, _: &Id) {}
    /// #   fn exit(&self, _: &Id) {}
    /// # }
    /// let subscriber = FooLayer::new()
    ///     .with_subscriber(MySubscriber::new());
    ///```
    ///
    /// [`Subscriber`]: tracing_core::Subscriber
    fn with_subscriber(mut self, mut inner: S) -> Layered<Self, S>
    where
        Self: Sized,
    {
        let inner_has_layer_filter = filter::subscriber_has_plf(&inner);
        self.on_layer(&mut inner);
        Layered::new(self, inner, inner_has_layer_filter)
    }

    /// Combines `self` with a [`Filter`], returning a [`Filtered`] layer.
    ///
    /// The [`Filter`] will control which spans and events are enabled for
    /// this layer. See [the trait-level documentation][plf] for details on
    /// per-layer filtering.
    ///
    /// [`Filtered`]: crate::filter::Filtered
    /// [plf]: crate::layer#per-layer-filtering
    #[cfg(all(feature = "registry", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "registry", feature = "std"))))]
    fn with_filter<F>(self, filter: F) -> filter::Filtered<Self, F, S>
    where
        Self: Sized,
        F: Filter<S>,
    {
        filter::Filtered::new(self, filter)
    }

    /// Erases the type of this [`Layer`], returning a [`Box`]ed `dyn
    /// Layer` trait object.
    ///
    /// This can be used when a function returns a `Layer` which may be of
    /// one of several types, or when a `Layer` subscriber has a very long type
    /// signature.
    ///
    /// # Examples
    ///
    /// The following example will *not* compile, because the value assigned to
    /// `log_layer` may have one of several different types:
    ///
    /// ```compile_fail
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use tracing_subscriber::{Layer, filter::LevelFilter, prelude::*};
    /// use std::{path::PathBuf, fs::File, io};
    ///
    /// /// Configures whether logs are emitted to a file, to stdout, or to stderr.
    /// pub enum LogConfig {
    ///     File(PathBuf),
    ///     Stdout,
    ///     Stderr,
    /// }
    ///
    /// let config = // ...
    ///     # LogConfig::Stdout;
    ///
    /// // Depending on the config, construct a layer of one of several types.
    /// let log_layer = match config {
    ///     // If logging to a file, use a maximally-verbose configuration.
    ///     LogConfig::File(path) => {
    ///         let file = File::create(path)?;
    ///         tracing_subscriber::fmt::layer()
    ///             .with_thread_ids(true)
    ///             .with_thread_names(true)
    ///             // Selecting the JSON logging format changes the layer's
    ///             // type.
    ///             .json()
    ///             .with_span_list(true)
    ///             // Setting the writer to use our log file changes the
    ///             // layer's type again.
    ///             .with_writer(file)
    ///     },
    ///
    ///     // If logging to stdout, use a pretty, human-readable configuration.
    ///     LogConfig::Stdout => tracing_subscriber::fmt::layer()
    ///         // Selecting the "pretty" logging format changes the
    ///         // layer's type!
    ///         .pretty()
    ///         .with_writer(io::stdout)
    ///         // Add a filter based on the RUST_LOG environment variable;
    ///         // this changes the type too!
    ///         .and_then(tracing_subscriber::EnvFilter::from_default_env()),
    ///
    ///     // If logging to stdout, only log errors and warnings.
    ///     LogConfig::Stderr => tracing_subscriber::fmt::layer()
    ///         // Changing the writer changes the layer's type
    ///         .with_writer(io::stderr)
    ///         // Only log the `WARN` and `ERROR` levels. Adding a filter
    ///         // changes the layer's type to `Filtered<LevelFilter, ...>`.
    ///         .with_filter(LevelFilter::WARN),
    /// };
    ///
    /// tracing_subscriber::registry()
    ///     .with(log_layer)
    ///     .init();
    /// # Ok(()) }
    /// ```
    ///
    /// However, adding a call to `.boxed()` after each match arm erases the
    /// layer's type, so this code *does* compile:
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use tracing_subscriber::{Layer, filter::LevelFilter, prelude::*};
    /// # use std::{path::PathBuf, fs::File, io};
    /// # pub enum LogConfig {
    /// #    File(PathBuf),
    /// #    Stdout,
    /// #    Stderr,
    /// # }
    /// # let config = LogConfig::Stdout;
    /// let log_layer = match config {
    ///     LogConfig::File(path) => {
    ///         let file = File::create(path)?;
    ///         tracing_subscriber::fmt::layer()
    ///             .with_thread_ids(true)
    ///             .with_thread_names(true)
    ///             .json()
    ///             .with_span_list(true)
    ///             .with_writer(file)
    ///             // Erase the type by boxing the layer
    ///             .boxed()
    ///     },
    ///
    ///     LogConfig::Stdout => tracing_subscriber::fmt::layer()
    ///         .pretty()
    ///         .with_writer(io::stdout)
    ///         .and_then(tracing_subscriber::EnvFilter::from_default_env())
    ///         // Erase the type by boxing the layer
    ///         .boxed(),
    ///
    ///     LogConfig::Stderr => tracing_subscriber::fmt::layer()
    ///         .with_writer(io::stderr)
    ///         .with_filter(LevelFilter::WARN)
    ///         // Erase the type by boxing the layer
    ///         .boxed(),
    /// };
    ///
    /// tracing_subscriber::registry()
    ///     .with(log_layer)
    ///     .init();
    /// # Ok(()) }
    /// ```
    #[cfg(any(feature = "alloc", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "alloc", feature = "std"))))]
    fn boxed(self) -> Box<dyn Layer<S> + Send + Sync + 'static>
    where
        Self: Sized,
        Self: Layer<S> + Send + Sync + 'static,
        S: Subscriber,
    {
        Box::new(self)
    }

    #[doc(hidden)]
    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        if id == TypeId::of::<Self>() {
            Some(self as *const _ as *const ())
        } else {
            None
        }
    }
}

feature! {
    #![all(feature = "registry", feature = "std")]

    /// A per-[`Layer`] filter that determines whether a span or event is enabled
    /// for an individual layer.
    ///
    /// See [the module-level documentation][plf] for details on using [`Filter`]s.
    ///
    /// [plf]: crate::layer#per-layer-filtering
    #[cfg_attr(docsrs, doc(notable_trait))]
    pub trait Filter<S> {
        /// Returns `true` if this layer is interested in a span or event with the
        /// given [`Metadata`] in the current [`Context`], similarly to
        /// [`Subscriber::enabled`].
        ///
        /// If this returns `false`, the span or event will be disabled _for the
        /// wrapped [`Layer`]_. Unlike [`Layer::enabled`], the span or event will
        /// still be recorded if any _other_ layers choose to enable it. However,
        /// the layer [filtered] by this filter will skip recording that span or
        /// event.
        ///
        /// If all layers indicate that they do not wish to see this span or event,
        /// it will be disabled.
        ///
        /// [`metadata`]: tracing_core::Metadata
        /// [`Subscriber::enabled`]: tracing_core::Subscriber::enabled
        /// [filtered]: crate::filter::Filtered
        fn enabled(&self, meta: &Metadata<'_>, cx: &Context<'_, S>) -> bool;

        /// Returns an [`Interest`] indicating whether this layer will [always],
        /// [sometimes], or [never] be interested in the given [`Metadata`].
        ///
        /// When a given callsite will [always] or [never] be enabled, the results
        /// of evaluating the filter may be cached for improved performance.
        /// Therefore, if a filter is capable of determining that it will always or
        /// never enable a particular callsite, providing an implementation of this
        /// function is recommended.
        ///
        /// <pre class="ignore" style="white-space:normal;font:inherit;">
        /// <strong>Note</strong>: If a <code>Filter</code> will perform
        /// <em>dynamic filtering</em> that depends on the current context in which
        /// a span or event was observed (e.g. only enabling an event when it
        /// occurs within a particular span), it <strong>must</strong> return
        /// <code>Interest::sometimes()</code> from this method. If it returns
        /// <code>Interest::always()</code> or <code>Interest::never()</code>, the
        /// <code>enabled</code> method may not be called when a particular instance
        /// of that span or event is recorded.
        /// </pre>
        ///
        /// This method is broadly similar to [`Subscriber::register_callsite`];
        /// however, since the returned value represents only the interest of
        /// *this* layer, the resulting behavior is somewhat different.
        ///
        /// If a [`Subscriber`] returns [`Interest::always()`][always] or
        /// [`Interest::never()`][never] for a given [`Metadata`], its [`enabled`]
        /// method is then *guaranteed* to never be called for that callsite. On the
        /// other hand, when a `Filter` returns [`Interest::always()`][always] or
        /// [`Interest::never()`][never] for a callsite, _other_ [`Layer`]s may have
        /// differing interests in that callsite. If this is the case, the callsite
        /// will receive [`Interest::sometimes()`][sometimes], and the [`enabled`]
        /// method will still be called for that callsite when it records a span or
        /// event.
        ///
        /// Returning [`Interest::always()`][always] or [`Interest::never()`][never] from
        /// `Filter::callsite_enabled` will permanently enable or disable a
        /// callsite (without requiring subsequent calls to [`enabled`]) if and only
        /// if the following is true:
        ///
        /// - all [`Layer`]s that comprise the subscriber include `Filter`s
        ///   (this includes a tree of [`Layered`] layers that share the same
        ///   `Filter`)
        /// - all those `Filter`s return the same [`Interest`].
        ///
        /// For example, if a [`Subscriber`] consists of two [`Filtered`] layers,
        /// and both of those layers return [`Interest::never()`][never], that
        /// callsite *will* never be enabled, and the [`enabled`] methods of those
        /// [`Filter`]s will not be called.
        ///
        /// ## Default Implementation
        ///
        /// The default implementation of this method assumes that the
        /// `Filter`'s [`enabled`] method _may_ perform dynamic filtering, and
        /// returns [`Interest::sometimes()`][sometimes], to ensure that [`enabled`]
        /// is called to determine whether a particular _instance_ of the callsite
        /// is enabled in the current context. If this is *not* the case, and the
        /// `Filter`'s [`enabled`] method will always return the same result
        /// for a particular [`Metadata`], this method can be overridden as
        /// follows:
        ///
        /// ```
        /// use tracing_subscriber::layer;
        /// use tracing_core::{Metadata, subscriber::Interest};
        ///
        /// struct MyFilter {
        ///     // ...
        /// }
        ///
        /// impl MyFilter {
        ///     // The actual logic for determining whether a `Metadata` is enabled
        ///     // must be factored out from the `enabled` method, so that it can be
        ///     // called without a `Context` (which is not provided to the
        ///     // `callsite_enabled` method).
        ///     fn is_enabled(&self, metadata: &Metadata<'_>) -> bool {
        ///         // ...
        ///         # drop(metadata); true
        ///     }
        /// }
        ///
        /// impl<S> layer::Filter<S> for MyFilter {
        ///     fn enabled(&self, metadata: &Metadata<'_>, _: &layer::Context<'_, S>) -> bool {
        ///         // Even though we are implementing `callsite_enabled`, we must still provide a
        ///         // working implementation of `enabled`, as returning `Interest::always()` or
        ///         // `Interest::never()` will *allow* caching, but will not *guarantee* it.
        ///         // Other filters may still return `Interest::sometimes()`, so we may be
        ///         // asked again in `enabled`.
        ///         self.is_enabled(metadata)
        ///     }
        ///
        ///     fn callsite_enabled(&self, metadata: &'static Metadata<'static>) -> Interest {
        ///         // The result of `self.enabled(metadata, ...)` will always be
        ///         // the same for any given `Metadata`, so we can convert it into
        ///         // an `Interest`:
        ///         if self.is_enabled(metadata) {
        ///             Interest::always()
        ///         } else {
        ///             Interest::never()
        ///         }
        ///     }
        /// }
        /// ```
        ///
        /// [`Metadata`]: tracing_core::Metadata
        /// [`Interest`]: tracing_core::Interest
        /// [always]: tracing_core::Interest::always
        /// [sometimes]: tracing_core::Interest::sometimes
        /// [never]: tracing_core::Interest::never
        /// [`Subscriber::register_callsite`]: tracing_core::Subscriber::register_callsite
        /// [`Subscriber`]: tracing_core::Subscriber
        /// [`enabled`]: Filter::enabled
        /// [`Filtered`]: crate::filter::Filtered
        fn callsite_enabled(&self, meta: &'static Metadata<'static>) -> Interest {
            let _ = meta;
            Interest::sometimes()
        }

        /// Called before the filtered [`Layer]'s [`on_event`], to determine if
        /// `on_event` should be called.
        ///
        /// This gives a chance to filter events based on their fields. Note,
        /// however, that this *does not* override [`enabled`], and is not even
        /// called if [`enabled`] returns `false`.
        ///
        /// ## Default Implementation
        ///
        /// By default, this method returns `true`, indicating that no events are
        /// filtered out based on their fields.
        ///
        /// [`enabled`]: crate::layer::Filter::enabled
        /// [`on_event`]: crate::layer::Layer::on_event
        #[inline] // collapse this to a constant please mrs optimizer
        fn event_enabled(&self, event: &Event<'_>, cx: &Context<'_, S>) -> bool {
            let _ = (event, cx);
            true
        }

        /// Returns an optional hint of the highest [verbosity level][level] that
        /// this `Filter` will enable.
        ///
        /// If this method returns a [`LevelFilter`], it will be used as a hint to
        /// determine the most verbose level that will be enabled. This will allow
        /// spans and events which are more verbose than that level to be skipped
        /// more efficiently. An implementation of this method is optional, but
        /// strongly encouraged.
        ///
        /// If the maximum level the `Filter` will enable can change over the
        /// course of its lifetime, it is free to return a different value from
        /// multiple invocations of this method. However, note that changes in the
        /// maximum level will **only** be reflected after the callsite [`Interest`]
        /// cache is rebuilt, by calling the
        /// [`tracing_core::callsite::rebuild_interest_cache`][rebuild] function.
        /// Therefore, if the `Filter will change the value returned by this
        /// method, it is responsible for ensuring that
        /// [`rebuild_interest_cache`][rebuild] is called after the value of the max
        /// level changes.
        ///
        /// ## Default Implementation
        ///
        /// By default, this method returns `None`, indicating that the maximum
        /// level is unknown.
        ///
        /// [level]: tracing_core::metadata::Level
        /// [`LevelFilter`]: crate::filter::LevelFilter
        /// [`Interest`]: tracing_core::subscriber::Interest
        /// [rebuild]: tracing_core::callsite::rebuild_interest_cache
        fn max_level_hint(&self) -> Option<LevelFilter> {
            None
        }

        /// Notifies this filter that a new span was constructed with the given
        /// `Attributes` and `Id`.
        ///
        /// By default, this method does nothing. `Filter` implementations that
        /// need to be notified when new spans are created can override this
        /// method.
        fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
            let _ = (attrs, id, ctx);
        }


        /// Notifies this filter that a span with the given `Id` recorded the given
        /// `values`.
        ///
        /// By default, this method does nothing. `Filter` implementations that
        /// need to be notified when new spans are created can override this
        /// method.
        fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
            let _ = (id, values, ctx);
        }

        /// Notifies this filter that a span with the given ID was entered.
        ///
        /// By default, this method does nothing. `Filter` implementations that
        /// need to be notified when a span is entered can override this method.
        fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
            let _ = (id, ctx);
        }

        /// Notifies this filter that a span with the given ID was exited.
        ///
        /// By default, this method does nothing. `Filter` implementations that
        /// need to be notified when a span is exited can override this method.
        fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
            let _ = (id, ctx);
        }

        /// Notifies this filter that a span with the given ID has been closed.
        ///
        /// By default, this method does nothing. `Filter` implementations that
        /// need to be notified when a span is closed can override this method.
        fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
            let _ = (id, ctx);
        }
    }
}

/// Extension trait adding a `with(Layer)` combinator to `Subscriber`s.
pub trait SubscriberExt: Subscriber + crate::sealed::Sealed {
    /// Wraps `self` with the provided `layer`.
    fn with<L>(self, layer: L) -> Layered<L, Self>
    where
        L: Layer<Self>,
        Self: Sized,
    {
        layer.with_subscriber(self)
    }
}

/// A layer that does nothing.
#[derive(Clone, Debug, Default)]
pub struct Identity {
    _p: (),
}

// === impl Layer ===

#[derive(Clone, Copy)]
pub(crate) struct NoneLayerMarker(());
static NONE_LAYER_MARKER: NoneLayerMarker = NoneLayerMarker(());

/// Is a type implementing `Layer` `Option::<_>::None`?
pub(crate) fn layer_is_none<L, S>(layer: &L) -> bool
where
    L: Layer<S>,
    S: Subscriber,
{
    unsafe {
        // Safety: we're not actually *doing* anything with this pointer ---
        // this only care about the `Option`, which is essentially being used
        // as a bool. We can rely on the pointer being valid, because it is
        // a crate-private type, and is only returned by the `Layer` impl
        // for `Option`s. However, even if the layer *does* decide to be
        // evil and give us an invalid pointer here, that's fine, because we'll
        // never actually dereference it.
        layer.downcast_raw(TypeId::of::<NoneLayerMarker>())
    }
    .is_some()
}

/// Is a type implementing `Subscriber` `Option::<_>::None`?
pub(crate) fn subscriber_is_none<S>(subscriber: &S) -> bool
where
    S: Subscriber,
{
    unsafe {
        // Safety: we're not actually *doing* anything with this pointer ---
        // this only care about the `Option`, which is essentially being used
        // as a bool. We can rely on the pointer being valid, because it is
        // a crate-private type, and is only returned by the `Layer` impl
        // for `Option`s. However, even if the subscriber *does* decide to be
        // evil and give us an invalid pointer here, that's fine, because we'll
        // never actually dereference it.
        subscriber.downcast_raw(TypeId::of::<NoneLayerMarker>())
    }
    .is_some()
}

impl<L, S> Layer<S> for Option<L>
where
    L: Layer<S>,
    S: Subscriber,
{
    fn on_layer(&mut self, subscriber: &mut S) {
        if let Some(ref mut layer) = self {
            layer.on_layer(subscriber)
        }
    }

    #[inline]
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_new_span(attrs, id, ctx)
        }
    }

    #[inline]
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        match self {
            Some(ref inner) => inner.register_callsite(metadata),
            None => Interest::always(),
        }
    }

    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
        match self {
            Some(ref inner) => inner.enabled(metadata, ctx),
            None => true,
        }
    }

    #[inline]
    fn max_level_hint(&self) -> Option<LevelFilter> {
        match self {
            Some(ref inner) => inner.max_level_hint(),
            None => {
                // There is no inner layer, so this layer will
                // never enable anything.
                Some(LevelFilter::OFF)
            }
        }
    }

    #[inline]
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_record(span, values, ctx);
        }
    }

    #[inline]
    fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_follows_from(span, follows, ctx);
        }
    }

    #[inline]
    fn event_enabled(&self, event: &Event<'_>, ctx: Context<'_, S>) -> bool {
        match self {
            Some(ref inner) => inner.event_enabled(event, ctx),
            None => true,
        }
    }

    #[inline]
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_event(event, ctx);
        }
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_enter(id, ctx);
        }
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_exit(id, ctx);
        }
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_close(id, ctx);
        }
    }

    #[inline]
    fn on_id_change(&self, old: &span::Id, new: &span::Id, ctx: Context<'_, S>) {
        if let Some(ref inner) = self {
            inner.on_id_change(old, new, ctx)
        }
    }

    #[doc(hidden)]
    #[inline]
    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        if id == TypeId::of::<Self>() {
            Some(self as *const _ as *const ())
        } else if id == TypeId::of::<NoneLayerMarker>() && self.is_none() {
            Some(&NONE_LAYER_MARKER as *const _ as *const ())
        } else {
            self.as_ref().and_then(|inner| inner.downcast_raw(id))
        }
    }
}

feature! {
    #![any(feature = "std", feature = "alloc")]
    #[cfg(not(feature = "std"))]
    use alloc::vec::Vec;

    macro_rules! layer_impl_body {
        () => {
            #[inline]
            fn on_register_dispatch(&self, subscriber: &Dispatch) {
                self.deref().on_register_dispatch(subscriber);
            }

            #[inline]
            fn on_layer(&mut self, subscriber: &mut S) {
                self.deref_mut().on_layer(subscriber);
            }

            #[inline]
            fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
                self.deref().on_new_span(attrs, id, ctx)
            }

            #[inline]
            fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
                self.deref().register_callsite(metadata)
            }

            #[inline]
            fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
                self.deref().enabled(metadata, ctx)
            }

            #[inline]
            fn max_level_hint(&self) -> Option<LevelFilter> {
                self.deref().max_level_hint()
            }

            #[inline]
            fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
                self.deref().on_record(span, values, ctx)
            }

            #[inline]
            fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: Context<'_, S>) {
                self.deref().on_follows_from(span, follows, ctx)
            }

            #[inline]
            fn event_enabled(&self, event: &Event<'_>, ctx: Context<'_, S>) -> bool {
                self.deref().event_enabled(event, ctx)
            }

            #[inline]
            fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
                self.deref().on_event(event, ctx)
            }

            #[inline]
            fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
                self.deref().on_enter(id, ctx)
            }

            #[inline]
            fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
                self.deref().on_exit(id, ctx)
            }

            #[inline]
            fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
                self.deref().on_close(id, ctx)
            }

            #[inline]
            fn on_id_change(&self, old: &span::Id, new: &span::Id, ctx: Context<'_, S>) {
                self.deref().on_id_change(old, new, ctx)
            }

            #[doc(hidden)]
            #[inline]
            unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
                self.deref().downcast_raw(id)
            }
        };
    }

    impl<L, S> Layer<S> for Box<L>
    where
        L: Layer<S>,
        S: Subscriber,
    {
        layer_impl_body! {}
    }

    impl<S> Layer<S> for Box<dyn Layer<S> + Send + Sync>
    where
        S: Subscriber,
    {
        layer_impl_body! {}
    }



    impl<S, L> Layer<S> for Vec<L>
    where
        L: Layer<S>,
        S: Subscriber,
    {

        fn on_layer(&mut self, subscriber: &mut S) {
            for l in self {
                l.on_layer(subscriber);
            }
        }

        fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
            // Return highest level of interest.
            let mut interest = Interest::never();
            for l in self {
                let new_interest = l.register_callsite(metadata);
                if (interest.is_sometimes() && new_interest.is_always())
                    || (interest.is_never() && !new_interest.is_never())
                {
                    interest = new_interest;
                }
            }

            interest
        }

        fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
            self.iter().all(|l| l.enabled(metadata, ctx.clone()))
        }

        fn event_enabled(&self, event: &Event<'_>, ctx: Context<'_, S>) -> bool {
            self.iter().all(|l| l.event_enabled(event, ctx.clone()))
        }

        fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
            for l in self {
                l.on_new_span(attrs, id, ctx.clone());
            }
        }

        fn max_level_hint(&self) -> Option<LevelFilter> {
            // Default to `OFF` if there are no inner layers.
            let mut max_level = LevelFilter::OFF;
            for l in self {
                // NOTE(eliza): this is slightly subtle: if *any* layer
                // returns `None`, we have to return `None`, assuming there is
                // no max level hint, since that particular layer cannot
                // provide a hint.
                let hint = l.max_level_hint()?;
                max_level = core::cmp::max(hint, max_level);
            }
            Some(max_level)
        }

        fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
            for l in self {
                l.on_record(span, values, ctx.clone())
            }
        }

        fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: Context<'_, S>) {
            for l in self {
                l.on_follows_from(span, follows, ctx.clone());
            }
        }

        fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
            for l in self {
                l.on_event(event, ctx.clone());
            }
        }

        fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
            for l in self {
                l.on_enter(id, ctx.clone());
            }
        }

        fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
            for l in self {
                l.on_exit(id, ctx.clone());
            }
        }

        fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
            for l in self {
                l.on_close(id.clone(), ctx.clone());
            }
        }

        #[doc(hidden)]
        unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
            // If downcasting to `Self`, return a pointer to `self`.
            if id == TypeId::of::<Self>() {
                return Some(self as *const _ as *const ());
            }

            // Someone is looking for per-layer filters. But, this `Vec`
            // might contain layers with per-layer filters *and*
            // layers without filters. It should only be treated as a
            // per-layer-filtered layer if *all* its layers have
            // per-layer filters.
            // XXX(eliza): it's a bummer we have to do this linear search every
            // time. It would be nice if this could be cached, but that would
            // require replacing the `Vec` impl with an impl for a newtype...
            if filter::is_plf_downcast_marker(id) && self.iter().any(|s| s.downcast_raw(id).is_none()) {
                return None;
            }

            // Otherwise, return the first child of `self` that downcaasts to
            // the selected type, if any.
            // XXX(eliza): hope this is reasonable lol
            self.iter().find_map(|l| l.downcast_raw(id))
        }
    }
}

// === impl SubscriberExt ===

impl<S: Subscriber> crate::sealed::Sealed for S {}
impl<S: Subscriber> SubscriberExt for S {}

// === impl Identity ===

impl<S: Subscriber> Layer<S> for Identity {}

impl Identity {
    /// Returns a new `Identity` layer.
    pub fn new() -> Self {
        Self { _p: () }
    }
}
