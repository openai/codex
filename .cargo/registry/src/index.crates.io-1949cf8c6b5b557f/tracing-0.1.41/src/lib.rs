//! A scoped, structured logging and diagnostics system.
//!
//! # Overview
//!
//! `tracing` is a framework for instrumenting Rust programs to collect
//! structured, event-based diagnostic information.
//!
//! In asynchronous systems like Tokio, interpreting traditional log messages can
//! often be quite challenging. Since individual tasks are multiplexed on the same
//! thread, associated events and log lines are intermixed making it difficult to
//! trace the logic flow. `tracing` expands upon logging-style diagnostics by
//! allowing libraries and applications to record structured events with additional
//! information about *temporality* and *causality* — unlike a log message, a span
//! in `tracing` has a beginning and end time, may be entered and exited by the
//! flow of execution, and may exist within a nested tree of similar spans. In
//! addition, `tracing` spans are *structured*, with the ability to record typed
//! data as well as textual messages.
//!
//! The `tracing` crate provides the APIs necessary for instrumenting libraries
//! and applications to emit trace data.
//!
//! *Compiler support: [requires `rustc` 1.63+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//! # Core Concepts
//!
//! The core of `tracing`'s API is composed of _spans_, _events_ and
//! _subscribers_. We'll cover these in turn.
//!
//! ## Spans
//!
//! To record the flow of execution through a program, `tracing` introduces the
//! concept of [spans]. Unlike a log line that represents a _moment in
//! time_, a span represents a _period of time_ with a beginning and an end. When a
//! program begins executing in a context or performing a unit of work, it
//! _enters_ that context's span, and when it stops executing in that context,
//! it _exits_ the span. The span in which a thread is currently executing is
//! referred to as that thread's _current_ span.
//!
//! For example:
//! ```
//! use tracing::{span, Level};
//! # fn main() {
//! let span = span!(Level::TRACE, "my_span");
//! // `enter` returns a RAII guard which, when dropped, exits the span. this
//! // indicates that we are in the span for the current lexical scope.
//! let _enter = span.enter();
//! // perform some work in the context of `my_span`...
//! # }
//!```
//!
//! The [`span` module][span]'s documentation provides further details on how to
//! use spans.
//!
//! <div class="example-wrap" style="display:inline-block"><pre class="compile_fail" style="white-space:normal;font:inherit;">
//!
//!  **Warning**: In asynchronous code that uses async/await syntax,
//!  `Span::enter` may produce incorrect traces if the returned drop
//!  guard is held across an await point. See
//!  [the method documentation][Span#in-asynchronous-code] for details.
//!
//! </pre></div>
//!
//! ## Events
//!
//! An [`Event`] represents a _moment_ in time. It signifies something that
//! happened while a trace was being recorded. `Event`s are comparable to the log
//! records emitted by unstructured logging code, but unlike a typical log line,
//! an `Event` may occur within the context of a span.
//!
//! For example:
//! ```
//! use tracing::{event, span, Level};
//!
//! # fn main() {
//! // records an event outside of any span context:
//! event!(Level::INFO, "something happened");
//!
//! let span = span!(Level::INFO, "my_span");
//! let _guard = span.enter();
//!
//! // records an event within "my_span".
//! event!(Level::DEBUG, "something happened inside my_span");
//! # }
//!```
//!
//! In general, events should be used to represent points in time _within_ a
//! span — a request returned with a given status code, _n_ new items were
//! taken from a queue, and so on.
//!
//! The [`Event` struct][`Event`] documentation provides further details on using
//! events.
//!
//! ## Subscribers
//!
//! As `Span`s and `Event`s occur, they are recorded or aggregated by
//! implementations of the [`Subscriber`] trait. `Subscriber`s are notified
//! when an `Event` takes place and when a `Span` is entered or exited. These
//! notifications are represented by the following `Subscriber` trait methods:
//!
//! + [`event`][Subscriber::event], called when an `Event` takes place,
//! + [`enter`], called when execution enters a `Span`,
//! + [`exit`], called when execution exits a `Span`
//!
//! In addition, subscribers may implement the [`enabled`] function to _filter_
//! the notifications they receive based on [metadata] describing each `Span`
//! or `Event`. If a call to `Subscriber::enabled` returns `false` for a given
//! set of metadata, that `Subscriber` will *not* be notified about the
//! corresponding `Span` or `Event`. For performance reasons, if no currently
//! active subscribers express interest in a given set of metadata by returning
//! `true`, then the corresponding `Span` or `Event` will never be constructed.
//!
//! # Usage
//!
//! First, add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tracing = "0.1"
//! ```
//!
//! ## Recording Spans and Events
//!
//! Spans and events are recorded using macros.
//!
//! ### Spans
//!
//! The [`span!`] macro expands to a [`Span` struct][`Span`] which is used to
//! record a span. The [`Span::enter`] method on that struct records that the
//! span has been entered, and returns a [RAII] guard object, which will exit
//! the span when dropped.
//!
//! For example:
//!
//! ```rust
//! use tracing::{span, Level};
//! # fn main() {
//! // Construct a new span named "my span" with trace log level.
//! let span = span!(Level::TRACE, "my span");
//!
//! // Enter the span, returning a guard object.
//! let _enter = span.enter();
//!
//! // Any trace events that occur before the guard is dropped will occur
//! // within the span.
//!
//! // Dropping the guard will exit the span.
//! # }
//! ```
//!
//! The [`#[instrument]`][instrument] attribute provides an easy way to
//! add `tracing` spans to functions. A function annotated with `#[instrument]`
//! will create and enter a span with that function's name every time the
//! function is called, with arguments to that function will be recorded as
//! fields using `fmt::Debug`.
//!
//! For example:
//! ```ignore
//! # // this doctest is ignored because we don't have a way to say
//! # // that it should only be run with cfg(feature = "attributes")
//! use tracing::{Level, event, instrument};
//!
//! #[instrument]
//! pub fn my_function(my_arg: usize) {
//!     // This event will be recorded inside a span named `my_function` with the
//!     // field `my_arg`.
//!     event!(Level::INFO, "inside my_function!");
//!     // ...
//! }
//! # fn main() {}
//! ```
//!
//! For functions which don't have built-in tracing support and can't have
//! the `#[instrument]` attribute applied (such as from an external crate),
//! the [`Span` struct][`Span`] has a [`in_scope()` method][`in_scope`]
//! which can be used to easily wrap synchronous code in a span.
//!
//! For example:
//! ```rust
//! use tracing::info_span;
//!
//! # fn doc() -> Result<(), ()> {
//! # mod serde_json {
//! #    pub(crate) fn from_slice(buf: &[u8]) -> Result<(), ()> { Ok(()) }
//! # }
//! # let buf: [u8; 0] = [];
//! let json = info_span!("json.parse").in_scope(|| serde_json::from_slice(&buf))?;
//! # let _ = json; // suppress unused variable warning
//! # Ok(())
//! # }
//! ```
//!
//! You can find more examples showing how to use this crate [here][examples].
//!
//! [RAII]: https://github.com/rust-unofficial/patterns/blob/main/src/patterns/behavioural/RAII.md
//! [examples]: https://github.com/tokio-rs/tracing/tree/master/examples
//!
//! ### Events
//!
//! [`Event`]s are recorded using the [`event!`] macro:
//!
//! ```rust
//! # fn main() {
//! use tracing::{event, Level};
//! event!(Level::INFO, "something has happened!");
//! # }
//! ```
//!
//! ## Using the Macros
//!
//! The [`span!`] and [`event!`] macros as well as the `#[instrument]` attribute
//! use fairly similar syntax, with some exceptions.
//!
//! ### Configuring Attributes
//!
//! Both macros require a [`Level`] specifying the verbosity of the span or
//! event. Optionally, the, [target] and [parent span] may be overridden. If the
//! target and parent span are not overridden, they will default to the
//! module path where the macro was invoked and the current span (as determined
//! by the subscriber), respectively.
//!
//! For example:
//!
//! ```
//! # use tracing::{span, event, Level};
//! # fn main() {
//! span!(target: "app_spans", Level::TRACE, "my span");
//! event!(target: "app_events", Level::INFO, "something has happened!");
//! # }
//! ```
//! ```
//! # use tracing::{span, event, Level};
//! # fn main() {
//! let span = span!(Level::TRACE, "my span");
//! event!(parent: &span, Level::INFO, "something has happened!");
//! # }
//! ```
//!
//! The span macros also take a string literal after the level, to set the name
//! of the span (as above).  In the case of the event macros, the name of the event can
//! be overridden (the default is `event file:line`) using the `name:` specifier.
//!
//! ```
//! # use tracing::{span, event, Level};
//! # fn main() {
//! span!(Level::TRACE, "my span");
//! event!(name: "some_info", Level::INFO, "something has happened!");
//! # }
//! ```
//!
//! ### Recording Fields
//!
//! Structured fields on spans and events are specified using the syntax
//! `field_name = field_value`. Fields are separated by commas.
//!
//! ```
//! # use tracing::{event, Level};
//! # fn main() {
//! // records an event with two fields:
//! //  - "answer", with the value 42
//! //  - "question", with the value "life, the universe and everything"
//! event!(Level::INFO, answer = 42, question = "life, the universe, and everything");
//! # }
//! ```
//!
//! As shorthand, local variables may be used as field values without an
//! assignment, similar to [struct initializers]. For example:
//!
//! ```
//! # use tracing::{span, Level};
//! # fn main() {
//! let user = "ferris";
//!
//! span!(Level::TRACE, "login", user);
//! // is equivalent to:
//! span!(Level::TRACE, "login", user = user);
//! # }
//!```
//!
//! Field names can include dots, but should not be terminated by them:
//! ```
//! # use tracing::{span, Level};
//! # fn main() {
//! let user = "ferris";
//! let email = "ferris@rust-lang.org";
//! span!(Level::TRACE, "login", user, user.email = email);
//! # }
//!```
//!
//! Since field names can include dots, fields on local structs can be used
//! using the local variable shorthand:
//! ```
//! # use tracing::{span, Level};
//! # fn main() {
//! # struct User {
//! #    name: &'static str,
//! #    email: &'static str,
//! # }
//! let user = User {
//!     name: "ferris",
//!     email: "ferris@rust-lang.org",
//! };
//! // the span will have the fields `user.name = "ferris"` and
//! // `user.email = "ferris@rust-lang.org"`.
//! span!(Level::TRACE, "login", user.name, user.email);
//! # }
//!```
//!
//! Fields with names that are not Rust identifiers, or with names that are Rust reserved words,
//! may be created using quoted string literals. However, this may not be used with the local
//! variable shorthand.
//! ```
//! # use tracing::{span, Level};
//! # fn main() {
//! // records an event with fields whose names are not Rust identifiers
//! //  - "guid:x-request-id", containing a `:`, with the value "abcdef"
//! //  - "type", which is a reserved word, with the value "request"
//! span!(Level::TRACE, "api", "guid:x-request-id" = "abcdef", "type" = "request");
//! # }
//!```
//!
//! Constant expressions can also be used as field names. Constants
//! must be enclosed in curly braces (`{}`) to indicate that the *value*
//! of the constant is to be used as the field name, rather than the
//! constant's name. For example:
//! ```
//! # use tracing::{span, Level};
//! # fn main() {
//! const RESOURCE_NAME: &str = "foo";
//! // this span will have the field `foo = "some_id"`
//! span!(Level::TRACE, "get", { RESOURCE_NAME } = "some_id");
//! # }
//!```
//!
//! The `?` sigil is shorthand that specifies a field should be recorded using
//! its [`fmt::Debug`] implementation:
//! ```
//! # use tracing::{event, Level};
//! # fn main() {
//! #[derive(Debug)]
//! struct MyStruct {
//!     field: &'static str,
//! }
//!
//! let my_struct = MyStruct {
//!     field: "Hello world!"
//! };
//!
//! // `my_struct` will be recorded using its `fmt::Debug` implementation.
//! event!(Level::TRACE, greeting = ?my_struct);
//! // is equivalent to:
//! event!(Level::TRACE, greeting = tracing::field::debug(&my_struct));
//! # }
//! ```
//!
//! The `%` sigil operates similarly, but indicates that the value should be
//! recorded using its [`fmt::Display`] implementation:
//! ```
//! # use tracing::{event, Level};
//! # fn main() {
//! # #[derive(Debug)]
//! # struct MyStruct {
//! #     field: &'static str,
//! # }
//! #
//! # let my_struct = MyStruct {
//! #     field: "Hello world!"
//! # };
//! // `my_struct.field` will be recorded using its `fmt::Display` implementation.
//! event!(Level::TRACE, greeting = %my_struct.field);
//! // is equivalent to:
//! event!(Level::TRACE, greeting = tracing::field::display(&my_struct.field));
//! # }
//! ```
//!
//! The `%` and `?` sigils may also be used with local variable shorthand:
//!
//! ```
//! # use tracing::{event, Level};
//! # fn main() {
//! # #[derive(Debug)]
//! # struct MyStruct {
//! #     field: &'static str,
//! # }
//! #
//! # let my_struct = MyStruct {
//! #     field: "Hello world!"
//! # };
//! // `my_struct.field` will be recorded using its `fmt::Display` implementation.
//! event!(Level::TRACE, %my_struct.field);
//! # }
//! ```
//!
//! Additionally, a span may declare fields with the special value [`Empty`],
//! which indicates that that the value for that field does not currently exist
//! but may be recorded later. For example:
//!
//! ```
//! use tracing::{trace_span, field};
//!
//! // Create a span with two fields: `greeting`, with the value "hello world", and
//! // `parting`, without a value.
//! let span = trace_span!("my_span", greeting = "hello world", parting = field::Empty);
//!
//! // ...
//!
//! // Now, record a value for parting as well.
//! span.record("parting", &"goodbye world!");
//! ```
//!
//! Finally, events may also include human-readable messages, in the form of a
//! [format string][fmt] and (optional) arguments, **after** the event's
//! key-value fields. If a format string and arguments are provided,
//! they will implicitly create a new field named `message` whose value is the
//! provided set of format arguments.
//!
//! For example:
//!
//! ```
//! # use tracing::{event, Level};
//! # fn main() {
//! let question = "the ultimate question of life, the universe, and everything";
//! let answer = 42;
//! // records an event with the following fields:
//! // - `question.answer` with the value 42,
//! // - `question.tricky` with the value `true`,
//! // - "message", with the value "the answer to the ultimate question of life, the
//! //    universe, and everything is 42."
//! event!(
//!     Level::DEBUG,
//!     question.answer = answer,
//!     question.tricky = true,
//!     "the answer to {} is {}.", question, answer
//! );
//! # }
//! ```
//!
//! Specifying a formatted message in this manner does not allocate by default.
//!
//! [struct initializers]: https://doc.rust-lang.org/book/ch05-01-defining-structs.html#using-the-field-init-shorthand-when-variables-and-fields-have-the-same-name
//! [target]: Metadata::target
//! [parent span]: span::Attributes::parent
//! [determined contextually]: span::Attributes::is_contextual
//! [`fmt::Debug`]: std::fmt::Debug
//! [`fmt::Display`]: std::fmt::Display
//! [fmt]: std::fmt#usage
//! [`Empty`]: field::Empty
//!
//! ### Shorthand Macros
//!
//! `tracing` also offers a number of macros with preset verbosity levels.
//! The [`trace!`], [`debug!`], [`info!`], [`warn!`], and [`error!`] behave
//! similarly to the [`event!`] macro, but with the [`Level`] argument already
//! specified, while the corresponding [`trace_span!`], [`debug_span!`],
//! [`info_span!`], [`warn_span!`], and [`error_span!`] macros are the same,
//! but for the [`span!`] macro.
//!
//! These are intended both as a shorthand, and for compatibility with the [`log`]
//! crate (see the next section).
//!
//! [`span!`]: span!
//! [`event!`]: event!
//! [`trace!`]: trace!
//! [`debug!`]: debug!
//! [`info!`]: info!
//! [`warn!`]: warn!
//! [`error!`]: error!
//! [`trace_span!`]: trace_span!
//! [`debug_span!`]: debug_span!
//! [`info_span!`]: info_span!
//! [`warn_span!`]: warn_span!
//! [`error_span!`]: error_span!
//!
//! ### For `log` Users
//!
//! Users of the [`log`] crate should note that `tracing` exposes a set of
//! macros for creating `Event`s (`trace!`, `debug!`, `info!`, `warn!`, and
//! `error!`) which may be invoked with the same syntax as the similarly-named
//! macros from the `log` crate. Often, the process of converting a project to
//! use `tracing` can begin with a simple drop-in replacement.
//!
//! Let's consider the `log` crate's yak-shaving example:
//!
//! ```rust,ignore
//! use std::{error::Error, io};
//! use tracing::{debug, error, info, span, warn, Level};
//!
//! // the `#[tracing::instrument]` attribute creates and enters a span
//! // every time the instrumented function is called. The span is named after the
//! // the function or method. Parameters passed to the function are recorded as fields.
//! #[tracing::instrument]
//! pub fn shave(yak: usize) -> Result<(), Box<dyn Error + 'static>> {
//!     // this creates an event at the DEBUG level with two fields:
//!     // - `excitement`, with the key "excitement" and the value "yay!"
//!     // - `message`, with the key "message" and the value "hello! I'm gonna shave a yak."
//!     //
//!     // unlike other fields, `message`'s shorthand initialization is just the string itself.
//!     debug!(excitement = "yay!", "hello! I'm gonna shave a yak.");
//!     if yak == 3 {
//!         warn!("could not locate yak!");
//!         // note that this is intended to demonstrate `tracing`'s features, not idiomatic
//!         // error handling! in a library or application, you should consider returning
//!         // a dedicated `YakError`. libraries like snafu or thiserror make this easy.
//!         return Err(io::Error::new(io::ErrorKind::Other, "shaving yak failed!").into());
//!     } else {
//!         debug!("yak shaved successfully");
//!     }
//!     Ok(())
//! }
//!
//! pub fn shave_all(yaks: usize) -> usize {
//!     // Constructs a new span named "shaving_yaks" at the TRACE level,
//!     // and a field whose key is "yaks". This is equivalent to writing:
//!     //
//!     // let span = span!(Level::TRACE, "shaving_yaks", yaks = yaks);
//!     //
//!     // local variables (`yaks`) can be used as field values
//!     // without an assignment, similar to struct initializers.
//!     let _span = span!(Level::TRACE, "shaving_yaks", yaks).entered();
//!
//!     info!("shaving yaks");
//!
//!     let mut yaks_shaved = 0;
//!     for yak in 1..=yaks {
//!         let res = shave(yak);
//!         debug!(yak, shaved = res.is_ok());
//!
//!         if let Err(ref error) = res {
//!             // Like spans, events can also use the field initialization shorthand.
//!             // In this instance, `yak` is the field being initalized.
//!             error!(yak, error = error.as_ref(), "failed to shave yak!");
//!         } else {
//!             yaks_shaved += 1;
//!         }
//!         debug!(yaks_shaved);
//!     }
//!
//!     yaks_shaved
//! }
//! ```
//!
//! ## In libraries
//!
//! Libraries should link only to the `tracing` crate, and use the provided
//! macros to record whatever information will be useful to downstream
//! consumers.
//!
//! ## In executables
//!
//! In order to record trace events, executables have to use a `Subscriber`
//! implementation compatible with `tracing`. A `Subscriber` implements a
//! way of collecting trace data, such as by logging it to standard output.
//!
//! This library does not contain any `Subscriber` implementations; these are
//! provided by [other crates](#related-crates).
//!
//! The simplest way to use a subscriber is to call the [`set_global_default`]
//! function:
//!
//! ```
//! extern crate tracing;
//! # pub struct FooSubscriber;
//! # use tracing::{span::{Id, Attributes, Record}, Metadata};
//! # impl tracing::Subscriber for FooSubscriber {
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &tracing::Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! # }
//! # impl FooSubscriber {
//! #   fn new() -> Self { FooSubscriber }
//! # }
//! # fn main() {
//!
//! let my_subscriber = FooSubscriber::new();
//! tracing::subscriber::set_global_default(my_subscriber)
//!     .expect("setting tracing default failed");
//! # }
//! ```
//!
//! <pre class="compile_fail" style="white-space:normal;font:inherit;">
//!     <strong>Warning</strong>: In general, libraries should <em>not</em> call
//!     <code>set_global_default()</code>! Doing so will cause conflicts when
//!     executables that depend on the library try to set the default later.
//! </pre>
//!
//! This subscriber will be used as the default in all threads for the
//! remainder of the duration of the program, similar to setting the logger
//! in the `log` crate.
//!
//! In addition, the default subscriber can be set through using the
//! [`with_default`] function. This follows the `tokio` pattern of using
//! closures to represent executing code in a context that is exited at the end
//! of the closure. For example:
//!
//! ```rust
//! # pub struct FooSubscriber;
//! # use tracing::{span::{Id, Attributes, Record}, Metadata};
//! # impl tracing::Subscriber for FooSubscriber {
//! #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
//! #   fn record(&self, _: &Id, _: &Record) {}
//! #   fn event(&self, _: &tracing::Event) {}
//! #   fn record_follows_from(&self, _: &Id, _: &Id) {}
//! #   fn enabled(&self, _: &Metadata) -> bool { false }
//! #   fn enter(&self, _: &Id) {}
//! #   fn exit(&self, _: &Id) {}
//! # }
//! # impl FooSubscriber {
//! #   fn new() -> Self { FooSubscriber }
//! # }
//! # fn main() {
//!
//! let my_subscriber = FooSubscriber::new();
//! # #[cfg(feature = "std")]
//! tracing::subscriber::with_default(my_subscriber, || {
//!     // Any trace events generated in this closure or by functions it calls
//!     // will be collected by `my_subscriber`.
//! })
//! # }
//! ```
//!
//! This approach allows trace data to be collected by multiple subscribers
//! within different contexts in the program. Note that the override only applies to the
//! currently executing thread; other threads will not see the change from with_default.
//!
//! Any trace events generated outside the context of a subscriber will not be collected.
//!
//! Once a subscriber has been set, instrumentation points may be added to the
//! executable using the `tracing` crate's macros.
//!
//! ## `log` Compatibility
//!
//! The [`log`] crate provides a simple, lightweight logging facade for Rust.
//! While `tracing` builds upon `log`'s foundation with richer structured
//! diagnostic data, `log`'s simplicity and ubiquity make it the "lowest common
//! denominator" for text-based logging in Rust — a vast majority of Rust
//! libraries and applications either emit or consume `log` records. Therefore,
//! `tracing` provides multiple forms of interoperability with `log`: `tracing`
//! instrumentation can emit `log` records, and a compatibility layer enables
//! `tracing` [`Subscriber`]s to consume `log` records as `tracing` [`Event`]s.
//!
//! ### Emitting `log` Records
//!
//! This crate provides two feature flags, "log" and "log-always", which will
//! cause [spans] and [events] to emit `log` records. When the "log" feature is
//! enabled, if no `tracing` `Subscriber` is active, invoking an event macro or
//! creating a span with fields will emit a `log` record. This is intended
//! primarily for use in libraries which wish to emit diagnostics that can be
//! consumed by applications using `tracing` *or* `log`, without paying the
//! additional overhead of emitting both forms of diagnostics when `tracing` is
//! in use.
//!
//! Enabling the "log-always" feature will cause `log` records to be emitted
//! even if a `tracing` `Subscriber` _is_ set. This is intended to be used in
//! applications where a `log` `Logger` is being used to record a textual log,
//! and `tracing` is used only to record other forms of diagnostics (such as
//! metrics, profiling, or distributed tracing data). Unlike the "log" feature,
//! libraries generally should **not** enable the "log-always" feature, as doing
//! so will prevent applications from being able to opt out of the `log` records.
//!
//! See [here][flags] for more details on this crate's feature flags.
//!
//! The generated `log` records' messages will be a string representation of the
//! span or event's fields, and all additional information recorded by `log`
//! (target, verbosity level, module path, file, and line number) will also be
//! populated. Additionally, `log` records are also generated when spans are
//! entered, exited, and closed. Since these additional span lifecycle logs have
//! the potential to be very verbose, and don't include additional fields, they
//! will always be emitted at the `Trace` level, rather than inheriting the
//! level of the span that generated them. Furthermore, they are are categorized
//! under a separate `log` target, "tracing::span" (and its sub-target,
//! "tracing::span::active", for the logs on entering and exiting a span), which
//! may be enabled or disabled separately from other `log` records emitted by
//! `tracing`.
//!
//! ### Consuming `log` Records
//!
//! The [`tracing-log`] crate provides a compatibility layer which
//! allows a `tracing` [`Subscriber`] to consume `log` records as though they
//! were `tracing` [events]. This allows applications using `tracing` to record
//! the logs emitted by dependencies using `log` as events within the context of
//! the application's trace tree. See [that crate's documentation][log-tracer]
//! for details.
//!
//! [log-tracer]: https://docs.rs/tracing-log/latest/tracing_log/#convert-log-records-to-tracing-events
//!
//! ## Related Crates
//!
//! In addition to `tracing` and `tracing-core`, the [`tokio-rs/tracing`] repository
//! contains several additional crates designed to be used with the `tracing` ecosystem.
//! This includes a collection of `Subscriber` implementations, as well as utility
//! and adapter crates to assist in writing `Subscriber`s and instrumenting
//! applications.
//!
//! In particular, the following crates are likely to be of interest:
//!
//!  - [`tracing-futures`] provides a compatibility layer with the `futures`
//!    crate, allowing spans to be attached to `Future`s, `Stream`s, and `Executor`s.
//!  - [`tracing-subscriber`] provides `Subscriber` implementations and
//!    utilities for working with `Subscriber`s. This includes a [`FmtSubscriber`]
//!    `FmtSubscriber` for logging formatted trace data to stdout, with similar
//!    filtering and formatting to the [`env_logger`] crate.
//!  - [`tracing-log`] provides a compatibility layer with the [`log`] crate,
//!    allowing log messages to be recorded as `tracing` `Event`s within the
//!    trace tree. This is useful when a project using `tracing` have
//!    dependencies which use `log`. Note that if you're using
//!    `tracing-subscriber`'s `FmtSubscriber`, you don't need to depend on
//!    `tracing-log` directly.
//!  - [`tracing-appender`] provides utilities for outputting tracing data,
//!     including a file appender and non blocking writer.
//!
//! Additionally, there are also several third-party crates which are not
//! maintained by the `tokio` project. These include:
//!
//!  - [`tracing-timing`] implements inter-event timing metrics on top of `tracing`.
//!    It provides a subscriber that records the time elapsed between pairs of
//!    `tracing` events and generates histograms.
//!  - [`tracing-opentelemetry`] provides a subscriber for emitting traces to
//!    [OpenTelemetry]-compatible distributed tracing systems.
//!  - [`tracing-honeycomb`] Provides a layer that reports traces spanning multiple machines to [honeycomb.io]. Backed by [`tracing-distributed`].
//!  - [`tracing-distributed`] Provides a generic implementation of a layer that reports traces spanning multiple machines to some backend.
//!  - [`tracing-actix-web`] provides `tracing` integration for the `actix-web` web framework.
//!  - [`tracing-actix`] provides `tracing` integration for the `actix` actor
//!    framework.
//!  - [`axum-insights`] provides `tracing` integration and Application insights export for the `axum` web framework.
//!  - [`tracing-gelf`] implements a subscriber for exporting traces in Greylog
//!    GELF format.
//!  - [`tracing-coz`] provides integration with the [coz] causal profiler
//!    (Linux-only).
//!  - [`tracing-bunyan-formatter`] provides a layer implementation that reports events and spans
//!    in [bunyan] format, enriched with timing information.
//!  - [`tracing-wasm`] provides a `Subscriber`/`Layer` implementation that reports
//!    events and spans via browser `console.log` and [User Timing API (`window.performance`)].
//!  - [`tracing-web`] provides a layer implementation of level-aware logging of events
//!    to web browsers' `console.*` and span events to the [User Timing API (`window.performance`)].
//!  - [`tide-tracing`] provides a [tide] middleware to trace all incoming requests and responses.
//!  - [`test-log`] takes care of initializing `tracing` for tests, based on
//!    environment variables with an `env_logger` compatible syntax.
//!  - [`tracing-unwrap`] provides convenience methods to report failed unwraps
//!    on `Result` or `Option` types to a `Subscriber`.
//!  - [`diesel-tracing`] provides integration with [`diesel`] database connections.
//!  - [`tracing-tracy`] provides a way to collect [Tracy] profiles in instrumented
//!    applications.
//!  - [`tracing-elastic-apm`] provides a layer for reporting traces to [Elastic APM].
//!  - [`tracing-etw`] provides a layer for emitting Windows [ETW] events.
//!  - [`tracing-fluent-assertions`] provides a fluent assertions-style testing
//!    framework for validating the behavior of `tracing` spans.
//!  - [`sentry-tracing`] provides a layer for reporting events and traces to [Sentry].
//!  - [`tracing-forest`] provides a subscriber that preserves contextual coherence by
//!    grouping together logs from the same spans during writing.
//!  - [`tracing-loki`] provides a layer for shipping logs to [Grafana Loki].
//!  - [`tracing-logfmt`] provides a layer that formats events and spans into the logfmt format.
//!  - [`reqwest-tracing`] provides a middleware to trace [`reqwest`] HTTP requests.
//!  - [`tracing-cloudwatch`] provides a layer that sends events to AWS CloudWatch Logs.
//!  - [`clippy-tracing`] provides a tool to add, remove and check for `tracing::instrument`.
//!  - [`json-subscriber`] provides a subscriber for emitting JSON logs. The output can be customized much more than with [`tracing-subscriber`]'s JSON output.
//!
//! If you're the maintainer of a `tracing` ecosystem crate not listed above,
//! please let us know! We'd love to add your project to the list!
//!
//! [`tracing-opentelemetry`]: https://crates.io/crates/tracing-opentelemetry
//! [OpenTelemetry]: https://opentelemetry.io/
//! [`tracing-honeycomb`]: https://crates.io/crates/tracing-honeycomb
//! [`tracing-distributed`]: https://crates.io/crates/tracing-distributed
//! [honeycomb.io]: https://www.honeycomb.io/
//! [`tracing-actix-web`]: https://crates.io/crates/tracing-actix-web
//! [`tracing-actix`]: https://crates.io/crates/tracing-actix
//! [`axum-insights`]: https://crates.io/crates/axum-insights
//! [`tracing-gelf`]: https://crates.io/crates/tracing-gelf
//! [`tracing-coz`]: https://crates.io/crates/tracing-coz
//! [coz]: https://github.com/plasma-umass/coz
//! [`tracing-bunyan-formatter`]: https://crates.io/crates/tracing-bunyan-formatter
//! [bunyan]: https://github.com/trentm/node-bunyan
//! [`tracing-wasm`]: https://docs.rs/tracing-wasm
//! [`tracing-web`]: https://docs.rs/tracing-web
//! [User Timing API (`window.performance`)]: https://developer.mozilla.org/en-US/docs/Web/API/User_Timing_API
//! [`tide-tracing`]: https://crates.io/crates/tide-tracing
//! [tide]: https://crates.io/crates/tide
//! [`test-log`]: https://crates.io/crates/test-log
//! [`tracing-unwrap`]: https://docs.rs/tracing-unwrap
//! [`diesel`]: https://crates.io/crates/diesel
//! [`diesel-tracing`]: https://crates.io/crates/diesel-tracing
//! [`tracing-tracy`]: https://crates.io/crates/tracing-tracy
//! [Tracy]: https://github.com/wolfpld/tracy
//! [`tracing-elastic-apm`]: https://crates.io/crates/tracing-elastic-apm
//! [Elastic APM]: https://www.elastic.co/apm
//! [`tracing-etw`]: https://github.com/microsoft/rust_win_etw/tree/main/win_etw_tracing
//! [ETW]: https://docs.microsoft.com/en-us/windows/win32/etw/about-event-tracing
//! [`tracing-fluent-assertions`]: https://crates.io/crates/tracing-fluent-assertions
//! [`sentry-tracing`]: https://crates.io/crates/sentry-tracing
//! [Sentry]: https://sentry.io/welcome/
//! [`tracing-forest`]: https://crates.io/crates/tracing-forest
//! [`tracing-loki`]: https://crates.io/crates/tracing-loki
//! [Grafana Loki]: https://grafana.com/oss/loki/
//! [`tracing-logfmt`]: https://crates.io/crates/tracing-logfmt
//! [`reqwest-tracing`]: https://crates.io/crates/reqwest-tracing
//! [`reqwest`]: https://crates.io/crates/reqwest
//! [`tracing-cloudwatch`]: https://crates.io/crates/tracing-cloudwatch
//! [`clippy-tracing`]: https://crates.io/crates/clippy-tracing
//! [`json-subscriber`]: https://crates.io/crates/json-subscriber
//!
//! <pre class="ignore" style="white-space:normal;font:inherit;">
//!     <strong>Note</strong>: Some of these ecosystem crates are currently
//!     unreleased and/or in earlier stages of development. They may be less stable
//!     than <code>tracing</code> and <code>tracing-core</code>.
//! </pre>
//!
//! ## Crate Feature Flags
//!
//! The following crate [feature flags] are available:
//!
//! * A set of features controlling the [static verbosity level].
//! * `log`: causes trace instrumentation points to emit [`log`] records as well
//!   as trace events, if a default `tracing` subscriber has not been set. This
//!   is intended for use in libraries whose users may be using either `tracing`
//!   or `log`.
//! * `log-always`: Emit `log` records from all `tracing` spans and events, even
//!   if a `tracing` subscriber has been set. This should be set only by
//!   applications which intend to collect traces and logs separately; if an
//!   adapter is used to convert `log` records into `tracing` events, this will
//!   cause duplicate events to occur.
//! * `attributes`: Includes support for the `#[instrument]` attribute.
//!   This is on by default, but does bring in the `syn` crate as a dependency,
//!   which may add to the compile time of crates that do not already use it.
//! * `std`: Depend on the Rust standard library (enabled by default).
//!
//!   `no_std` users may disable this feature with `default-features = false`:
//!
//!   ```toml
//!   [dependencies]
//!   tracing = { version = "0.1.38", default-features = false }
//!   ```
//!
//! <pre class="ignore" style="white-space:normal;font:inherit;">
//!     <strong>Note</strong>: <code>tracing</code>'s <code>no_std</code> support
//!     requires <code>liballoc</code>.
//! </pre>
//!
//! ### Unstable Features
//!
//! These feature flags enable **unstable** features. The public API may break in 0.1.x
//! releases. To enable these features, the `--cfg tracing_unstable` must be passed to
//! `rustc` when compiling.
//!
//! The following unstable feature flags are currently available:
//!
//! * `valuable`: Enables support for recording [field values] using the
//!   [`valuable`] crate.
//!
//! #### Enabling Unstable Features
//!
//! The easiest way to set the `tracing_unstable` cfg is to use the `RUSTFLAGS`
//! env variable when running `cargo` commands:
//!
//! ```shell
//! RUSTFLAGS="--cfg tracing_unstable" cargo build
//! ```
//! Alternatively, the following can be added to the `.cargo/config` file in a
//! project to automatically enable the cfg flag for that project:
//!
//! ```toml
//! [build]
//! rustflags = ["--cfg", "tracing_unstable"]
//! ```
//!
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section
//! [field values]: crate::field
//! [`valuable`]: https://crates.io/crates/valuable
//!
//! ## Supported Rust Versions
//!
//! Tracing is built against the latest stable release. The minimum supported
//! version is 1.63. The current Tracing version is not guaranteed to build on
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
//! [`log`]: https://docs.rs/log/0.4.6/log/
//! [span]: mod@span
//! [spans]: mod@span
//! [`Span`]: span::Span
//! [`in_scope`]: span::Span::in_scope
//! [event]: Event
//! [events]: Event
//! [`Subscriber`]: subscriber::Subscriber
//! [Subscriber::event]: subscriber::Subscriber::event
//! [`enter`]: subscriber::Subscriber::enter
//! [`exit`]: subscriber::Subscriber::exit
//! [`enabled`]: subscriber::Subscriber::enabled
//! [metadata]: Metadata
//! [`field::display`]: field::display
//! [`field::debug`]: field::debug
//! [`set_global_default`]: subscriber::set_global_default
//! [`with_default`]: subscriber::with_default
//! [`tokio-rs/tracing`]: https://github.com/tokio-rs/tracing
//! [`tracing-futures`]: https://crates.io/crates/tracing-futures
//! [`tracing-subscriber`]: https://crates.io/crates/tracing-subscriber
//! [`tracing-log`]: https://crates.io/crates/tracing-log
//! [`tracing-timing`]: https://crates.io/crates/tracing-timing
//! [`tracing-appender`]: https://crates.io/crates/tracing-appender
//! [`env_logger`]: https://crates.io/crates/env_logger
//! [`FmtSubscriber`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/struct.Subscriber.html
//! [static verbosity level]: level_filters#compile-time-filters
//! [instrument]: https://docs.rs/tracing-attributes/latest/tracing_attributes/attr.instrument.html
//! [flags]: #crate-feature-flags
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg), deny(rustdoc::broken_intra_doc_links))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/logo-type.png",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
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
    private_interfaces,
    private_bounds,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Somehow this `use` statement is necessary for us to re-export the `core`
