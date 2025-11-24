//! Context extensions for tracing
use crate::{
    global, otel_debug,
    trace::{Span, SpanContext, Status},
    Context, ContextGuard, KeyValue,
};
use std::{borrow::Cow, error::Error, sync::Mutex};

// Re-export for compatability. This used to be contained here.
pub use crate::context::{FutureExt, WithContext};

const NOOP_SPAN: SynchronizedSpan = SynchronizedSpan {
    span_context: SpanContext::NONE,
    inner: None,
};

/// A reference to the currently active span in this context.
#[derive(Debug)]
pub struct SpanRef<'a>(&'a SynchronizedSpan);

#[derive(Debug)]
pub(crate) struct SynchronizedSpan {
    /// Immutable span context
    span_context: SpanContext,
    /// Mutable span inner that requires synchronization
    inner: Option<Mutex<global::BoxedSpan>>,
}

impl SynchronizedSpan {
    pub(crate) fn span_context(&self) -> &SpanContext {
        &self.span_context
    }
}

impl From<SpanContext> for SynchronizedSpan {
    fn from(value: SpanContext) -> Self {
        Self {
            span_context: value,
            inner: None,
        }
    }
}

impl<T: Span + Send + Sync + 'static> From<T> for SynchronizedSpan {
    fn from(value: T) -> Self {
        Self {
            span_context: value.span_context().clone(),
            inner: Some(Mutex::new(global::BoxedSpan::new(value))),
        }
    }
}

impl SpanRef<'_> {
    fn with_inner_mut<F: FnOnce(&mut global::BoxedSpan)>(&self, f: F) {
        if let Some(ref inner) = self.0.inner {
            match inner.lock() {
                Ok(mut locked) => f(&mut locked),
                Err(err) => {
                    otel_debug!(
                        name: "SpanRef.LockFailed",
                        message = "Failed to acquire lock for SpanRef: {:?}",
                        reason = format!("{:?}", err),
                        span_context = format!("{:?}", self.0.span_context));
                }
            }
        }
    }
}

