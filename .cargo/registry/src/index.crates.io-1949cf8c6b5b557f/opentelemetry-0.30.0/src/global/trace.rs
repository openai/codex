use crate::trace::{noop::NoopTracerProvider, SpanContext, Status};
use crate::InstrumentationScope;
use crate::{trace, trace::TracerProvider, Context, KeyValue};
use std::borrow::Cow;
use std::fmt;
use std::mem;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::SystemTime;

/// Allows a specific [`crate::trace::Span`] to be used generically by [`BoxedSpan`]
/// instances by mirroring the interface and boxing the return types.
pub trait ObjectSafeSpan {
    /// An API to record events at a specific time in the context of a given `Span`.
    ///
    /// Events SHOULD preserve the order in which they're set. This will typically match
    /// the ordering of the events' timestamps.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard event names and
    /// keys"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// which have prescribed semantic meanings.
    fn add_event_with_timestamp(
        &mut self,
        name: Cow<'static, str>,
        timestamp: SystemTime,
        attributes: Vec<KeyValue>,
    );

    /// Returns the `SpanContext` for the given `Span`. The returned value may be used even after
    /// the `Span is finished. The returned value MUST be the same for the entire `Span` lifetime.
    fn span_context(&self) -> &SpanContext;

    /// Returns true if this `Span` is recording information like events with the `add_event`
    /// operation, attributes using `set_attributes`, status with `set_status`, etc.
    ///
    /// This flag SHOULD be used to avoid expensive computations of a `Span` attributes or events in
    /// case when a `Span` is definitely not recorded. Note that any child span's recording is
    /// determined independently from the value of this flag (typically based on the sampled flag of
    /// a `TraceFlag` on `SpanContext`).
    ///
    /// This flag may be true despite the entire trace being sampled out. This allows to record and
    /// process information about the individual Span without sending it to the backend. An example
    /// of this scenario may be recording and processing of all incoming requests for the processing
    /// and building of SLA/SLO latency charts while sending only a subset - sampled spans - to the
    /// backend. See also the sampling section of SDK design.
    ///
    /// Users of the API should only access the `is_recording` property when instrumenting code and
    /// never access `SampledFlag` unless used in context propagators.
    fn is_recording(&self) -> bool;

    /// An API to set a single `Attribute` where the attribute properties are passed
    /// as arguments. To avoid extra allocations some implementations may offer a separate API for
    /// each of the possible value types.
    ///
    /// An `Attribute` is defined as a `KeyValue` pair.
    ///
    /// Attributes SHOULD preserve the order in which they're set. Setting an attribute
    /// with the same key as an existing attribute SHOULD overwrite the existing
    /// attribute's value.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard
    /// attributes"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// that have prescribed semantic meanings.
    fn set_attribute(&mut self, attribute: KeyValue);

    /// Sets the status of the `Span`. `message` MUST be ignored when the status is `OK` or
    /// `Unset`.
    ///
    /// The order of status is `Ok` > `Error` > `Unset`. That's means set the status
    /// to `Unset` will always be ignore, set the status to `Error` only works when current
    /// status is `Unset`, set the status to `Ok` will be consider final and any further call
    /// to this function will be ignore.
    fn set_status(&mut self, status: Status);

