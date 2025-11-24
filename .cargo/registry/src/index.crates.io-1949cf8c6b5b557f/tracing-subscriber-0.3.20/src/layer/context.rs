use tracing_core::{metadata::Metadata, span, subscriber::Subscriber, Event};

use crate::registry::{self, LookupSpan, SpanRef};

#[cfg(all(feature = "registry", feature = "std"))]
use crate::{filter::FilterId, registry::Registry};
/// Represents information about the current context provided to [`Layer`]s by the
/// wrapped [`Subscriber`].
///
/// To access [stored data] keyed by a span ID, implementors of the `Layer`
/// trait should ensure that the `Subscriber` type parameter is *also* bound by the
/// [`LookupSpan`]:
///
/// ```rust
/// use tracing::Subscriber;
/// use tracing_subscriber::{Layer, registry::LookupSpan};
///
/// pub struct MyLayer;
///
/// impl<S> Layer<S> for MyLayer
/// where
///     S: Subscriber + for<'a> LookupSpan<'a>,
/// {
///     // ...
/// }
/// ```
///
/// [`Layer`]: super::Layer
/// [`Subscriber`]: tracing_core::Subscriber
/// [stored data]: crate::registry::SpanRef
/// [`LookupSpan`]: crate::registry::LookupSpan
#[derive(Debug)]
pub struct Context<'a, S> {
    subscriber: Option<&'a S>,
    /// The bitmask of all [`Filtered`] layers that currently apply in this
    /// context. If there is only a single [`Filtered`] wrapping the layer that
    /// produced this context, then this is that filter's ID. Otherwise, if we
    /// are in a nested tree with multiple filters, this is produced by
    /// [`and`]-ing together the [`FilterId`]s of each of the filters that wrap
    /// the current layer.
    ///
    /// [`Filtered`]: crate::filter::Filtered
    /// [`FilterId`]: crate::filter::FilterId
    /// [`and`]: crate::filter::FilterId::and
    #[cfg(all(feature = "registry", feature = "std"))]
    filter: FilterId,
}

// === impl Context ===