// macros on Rust 1.26.0. I'm not sure how this makes it work, but it does.
#[allow(unused_imports)]
#[doc(hidden)]
use tracing_core::*;

#[doc(inline)]
pub use self::instrument::Instrument;
pub use self::{dispatcher::Dispatch, event::Event, field::Value, subscriber::Subscriber};

#[doc(hidden)]
pub use self::span::Id;

#[doc(hidden)]
pub use tracing_core::{
    callsite::{self, Callsite},
    metadata,
};
pub use tracing_core::{event, Level, Metadata};

#[doc(inline)]
pub use self::span::Span;
#[cfg(feature = "attributes")]
#[cfg_attr(docsrs, doc(cfg(feature = "attributes")))]
#[doc(inline)]
pub use tracing_attributes::instrument;

#[macro_use]
mod macros;

pub mod dispatcher;
pub mod field;
/// Attach a span to a `std::future::Future`.
pub mod instrument;
pub mod level_filters;
pub mod span;
pub(crate) mod stdlib;
pub mod subscriber;

#[doc(hidden)]
pub mod __macro_support {
    pub use crate::callsite::Callsite;
    use crate::{subscriber::Interest, Metadata};
    // Re-export the `core` functions that are used in macros. This allows
    // a crate to be named `core` and avoid name clashes.
    // See here: https://github.com/tokio-rs/tracing/issues/2761
    pub use core::{concat, file, format_args, iter::Iterator, line, option::Option};