impl SpanRef<'_> {
    /// Record an event in the context this span.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    pub fn add_event<T>(&self, name: T, attributes: Vec<KeyValue>)
    where
        T: Into<Cow<'static, str>>,
    {
        self.with_inner_mut(|inner| inner.add_event(name, attributes))
    }

    /// Record an error as an event for this span.
    ///
    /// An additional call to [Span::set_status] is required if the status of the
    /// span should be set to error, as this method does not change the span status.
    ///
    /// If this span is not being recorded then this method does nothing.
    pub fn record_error(&self, err: &dyn Error) {
        self.with_inner_mut(|inner| inner.record_error(err))
    }

    /// Record an event with a timestamp in the context this span.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    pub fn add_event_with_timestamp<T>(
        &self,
        name: T,
        timestamp: std::time::SystemTime,
        attributes: Vec<crate::KeyValue>,
    ) where
        T: Into<Cow<'static, str>>,
    {
        self.with_inner_mut(move |inner| {
            inner.add_event_with_timestamp(name, timestamp, attributes)
        })
    }

    /// A reference to the [`SpanContext`] for this span.
    pub fn span_context(&self) -> &SpanContext {
        &self.0.span_context
    }

    /// Returns `true` if this span is recording information.
    ///
    /// Spans will not be recording information after they have ended.
    ///
    /// This flag may be `true` despite the entire trace being sampled out. This
    /// allows recording and processing of information about the individual
    /// spans without sending it to the backend. An example of this scenario may
    /// be recording and processing of all incoming requests for the processing
    /// and building of SLA/SLO latency charts while sending only a subset -
    /// sampled spans - to the backend.
    pub fn is_recording(&self) -> bool {
        self.0
            .inner
            .as_ref()
            .and_then(|inner| inner.lock().ok().map(|active| active.is_recording()))
            .unwrap_or(false)
    }

    /// Set an attribute of this span.
    ///
    /// Setting an attribute with the same key as an existing attribute
    /// generally overwrites the existing attribute's value.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    pub fn set_attribute(&self, attribute: crate::KeyValue) {
        self.with_inner_mut(move |inner| inner.set_attribute(attribute))
    }

    /// Set multiple attributes of this span.
    ///
    /// Setting an attribute with the same key as an existing attribute
    /// generally overwrites the existing attribute's value.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    pub fn set_attributes(&self, attributes: impl IntoIterator<Item = KeyValue>) {
        self.with_inner_mut(move |inner| inner.set_attributes(attributes))
    }

    /// Sets the status of this `Span`.
    ///
    /// If used, this will override the default span status, which is [`Status::Unset`].
    pub fn set_status(&self, status: Status) {
        self.with_inner_mut(move |inner| inner.set_status(status))
    }

    /// Updates the span's name.
    ///
    /// After this update, any sampling behavior based on the name will depend on
    /// the implementation.
    pub fn update_name<T>(&self, new_name: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.with_inner_mut(move |inner| inner.update_name(new_name))
    }

    /// Adds a [`Link`] to another [`SpanContext`].
    ///
    /// This method allows linking the current span to another span, identified by
    /// its `SpanContext`. Links can be used to connect spans from different traces
    /// or within the same trace. Attributes can be attached to the link to provide
    /// additional context or metadata.
    ///
    /// # Arguments
    ///
    /// * `span_context` - The `SpanContext` of the span to link to. This represents
    ///   the target span's unique identifiers and trace information.
    /// * `attributes` - A vector of `KeyValue` pairs that describe additional
    ///   attributes of the link. These attributes can include any contextual
    ///   information relevant to the link between the spans.
    ///
    /// Note - Any [`Link`] added via this mechanism is not accessible to a `Sampler`.
    /// It is recommended to add Links at [`Span`] creation time, rather than adding
    /// them afterwards.
    ///
    /// [`Link`]: crate::trace::Link
    pub fn add_link(&self, span_context: SpanContext, attributes: Vec<KeyValue>) {
        self.with_inner_mut(move |inner| inner.add_link(span_context, attributes));
    }

    /// Signals that the operation described by this span has now ended.
    pub fn end(&self) {
        self.end_with_timestamp(crate::time::now());
    }

    /// Signals that the operation described by this span ended at the given time.
    pub fn end_with_timestamp(&self, timestamp: std::time::SystemTime) {
        self.with_inner_mut(move |inner| inner.end_with_timestamp(timestamp))
    }
}

/// Methods for storing and retrieving trace data in a [`Context`].
///
/// See [`Context`] for examples of setting and retrieving the current context.
pub trait TraceContextExt {
    /// Returns a clone of the current context with the included [`Span`].
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, trace::{TraceContextExt, Tracer}, Context};
    ///
    /// let tracer = global::tracer("example");
    ///
    /// // build a span
    /// let span = tracer.start("parent_span");
    ///
    /// // create a new context from the currently active context that includes this span
    /// let cx = Context::current_with_span(span);
    ///
    /// // create a child span by explicitly specifying the parent context
    /// let child = tracer.start_with_context("child_span", &cx);
    /// # drop(child)
    /// ```
    fn current_with_span<T: crate::trace::Span + Send + Sync + 'static>(span: T) -> Self;

    /// Returns a clone of this context with the included span.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, trace::{TraceContextExt, Tracer}, Context};
    ///
    /// fn fn_with_passed_in_context(cx: &Context) {
    ///     let tracer = global::tracer("example");
    ///
    ///     // build a span
    ///     let span = tracer.start("parent_span");
    ///
    ///     // create a new context from the given context that includes the span
    ///     let cx_with_parent = cx.with_span(span);
    ///
    ///     // create a child span by explicitly specifying the parent context
    ///     let child = tracer.start_with_context("child_span", &cx_with_parent);
    ///     # drop(child)
    /// }
    ///
    fn with_span<T: crate::trace::Span + Send + Sync + 'static>(&self, span: T) -> Self;

    /// Returns a reference to this context's span, or the default no-op span if
    /// none has been set.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{trace::TraceContextExt, Context};
    ///
    /// // Add an event to the currently active span
    /// Context::map_current(|cx| cx.span().add_event("An event!", vec![]));
    /// ```
    fn span(&self) -> SpanRef<'_>;

    /// Returns whether or not an active span has been set.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{trace::TraceContextExt, Context};
    ///
    /// assert!(!Context::map_current(|cx| cx.has_active_span()));
    /// ```
    fn has_active_span(&self) -> bool;

    /// Returns a copy of this context with the span context included.
    ///
    /// This is useful for building propagators.
    fn with_remote_span_context(&self, span_context: crate::trace::SpanContext) -> Self;
}