    /// Updates the `Span`'s name. After this update, any sampling behavior based on the
    /// name will depend on the implementation.
    ///
    /// It is highly discouraged to update the name of a `Span` after its creation.
    /// `Span` name is often used to group, filter and identify the logical groups of
    /// spans. Often, filtering logic will be implemented before the `Span` creation
    /// for performance reasons, and the name update may interfere with this logic.
    ///
    /// The method name is called `update_name` to differentiate this method from the
    /// regular property. It emphasizes that this operation signifies a
    /// major change for a `Span` and may lead to re-calculation of sampling or
    /// filtering decisions made previously depending on the implementation.
    fn update_name(&mut self, new_name: Cow<'static, str>);

    /// Adds a link to this span
    ///
    fn add_link(&mut self, span_context: SpanContext, attributes: Vec<KeyValue>);

    /// Finishes the `Span`.
    ///
    /// Implementations MUST ignore all subsequent calls to `end` (there might be
    /// exceptions when the tracer is streaming events and has no mutable state
    /// associated with the Span).
    ///
    /// Calls to `end` a Span MUST not have any effects on child `Span`s as they may
    /// still be running and can be ended later.
    ///
    /// This API MUST be non-blocking.
    fn end(&mut self) {
        self.end_with_timestamp(crate::time::now());
    }

    /// Finishes the `Span` with given timestamp
    ///
    /// For more details, refer to [`Span::end`]
    ///
    /// [`Span::end`]: trace::Span::end
    fn end_with_timestamp(&mut self, timestamp: SystemTime);
}

impl<T: trace::Span> ObjectSafeSpan for T {
    fn add_event_with_timestamp(
        &mut self,
        name: Cow<'static, str>,
        timestamp: SystemTime,
        attributes: Vec<KeyValue>,
    ) {
        self.add_event_with_timestamp(name, timestamp, attributes)
    }

    fn span_context(&self) -> &SpanContext {
        self.span_context()
    }

    fn is_recording(&self) -> bool {
        self.is_recording()
    }

    fn set_attribute(&mut self, attribute: KeyValue) {
        self.set_attribute(attribute)
    }

    fn set_status(&mut self, status: Status) {
        self.set_status(status)
    }

    fn update_name(&mut self, new_name: Cow<'static, str>) {
        self.update_name(new_name)
    }

    fn add_link(&mut self, span_context: SpanContext, attributes: Vec<KeyValue>) {
        self.add_link(span_context, attributes)
    }

    fn end_with_timestamp(&mut self, timestamp: SystemTime) {
        self.end_with_timestamp(timestamp)
    }
}

/// Wraps the [`BoxedTracer`]'s [`Span`] so it can be used generically by
/// applications without knowing the underlying type.
///
/// [`Span`]: crate::trace::Span
pub struct BoxedSpan(Box<dyn ObjectSafeSpan + Send + Sync>);

impl BoxedSpan {
    pub(crate) fn new<T>(span: T) -> Self
    where
        T: ObjectSafeSpan + Send + Sync + 'static,
    {
        BoxedSpan(Box::new(span))
    }
}

impl fmt::Debug for BoxedSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoxedSpan")
    }
}

impl trace::Span for BoxedSpan {
    /// Records events at a specific time in the context of a given `Span`.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard event names and
    /// keys"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// which have prescribed semantic meanings.
    fn add_event_with_timestamp<T>(
        &mut self,
        name: T,
        timestamp: SystemTime,
        attributes: Vec<KeyValue>,
    ) where
        T: Into<Cow<'static, str>>,
    {
        self.0
            .add_event_with_timestamp(name.into(), timestamp, attributes)
    }

    /// Returns the `SpanContext` for the given `Span`.
    fn span_context(&self) -> &trace::SpanContext {
        self.0.span_context()
    }

    /// Returns true if this `Span` is recording information like events with the `add_event`
    /// operation, attributes using `set_attributes`, status with `set_status`, etc.
    fn is_recording(&self) -> bool {
        self.0.is_recording()
    }

    /// Sets a single `Attribute` where the attribute properties are passed as arguments.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard
    /// attributes"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// that have prescribed semantic meanings.
    fn set_attribute(&mut self, attribute: KeyValue) {
        self.0.set_attribute(attribute)
    }

    /// Sets the status of the `Span`. If used, this will override the default `Span`
    /// status, which is `Unset`.
    fn set_status(&mut self, status: trace::Status) {
        self.0.set_status(status)
    }

    /// Updates the `Span`'s name.
    fn update_name<T>(&mut self, new_name: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.0.update_name(new_name.into())
    }

    /// Adds a link to this span
    ///
    fn add_link(&mut self, span_context: trace::SpanContext, attributes: Vec<KeyValue>) {
        self.0.add_link(span_context, attributes)
    }