    /// Callsite implementation used by macro-generated code.
    ///
    /// /!\ WARNING: This is *not* a stable API! /!\
    /// This type, and all code contained in the `__macro_support` module, is
    /// a *private* API of `tracing`. It is exposed publicly because it is used
    /// by the `tracing` macros, but it is not part of the stable versioned API.
    /// Breaking changes to this module may occur in small-numbered versions
    /// without warning.
    pub use tracing_core::callsite::DefaultCallsite as MacroCallsite;

    /// /!\ WARNING: This is *not* a stable API! /!\
    /// This function, and all code contained in the `__macro_support` module, is
    /// a *private* API of `tracing`. It is exposed publicly because it is used
    /// by the `tracing` macros, but it is not part of the stable versioned API.
    /// Breaking changes to this module may occur in small-numbered versions
    /// without warning.
    pub fn __is_enabled(meta: &Metadata<'static>, interest: Interest) -> bool {
        interest.is_always() || crate::dispatcher::get_default(|default| default.enabled(meta))
    }

    /// /!\ WARNING: This is *not* a stable API! /!\
    /// This function, and all code contained in the `__macro_support` module, is
    /// a *private* API of `tracing`. It is exposed publicly because it is used
    /// by the `tracing` macros, but it is not part of the stable versioned API.
    /// Breaking changes to this module may occur in small-numbered versions
    /// without warning.
    #[inline]
    #[cfg(feature = "log")]
    pub fn __disabled_span(meta: &'static Metadata<'static>) -> crate::Span {
        crate::Span::new_disabled(meta)
    }