impl TraceContextExt for Context {
    fn current_with_span<T: crate::trace::Span + Send + Sync + 'static>(span: T) -> Self {
        Context::current_with_synchronized_span(span.into())
    }

    fn with_span<T: crate::trace::Span + Send + Sync + 'static>(&self, span: T) -> Self {
        self.with_synchronized_span(span.into())
    }

    fn span(&self) -> SpanRef<'_> {
        if let Some(span) = self.span.as_ref() {
            SpanRef(span)
        } else {
            SpanRef(&NOOP_SPAN)
        }
    }

    fn has_active_span(&self) -> bool {
        self.span.is_some()
    }

    fn with_remote_span_context(&self, span_context: crate::trace::SpanContext) -> Self {
        self.with_synchronized_span(span_context.into())
    }
}

/// Mark a given `Span` as active.
///
/// The `Tracer` MUST provide a way to update its active `Span`, and MAY provide convenience
/// methods to manage a `Span`'s lifetime and the scope in which a `Span` is active. When an
/// active `Span` is made inactive, the previously-active `Span` SHOULD be made active. A `Span`
/// maybe finished (i.e. have a non-null end time) but still be active. A `Span` may be active
/// on one thread after it has been made inactive on another.
///
/// # Examples
///
/// ```
/// use opentelemetry::{global, trace::{Span, Tracer}, KeyValue};
/// use opentelemetry::trace::{get_active_span, mark_span_as_active};
///
/// fn my_function() {
///     let tracer = global::tracer("my-component-a");
///     // start an active span in one function
///     let span = tracer.start("span-name");
///     let _guard = mark_span_as_active(span);
///     // anything happening in functions we call can still access the active span...
///     my_other_function();
/// }
///
/// fn my_other_function() {
///     // call methods on the current span from
///     get_active_span(|span| {
///         span.add_event("An event!".to_string(), vec![KeyValue::new("happened", true)]);
///     });
/// }
/// ```
#[must_use = "Dropping the guard detaches the context."]
pub fn mark_span_as_active<T: crate::trace::Span + Send + Sync + 'static>(span: T) -> ContextGuard {
    let cx = Context::current_with_span(span);
    cx.attach()
}

/// Executes a closure with a reference to this thread's current span.
///
/// # Examples
///
/// ```
/// use opentelemetry::{global, trace::{Span, Tracer}, KeyValue};
/// use opentelemetry::trace::get_active_span;
///
/// fn my_function() {
///     // start an active span in one function
///     global::tracer("my-component").in_span("span-name", |_cx| {
///         // anything happening in functions we call can still access the active span...
///         my_other_function();
///     })
/// }
///
/// fn my_other_function() {
///     // call methods on the current span from
///     get_active_span(|span| {
///         span.add_event("An event!", vec![KeyValue::new("happened", true)]);
///     })
/// }
/// ```
pub fn get_active_span<F, T>(f: F) -> T
where
    F: FnOnce(SpanRef<'_>) -> T,
{
    Context::map_current(|cx| f(cx.span()))
}