    /// Finishes the span with given timestamp.
    fn end_with_timestamp(&mut self, timestamp: SystemTime) {
        self.0.end_with_timestamp(timestamp);
    }
}

/// Wraps the [`GlobalTracerProvider`]'s [`Tracer`] so it can be used generically by
/// applications without knowing the underlying type.
///
/// [`Tracer`]: crate::trace::Tracer
/// [`GlobalTracerProvider`]: crate::global::GlobalTracerProvider
pub struct BoxedTracer(Box<dyn ObjectSafeTracer + Send + Sync>);

impl BoxedTracer {
    /// Create a `BoxedTracer` from an object-safe tracer.
    pub fn new(tracer: Box<dyn ObjectSafeTracer + Send + Sync>) -> Self {
        BoxedTracer(tracer)
    }
}

impl fmt::Debug for BoxedTracer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoxedTracer")
    }
}

impl trace::Tracer for BoxedTracer {
    /// Global tracer uses `BoxedSpan`s so that it can be a global singleton,
    /// which is not possible if it takes generic type parameters.
    type Span = BoxedSpan;

    /// Create a span from a `SpanBuilder`
    fn build_with_context(&self, builder: trace::SpanBuilder, parent_cx: &Context) -> Self::Span {
        BoxedSpan(self.0.build_with_context_boxed(builder, parent_cx))
    }
}

/// Allows a specific [`Tracer`] to be used generically by [`BoxedTracer`]
/// instances by mirroring the interface and boxing the return types.
///
/// [`Tracer`]: crate::trace::Tracer
pub trait ObjectSafeTracer {
    /// Returns a trait object so the underlying implementation can be swapped
    /// out at runtime.
    fn build_with_context_boxed(
        &self,
        builder: trace::SpanBuilder,
        parent_cx: &Context,
    ) -> Box<dyn ObjectSafeSpan + Send + Sync>;
}

impl<S, T> ObjectSafeTracer for T
where
    S: trace::Span + Send + Sync + 'static,
    T: trace::Tracer<Span = S>,
{
    /// Returns a trait object so the underlying implementation can be swapped
    /// out at runtime.
    fn build_with_context_boxed(
        &self,
        builder: trace::SpanBuilder,
        parent_cx: &Context,
    ) -> Box<dyn ObjectSafeSpan + Send + Sync> {
        Box::new(self.build_with_context(builder, parent_cx))
    }
}

/// Allows a specific [`TracerProvider`] to be used generically by the
/// [`GlobalTracerProvider`] by mirroring the interface and boxing the return types.
///
/// [`TracerProvider`]: crate::trace::TracerProvider
/// [`GlobalTracerProvider`]: crate::global::GlobalTracerProvider
pub trait ObjectSafeTracerProvider {
    /// Creates a versioned named tracer instance that is a trait object through the underlying
    /// `TracerProvider`.
    fn boxed_tracer(&self, scope: InstrumentationScope) -> Box<dyn ObjectSafeTracer + Send + Sync>;
}

impl<S, T, P> ObjectSafeTracerProvider for P
where
    S: trace::Span + Send + Sync + 'static,
    T: trace::Tracer<Span = S> + Send + Sync + 'static,
    P: trace::TracerProvider<Tracer = T>,
{
    /// Return a versioned boxed tracer
    fn boxed_tracer(&self, scope: InstrumentationScope) -> Box<dyn ObjectSafeTracer + Send + Sync> {
        Box::new(self.tracer_with_scope(scope))
    }
}

/// Represents the globally configured [`TracerProvider`] instance for this
/// application. This allows generic tracing through the returned
/// [`BoxedTracer`] instances.
///
/// [`TracerProvider`]: crate::trace::TracerProvider
#[derive(Clone)]
pub struct GlobalTracerProvider {
    provider: Arc<dyn ObjectSafeTracerProvider + Send + Sync>,
}

impl fmt::Debug for GlobalTracerProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GlobalTracerProvider")
    }
}

