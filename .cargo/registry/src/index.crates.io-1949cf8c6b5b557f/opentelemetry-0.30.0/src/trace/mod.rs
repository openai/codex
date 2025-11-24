//! API for tracing applications and libraries.
//!
//! The `trace` module includes types for tracking the progression of a single
//! request while it is handled by services that make up an application. A trace
//! is a tree of [`Span`]s which are objects that represent the work being done
//! by individual services or components involved in a request as it flows
//! through a system. This module implements the OpenTelemetry [trace
//! specification].
//!
//! [trace specification]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/api.md
//!
//! ## Getting Started
//!
//! In application code:
//!
//! ```
//! use opentelemetry::trace::{Tracer, noop::NoopTracerProvider};
//! use opentelemetry::global;
//!
//! fn init_tracer() {
//!     // Swap this no-op provider for your tracing service of choice (jaeger, zipkin, etc)
//!     let provider = NoopTracerProvider::new();
//!
//!     // Configure the global `TracerProvider` singleton when your app starts
//!     // (there is a no-op default if this is not set by your application)
//!     let _ = global::set_tracer_provider(provider);
//! }
//!
//! fn do_something_tracked() {
//!     // Then you can get a named tracer instance anywhere in your codebase.
//!     let tracer = global::tracer("my-component");
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//! }
//!
//! // in main or other app start
//! init_tracer();
//! do_something_tracked();
//! ```
//!
//! In library code:
//!
//! ```
//! use opentelemetry::{global, trace::{Span, Tracer, TracerProvider}};
//! use opentelemetry::InstrumentationScope;
//! use std::sync::Arc;
//!
//! fn my_library_function() {
//!     // Use the global tracer provider to get access to the user-specified
//!     // tracer configuration
//!     let tracer_provider = global::tracer_provider();
//!
//!     // Get a tracer for this library
//!     let scope = InstrumentationScope::builder("my_name")
//!         .with_version(env!("CARGO_PKG_VERSION"))
//!         .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
//!         .build();
//!
//!     let tracer = tracer_provider.tracer_with_scope(scope);
//!
//!     // Create spans
//!     let mut span = tracer.start("doing_work");
//!
//!     // Do work...
//!
//!     // End the span
//!     span.end();
//! }
//! ```
//!
//! ## Overview
//!
//! The tracing API consists of a three main traits:
//!
//! * [`TracerProvider`]s are the entry point of the API. They provide access to
//!   `Tracer`s.
//! * [`Tracer`]s are types responsible for creating `Span`s.
//! * [`Span`]s provide the API to trace an operation.
//!
//! ## Working with Async Runtimes
//!
//! Exporting spans often involves sending data over a network or performing
//! other I/O tasks. OpenTelemetry allows you to schedule these tasks using
//! whichever runtime you are already using such as [Tokio].
//! When using an async runtime it's best to use the batch span processor
//! where the spans will be sent in batches as opposed to being sent once ended,
//! which often ends up being more efficient.
//!
//! [Tokio]: https://tokio.rs
//!
//! ## Managing Active Spans
//!
//! Spans can be marked as "active" for a given [`Context`], and all newly
//! created spans will automatically be children of the currently active span.
//!
//! The active span for a given thread can be managed via [`get_active_span`]
//! and [`mark_span_as_active`].
//!
//! [`Context`]: crate::Context
//!
//! ```
//! use opentelemetry::{global, trace::{self, Span, Status, Tracer, TracerProvider}};
//!
//! fn may_error(rand: f32) {
//!     if rand < 0.5 {
//!         // Get the currently active span to record additional attributes,
//!         // status, etc.
//!         trace::get_active_span(|span| {
//!             span.set_status(Status::error("value too small"));
//!         });
//!     }
//! }
//!
//! // Get a tracer
//! let tracer = global::tracer("my_tracer");
//!
//! // Create a span
//! let span = tracer.start("parent_span");
//!
//! // Mark the span as active
//! let active = trace::mark_span_as_active(span);
//!
//! // Any span created here will be a child of `parent_span`...
//!
//! // Drop the guard and the span will no longer be active
//! drop(active)
//! ```
//!
//! Additionally [`Tracer::in_span`] can be used as shorthand to simplify
//! managing the parent context.
//!
//! ```
//! use opentelemetry::{global, trace::Tracer};
//!
//! // Get a tracer
//! let tracer = global::tracer("my_tracer");
//!
//! // Use `in_span` to create a new span and mark it as the parent, dropping it
//! // at the end of the block.
//! tracer.in_span("parent_span", |cx| {
//!     // spans created here will be children of `parent_span`
//! });
//! ```
//!
//! #### Async active spans
//!
//! Async spans can be propagated with [`TraceContextExt`] and [`FutureExt`].
//!
//! ```
//! use opentelemetry::{Context, global, trace::{FutureExt, TraceContextExt, Tracer}};
//!
//! async fn some_work() { }
//! # async fn in_an_async_context() {
//!
//! // Get a tracer
//! let tracer = global::tracer("my_tracer");
//!
//! // Start a span
//! let span = tracer.start("my_span");
//!
//! // Perform some async work with this span as the currently active parent.
//! some_work().with_context(Context::current_with_span(span)).await;
//! # }
//! ```