    /// /!\ WARNING: This is *not* a stable API! /!\
    /// This function, and all code contained in the `__macro_support` module, is
    /// a *private* API of `tracing`. It is exposed publicly because it is used
    /// by the `tracing` macros, but it is not part of the stable versioned API.
    /// Breaking changes to this module may occur in small-numbered versions
    /// without warning.
    #[inline]
    #[cfg(not(feature = "log"))]
    pub fn __disabled_span(_: &'static Metadata<'static>) -> crate::Span {
        crate::Span::none()
    }

    /// /!\ WARNING: This is *not* a stable API! /!\
    /// This function, and all code contained in the `__macro_support` module, is
    /// a *private* API of `tracing`. It is exposed publicly because it is used
    /// by the `tracing` macros, but it is not part of the stable versioned API.
    /// Breaking changes to this module may occur in small-numbered versions
    /// without warning.
    #[cfg(feature = "log")]
    pub fn __tracing_log(
        meta: &Metadata<'static>,
        logger: &'static dyn log::Log,
        log_meta: log::Metadata<'_>,
        values: &tracing_core::field::ValueSet<'_>,
    ) {
        logger.log(
            &crate::log::Record::builder()
                .file(meta.file())
                .module_path(meta.module_path())
                .line(meta.line())
                .metadata(log_meta)
                .args(format_args!(
                    "{}",
                    crate::log::LogValueSet {
                        values,
                        is_first: true
                    }
                ))
                .build(),
        );
    }
}