impl GlobalTracerProvider {
    /// Create a new GlobalTracerProvider instance from a struct that implements `TracerProvider`.
    fn new<P, T, S>(provider: P) -> Self
    where
        S: trace::Span + Send + Sync + 'static,
        T: trace::Tracer<Span = S> + Send + Sync + 'static,
        P: trace::TracerProvider<Tracer = T> + Send + Sync + 'static,
    {
        GlobalTracerProvider {
            provider: Arc::new(provider),
        }
    }
}

impl trace::TracerProvider for GlobalTracerProvider {
    type Tracer = BoxedTracer;

    /// Create a tracer using the global provider.
    fn tracer_with_scope(&self, scope: InstrumentationScope) -> Self::Tracer {
        BoxedTracer(self.provider.boxed_tracer(scope))
    }
}

/// The global `Tracer` provider singleton.
static GLOBAL_TRACER_PROVIDER: OnceLock<RwLock<GlobalTracerProvider>> = OnceLock::new();

#[inline]
fn global_tracer_provider() -> &'static RwLock<GlobalTracerProvider> {
    GLOBAL_TRACER_PROVIDER
        .get_or_init(|| RwLock::new(GlobalTracerProvider::new(NoopTracerProvider::new())))
}

/// Returns an instance of the currently configured global [`TracerProvider`] through
/// [`GlobalTracerProvider`].
///
/// [`TracerProvider`]: crate::trace::TracerProvider
/// [`GlobalTracerProvider`]: crate::global::GlobalTracerProvider
pub fn tracer_provider() -> GlobalTracerProvider {
    global_tracer_provider()
        .read()
        .expect("GLOBAL_TRACER_PROVIDER RwLock poisoned")
        .clone()
}

/// Creates a named instance of [`Tracer`] via the configured [`GlobalTracerProvider`].
///
/// If the name is an empty string, the provider will use a default name.
///
/// This is a more convenient way of expressing `global::tracer_provider().tracer(name)`.
///
/// [`Tracer`]: crate::trace::Tracer
pub fn tracer(name: impl Into<Cow<'static, str>>) -> BoxedTracer {
    tracer_provider().tracer(name.into())
}

/// Creates a [`Tracer`] with the given instrumentation scope
/// via the configured [`GlobalTracerProvider`].
///
/// This is a simpler alternative to `global::tracer_provider().tracer_with_scope(...)`
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use opentelemetry::global::tracer_with_scope;
/// use opentelemetry::InstrumentationScope;
/// use opentelemetry::KeyValue;
///
/// let scope = InstrumentationScope::builder("io.opentelemetry")
///     .with_version("0.17")
///     .with_schema_url("https://opentelemetry.io/schema/1.2.0")
///     .with_attributes(vec![(KeyValue::new("key", "value"))])
///     .build();
///
/// let tracer = tracer_with_scope(scope);
/// ```
///
/// [`Tracer`]: crate::trace::Tracer
pub fn tracer_with_scope(scope: InstrumentationScope) -> BoxedTracer {
    tracer_provider().tracer_with_scope(scope)
}

/// Sets the given [`TracerProvider`] instance as the current global provider.
///
/// It returns the [`TracerProvider`] instance that was previously mounted as global provider
/// (e.g. [`NoopTracerProvider`] if a provider had not been set before).
///
/// Libraries should NOT call this function. It is intended for applications/executables.
/// [`TracerProvider`]: crate::trace::TracerProvider
pub fn set_tracer_provider<P, T, S>(new_provider: P) -> GlobalTracerProvider
where
    S: trace::Span + Send + Sync + 'static,
    T: trace::Tracer<Span = S> + Send + Sync + 'static,
    P: trace::TracerProvider<Tracer = T> + Send + Sync + 'static,
{
    let mut tracer_provider = global_tracer_provider()
        .write()
        .expect("GLOBAL_TRACER_PROVIDER RwLock poisoned");
    mem::replace(
        &mut *tracer_provider,
        GlobalTracerProvider::new(new_provider),
    )
}