impl<'a, S> Context<'a, S>
where
    S: Subscriber,
{
    pub(super) fn new(subscriber: &'a S) -> Self {
        Self {
            subscriber: Some(subscriber),

            #[cfg(feature = "registry")]
            filter: FilterId::none(),
        }
    }

    /// Returns the wrapped subscriber's view of the current span.
    #[inline]
    pub fn current_span(&self) -> span::Current {
        self.subscriber
            .map(Subscriber::current_span)
            // TODO: this would be more correct as "unknown", so perhaps
            // `tracing-core` should make `Current::unknown()` public?
            .unwrap_or_else(span::Current::none)
    }

    /// Returns whether the wrapped subscriber would enable the current span.
    #[inline]
    pub fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.subscriber
            .map(|subscriber| subscriber.enabled(metadata))
            // If this context is `None`, we are registering a callsite, so
            // return `true` so that the layer does not incorrectly assume that
            // the inner subscriber has disabled this metadata.
            // TODO(eliza): would it be more correct for this to return an `Option`?
            .unwrap_or(true)
    }

    /// Records the provided `event` with the wrapped subscriber.
    ///
    /// # Notes
    ///
    /// - The subscriber is free to expect that the event's callsite has been
    ///   [registered][register], and may panic or fail to observe the event if this is
    ///   not the case. The `tracing` crate's macros ensure that all events are
    ///   registered, but if the event is constructed through other means, the
    ///   user is responsible for ensuring that [`register_callsite`][register]
    ///   has been called prior to calling this method.
    /// - This does _not_ call [`enabled`] on the inner subscriber. If the
    ///   caller wishes to apply the wrapped subscriber's filter before choosing
    ///   whether to record the event, it may first call [`Context::enabled`] to
    ///   check whether the event would be enabled. This allows `Layer`s to
    ///   elide constructing the event if it would not be recorded.
    ///
    /// [register]: tracing_core::subscriber::Subscriber::register_callsite()
    /// [`enabled`]: tracing_core::subscriber::Subscriber::enabled()
    /// [`Context::enabled`]: Context::enabled()
    #[inline]
    pub fn event(&self, event: &Event<'_>) {
        if let Some(subscriber) = self.subscriber {
            subscriber.event(event);
        }
    }

    /// Returns a [`SpanRef`] for the parent span of the given [`Event`], if
    /// it has a parent.
    ///
    /// If the event has an explicitly overridden parent, this method returns
    /// a reference to that span. If the event's parent is the current span,
    /// this returns a reference to the current span, if there is one. If this
    /// returns `None`, then either the event's parent was explicitly set to
    /// `None`, or the event's parent was defined contextually, but no span
    /// is currently entered.
    ///
    /// Compared to [`Context::current_span`] and [`Context::lookup_current`],
    /// this respects overrides provided by the [`Event`].
    ///
    /// Compared to [`Event::parent`], this automatically falls back to the contextual
    /// span, if required.
    ///
    /// ```rust
    /// use tracing::{Event, Subscriber};
    /// use tracing_subscriber::{
    ///     layer::{Context, Layer},
    ///     prelude::*,
    ///     registry::LookupSpan,
    /// };
    ///
    /// struct PrintingLayer;
    /// impl<S> Layer<S> for PrintingLayer
    /// where
    ///     S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    /// {
    ///     fn on_event(&self, event: &Event, ctx: Context<S>) {
    ///         let span = ctx.event_span(event);
    ///         println!("Event in span: {:?}", span.map(|s| s.name()));
    ///     }
    /// }
    ///
    /// tracing::subscriber::with_default(tracing_subscriber::registry().with(PrintingLayer), || {
    ///     tracing::info!("no span");
    ///     // Prints: Event in span: None
    ///
    ///     let span = tracing::info_span!("span");
    ///     tracing::info!(parent: &span, "explicitly specified");
    ///     // Prints: Event in span: Some("span")
    ///
    ///     let _guard = span.enter();
    ///     tracing::info!("contextual span");
    ///     // Prints: Event in span: Some("span")
    /// });
    /// ```
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///     <strong>Note</strong>: This requires the wrapped subscriber to
    ///     implement the <a href="../registry/trait.LookupSpan.html"><code>
    ///     LookupSpan</code></a> trait. See the documentation on
    ///     <a href="./struct.Context.html"><code>Context</code>'s
    ///     declaration</a> for details.
    /// </pre>
    #[inline]
    pub fn event_span(&self, event: &Event<'_>) -> Option<SpanRef<'_, S>>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        if event.is_root() {
            None
        } else if event.is_contextual() {
            self.lookup_current()
        } else {
            // TODO(eliza): this should handle parent IDs
            event.parent().and_then(|id| self.span(id))
        }
    }

    /// Returns metadata for the span with the given `id`, if it exists.
    ///
    /// If this returns `None`, then no span exists for that ID (either it has
    /// closed or the ID is invalid).
    #[inline]
    pub fn metadata(&self, id: &span::Id) -> Option<&'static Metadata<'static>>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        let span = self.span(id)?;
        Some(span.metadata())
    }

    /// Returns [stored data] for the span with the given `id`, if it exists.
    ///
    /// If this returns `None`, then no span exists for that ID (either it has
    /// closed or the ID is invalid).
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///     <strong>Note</strong>: This requires the wrapped subscriber to
    ///     implement the <a href="../registry/trait.LookupSpan.html"><code>
    ///     LookupSpan</code></a> trait. See the documentation on
    ///     <a href="./struct.Context.html"><code>Context</code>'s
    ///     declaration</a> for details.
    /// </pre>
    ///
    /// [stored data]: crate::registry::SpanRef
    #[inline]
    pub fn span(&self, id: &span::Id) -> Option<registry::SpanRef<'_, S>>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        let span = self.subscriber.as_ref()?.span(id)?;

        #[cfg(all(feature = "registry", feature = "std"))]
        return span.try_with_filter(self.filter);

        #[cfg(not(feature = "registry"))]
        Some(span)
    }

    /// Returns `true` if an active span exists for the given `Id`.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///     <strong>Note</strong>: This requires the wrapped subscriber to
    ///     implement the <a href="../registry/trait.LookupSpan.html"><code>
    ///     LookupSpan</code></a> trait. See the documentation on
    ///     <a href="./struct.Context.html"><code>Context</code>'s
    ///     declaration</a> for details.
    /// </pre>
    #[inline]
    pub fn exists(&self, id: &span::Id) -> bool
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        self.subscriber.as_ref().and_then(|s| s.span(id)).is_some()
    }

    /// Returns [stored data] for the span that the wrapped subscriber considers
    /// to be the current.
    ///
    /// If this returns `None`, then we are not currently within a span.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///     <strong>Note</strong>: This requires the wrapped subscriber to
    ///     implement the <a href="../registry/trait.LookupSpan.html"><code>
    ///     LookupSpan</code></a> trait. See the documentation on
    ///     <a href="./struct.Context.html"><code>Context</code>'s
    ///     declaration</a> for details.
    /// </pre>
    ///
    /// [stored data]: crate::registry::SpanRef
    #[inline]
    pub fn lookup_current(&self) -> Option<registry::SpanRef<'_, S>>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        let subscriber = *self.subscriber.as_ref()?;
        let current = subscriber.current_span();
        let id = current.id()?;
        let span = subscriber.span(id);
        debug_assert!(
            span.is_some(),
            "the subscriber should have data for the current span ({:?})!",
            id,
        );

        // If we found a span, and our per-layer filter enables it, return that
        // span!
        #[cfg(all(feature = "registry", feature = "std"))]
        {
            if let Some(span) = span?.try_with_filter(self.filter) {
                Some(span)
            } else {
                // Otherwise, the span at the *top* of the stack is disabled by
                // per-layer filtering, but there may be additional spans in the stack.
                //
                // Currently, `LookupSpan` doesn't have a nice way of exposing access to
                // the whole span stack. However, if we can downcast the innermost
                // subscriber to a a `Registry`, we can iterate over its current span
                // stack.
                //
                // TODO(eliza): when https://github.com/tokio-rs/tracing/issues/1459 is
                // implemented, change this to use that instead...
                self.lookup_current_filtered(subscriber)
            }
        }

        #[cfg(not(feature = "registry"))]
        span
    }

    /// Slow path for when the current span is disabled by PLF and we have a
    /// registry.
    // This is called by `lookup_current` in the case that per-layer filtering
    // is in use. `lookup_current` is allowed to be inlined, but this method is
    // factored out to prevent the loop and (potentially-recursive) subscriber
    // downcasting from being inlined if `lookup_current` is inlined.
    #[inline(never)]
    #[cfg(all(feature = "registry", feature = "std"))]
    fn lookup_current_filtered<'lookup>(
        &self,
        subscriber: &'lookup S,
    ) -> Option<registry::SpanRef<'lookup, S>>
    where
        S: LookupSpan<'lookup>,
    {
        let registry = (subscriber as &dyn Subscriber).downcast_ref::<Registry>()?;
        registry
            .span_stack()
            .iter()
            .find_map(|id| subscriber.span(id)?.try_with_filter(self.filter))
    }

    /// Returns an iterator over the [stored data] for all the spans in the
    /// current context, starting with the specified span and ending with the
    /// root of the trace tree.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    /// <strong>Note</strong>: This returns the spans in reverse order (from leaf to root). Use
    /// <a href="../registry/struct.Scope.html#method.from_root"><code>Scope::from_root</code></a>
    /// in case root-to-leaf ordering is desired.
    /// </pre>
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///     <strong>Note</strong>: This requires the wrapped subscriber to
    ///     implement the <a href="../registry/trait.LookupSpan.html"><code>
    ///     LookupSpan</code></a> trait. See the documentation on
    ///     <a href="./struct.Context.html"><code>Context</code>'s
    ///     declaration</a> for details.
    /// </pre>
    ///
    /// [stored data]: crate::registry::SpanRef
    pub fn span_scope(&self, id: &span::Id) -> Option<registry::Scope<'_, S>>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        Some(self.span(id)?.scope())
    }

    /// Returns an iterator over the [stored data] for all the spans in the
    /// current context, starting with the parent span of the specified event,
    /// and ending with the root of the trace tree and ending with the current span.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    /// <strong>Note</strong>: Compared to <a href="#method.scope"><code>scope</code></a> this
    /// returns the spans in reverse order (from leaf to root). Use
    /// <a href="../registry/struct.Scope.html#method.from_root"><code>Scope::from_root</code></a>
    /// in case root-to-leaf ordering is desired.
    /// </pre>
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    ///     <strong>Note</strong>: This requires the wrapped subscriber to
    ///     implement the <a href="../registry/trait.LookupSpan.html"><code>
    ///     LookupSpan</code></a> trait. See the documentation on
    ///     <a href="./struct.Context.html"><code>Context</code>'s
    ///     declaration</a> for details.
    /// </pre>
    ///
    /// [stored data]: crate::registry::SpanRef
    pub fn event_scope(&self, event: &Event<'_>) -> Option<registry::Scope<'_, S>>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        Some(self.event_span(event)?.scope())
    }

    #[cfg(all(feature = "registry", feature = "std"))]
    pub(crate) fn with_filter(self, filter: FilterId) -> Self {
        // If we already have our own `FilterId`, combine it with the provided
        // one. That way, the new `FilterId` will consider a span to be disabled
        // if it was disabled by the given `FilterId` *or* any `FilterId`s for
        // layers "above" us in the stack.
        //
        // See the doc comment for `FilterId::and` for details.
        let filter = self.filter.and(filter);
        Self { filter, ..self }
    }

    #[cfg(all(feature = "registry", feature = "std"))]
    pub(crate) fn is_enabled_for(&self, span: &span::Id, filter: FilterId) -> bool
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        self.is_enabled_inner(span, filter).unwrap_or(false)
    }

    #[cfg(all(feature = "registry", feature = "std"))]
    pub(crate) fn if_enabled_for(self, span: &span::Id, filter: FilterId) -> Option<Self>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        if self.is_enabled_inner(span, filter)? {
            Some(self.with_filter(filter))
        } else {
            None
        }
    }

    #[cfg(all(feature = "registry", feature = "std"))]
    fn is_enabled_inner(&self, span: &span::Id, filter: FilterId) -> Option<bool>
    where
        S: for<'lookup> LookupSpan<'lookup>,
    {
        Some(self.span(span)?.is_enabled_for(filter))
    }
}

impl<S> Context<'_, S> {
    pub(crate) fn none() -> Self {
        Self {
            subscriber: None,

            #[cfg(feature = "registry")]
            filter: FilterId::none(),
        }
    }
}

impl<S> Clone for Context<'_, S> {
    #[inline]
    fn clone(&self) -> Self {
        let subscriber = self.subscriber.as_ref().copied();
        Context {
            subscriber,

            #[cfg(all(feature = "registry", feature = "std"))]
            filter: self.filter,
        }
    }
}