#[cfg(feature = "log")]
#[doc(hidden)]
pub mod log {
    use core::fmt;
    pub use log::*;
    use tracing_core::field::{Field, ValueSet, Visit};

    /// Utility to format [`ValueSet`]s for logging.
    pub(crate) struct LogValueSet<'a> {
        pub(crate) values: &'a ValueSet<'a>,
        pub(crate) is_first: bool,
    }

    impl<'a> fmt::Display for LogValueSet<'a> {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            struct LogVisitor<'a, 'b> {
                f: &'a mut fmt::Formatter<'b>,
                is_first: bool,
                result: fmt::Result,
            }

            impl Visit for LogVisitor<'_, '_> {
                fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
                    let res = if self.is_first {
                        self.is_first = false;
                        if field.name() == "message" {
                            write!(self.f, "{:?}", value)
                        } else {
                            write!(self.f, "{}={:?}", field.name(), value)
                        }
                    } else {
                        write!(self.f, " {}={:?}", field.name(), value)
                    };
                    if let Err(err) = res {
                        self.result = self.result.and(Err(err));
                    }
                }

                fn record_str(&mut self, field: &Field, value: &str) {
                    if field.name() == "message" {
                        self.record_debug(field, &format_args!("{}", value))
                    } else {
                        self.record_debug(field, &value)
                    }
                }
            }

            let mut visit = LogVisitor {
                f,
                is_first: self.is_first,
                result: Ok(()),
            };
            self.values.record(&mut visit);
            visit.result
        }
    }
}

mod sealed {
    pub trait Sealed {}
}