use std::borrow::Cow;
use std::time;

pub(crate) mod context;
pub mod noop;
mod span;
mod span_context;
mod tracer;
mod tracer_provider;

pub use self::{
    context::{
        get_active_span, mark_span_as_active, FutureExt, SpanRef, TraceContextExt, WithContext,
    },
    span::{Span, SpanKind, Status},
    span_context::{SpanContext, TraceState},
    tracer::{SamplingDecision, SamplingResult, SpanBuilder, Tracer},
    tracer_provider::TracerProvider,
};
use crate::KeyValue;
pub use crate::{SpanId, TraceFlags, TraceId};

/// Events record things that happened during a [`Span`]'s lifetime.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// The name of this event.
    pub name: Cow<'static, str>,

    /// The time at which this event occurred.
    pub timestamp: time::SystemTime,

    /// Attributes that describe this event.
    pub attributes: Vec<KeyValue>,

    /// The number of attributes that were above the configured limit, and thus
    /// dropped.
    pub dropped_attributes_count: u32,
}

impl Event {
    /// Create new `Event`
    pub fn new<T: Into<Cow<'static, str>>>(
        name: T,
        timestamp: time::SystemTime,
        attributes: Vec<KeyValue>,
        dropped_attributes_count: u32,
    ) -> Self {
        Event {
            name: name.into(),
            timestamp,
            attributes,
            dropped_attributes_count,
        }
    }

    /// Create new `Event` with a given name.
    pub fn with_name<T: Into<Cow<'static, str>>>(name: T) -> Self {
        Event {
            name: name.into(),
            timestamp: crate::time::now(),
            attributes: Vec::new(),
            dropped_attributes_count: 0,
        }
    }
}

/// Link is the relationship between two Spans.
///
/// The relationship can be within the same trace or across different traces.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// The span context of the linked span.
    pub span_context: SpanContext,

    /// Attributes that describe this link.
    pub attributes: Vec<KeyValue>,

    /// The number of attributes that were above the configured limit, and thus
    /// dropped.
    pub dropped_attributes_count: u32,
}

impl Link {
    /// Create new `Link`
    pub fn new(
        span_context: SpanContext,
        attributes: Vec<KeyValue>,
        dropped_attributes_count: u32,
    ) -> Self {
        Link {
            span_context,
            attributes,
            dropped_attributes_count,
        }
    }

    /// Create new `Link` with given context
    pub fn with_context(span_context: SpanContext) -> Self {
        Link {
            span_context,
            attributes: Vec::new(),
            dropped_attributes_count: 0,
        }
    }
}
