use std::cell::RefCell;
use std::collections::BTreeMap;

use sentry_core::protocol::Value;
use sentry_core::{Breadcrumb, TransactionOrSpan};
use tracing_core::field::Visit;
use tracing_core::{span, Event, Field, Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::converters::*;

/// The action that Sentry should perform for a [`Metadata`]
#[derive(Debug, Clone, Copy)]
pub enum EventFilter {
    /// Ignore the [`Event`]
    Ignore,
    /// Create a [`Breadcrumb`] from this [`Event`]
    Breadcrumb,
    /// Create a message [`sentry_core::protocol::Event`] from this [`Event`]
    Event,
    /// Create an exception [`sentry_core::protocol::Event`] from this [`Event`]
    Exception,
}

/// The type of data Sentry should ingest for a [`Event`]
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EventMapping {
    /// Ignore the [`Event`]
    Ignore,
    /// Adds the [`Breadcrumb`] to the Sentry scope.
    Breadcrumb(Breadcrumb),
    /// Captures the [`sentry_core::protocol::Event`] to Sentry.
    Event(sentry_core::protocol::Event<'static>),
}

/// The default event filter.
///
/// By default, an exception event is captured for `error`, a breadcrumb for
/// `warning` and `info`, and `debug` and `trace` logs are ignored.
pub fn default_event_filter(metadata: &Metadata) -> EventFilter {
    match metadata.level() {
        &Level::ERROR => EventFilter::Exception,
        &Level::WARN | &Level::INFO => EventFilter::Breadcrumb,
        &Level::DEBUG | &Level::TRACE => EventFilter::Ignore,
    }
}

/// The default span filter.
///
/// By default, spans at the `error`, `warning`, and `info`
/// levels are captured
pub fn default_span_filter(metadata: &Metadata) -> bool {
    matches!(
        metadata.level(),
        &Level::ERROR | &Level::WARN | &Level::INFO
    )
}

type EventMapper<S> = Box<dyn Fn(&Event, Context<'_, S>) -> EventMapping + Send + Sync>;

/// Provides a tracing layer that dispatches events to sentry
pub struct SentryLayer<S> {
    event_filter: Box<dyn Fn(&Metadata) -> EventFilter + Send + Sync>,
    event_mapper: Option<EventMapper<S>>,

    span_filter: Box<dyn Fn(&Metadata) -> bool + Send + Sync>,

    with_span_attributes: bool,
}

impl<S> SentryLayer<S> {
    /// Sets a custom event filter function.
    ///
    /// The filter classifies how sentry should handle [`Event`]s based
    /// on their [`Metadata`].
    #[must_use]
    pub fn event_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Metadata) -> EventFilter + Send + Sync + 'static,
    {
        self.event_filter = Box::new(filter);
        self
    }

    /// Sets a custom event mapper function.
    ///
    /// The mapper is responsible for creating either breadcrumbs or events from
    /// [`Event`]s.
    #[must_use]
    pub fn event_mapper<F>(mut self, mapper: F) -> Self
    where
        F: Fn(&Event, Context<'_, S>) -> EventMapping + Send + Sync + 'static,
    {
        self.event_mapper = Some(Box::new(mapper));
        self
    }

    /// Sets a custom span filter function.
    ///
    /// The filter classifies whether sentry should handle [`tracing::Span`]s based
    /// on their [`Metadata`].
    ///
    /// [`tracing::Span`]: https://docs.rs/tracing/latest/tracing/struct.Span.html
    #[must_use]
    pub fn span_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Metadata) -> bool + Send + Sync + 'static,
    {
        self.span_filter = Box::new(filter);
        self
    }

    /// Enable every parent span's attributes to be sent along with own event's attributes.
    ///
    /// Note that the root span is considered a [transaction][sentry_core::protocol::Transaction]
    /// so its context will only be grabbed only if you set the transaction to be sampled.
    /// The most straightforward way to do this is to set
    /// the [traces_sample_rate][sentry_core::ClientOptions::traces_sample_rate] to `1.0`
    /// while configuring your sentry client.
    #[must_use]
    pub fn enable_span_attributes(mut self) -> Self {
        self.with_span_attributes = true;
        self
    }
}

impl<S> Default for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn default() -> Self {
        Self {
            event_filter: Box::new(default_event_filter),
            event_mapper: None,

            span_filter: Box::new(default_span_filter),

            with_span_attributes: false,
        }
    }
}

/// Data that is attached to the tracing Spans `extensions`, in order to
/// `finish` the corresponding sentry span `on_close`, and re-set its parent as
/// the *current* span.
pub(super) struct SentrySpanData {
    pub(super) sentry_span: TransactionOrSpan,
    parent_sentry_span: Option<TransactionOrSpan>,
}

impl<S> Layer<S> for SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event, ctx: Context<'_, S>) {
        let item = match &self.event_mapper {
            Some(mapper) => mapper(event, ctx),
            None => {
                let span_ctx = self.with_span_attributes.then_some(ctx);
                match (self.event_filter)(event.metadata()) {
                    EventFilter::Ignore => EventMapping::Ignore,
                    EventFilter::Breadcrumb => {
                        EventMapping::Breadcrumb(breadcrumb_from_event(event))
                    }
                    EventFilter::Event => EventMapping::Event(event_from_event(event, span_ctx)),
                    EventFilter::Exception => {
                        EventMapping::Event(exception_from_event(event, span_ctx))
                    }
                }
            }
        };

        match item {
            EventMapping::Event(event) => {
                sentry_core::capture_event(event);
            }
            EventMapping::Breadcrumb(breadcrumb) => sentry_core::add_breadcrumb(breadcrumb),
            _ => (),
        }
    }

    /// When a new Span gets created, run the filter and start a new sentry span
    /// if it passes, setting it as the *current* sentry span.
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        if !(self.span_filter)(span.metadata()) {
            return;
        }

        let (description, data) = extract_span_data(attrs);
        let op = span.name();

        // Spans don't always have a description, this ensures our data is not empty,
        // therefore the Sentry UI will be a lot more valuable for navigating spans.
        let description = description.unwrap_or_else(|| {
            let target = span.metadata().target();
            if target.is_empty() {
                op.to_string()
            } else {
                format!("{target}::{op}")
            }
        });

        let parent_sentry_span = sentry_core::configure_scope(|s| s.get_span());
        let sentry_span: sentry_core::TransactionOrSpan = match &parent_sentry_span {
            Some(parent) => parent.start_child(op, &description).into(),
            None => {
                let ctx = sentry_core::TransactionContext::new(&description, op);
                sentry_core::start_transaction(ctx).into()
            }
        };
        // Add the data from the original span to the sentry span.
        // This comes from typically the `fields` in `tracing::instrument`.
        for (key, value) in data {
            sentry_span.set_data(key, value);
        }

        sentry_core::configure_scope(|scope| scope.set_span(Some(sentry_span.clone())));

        let mut extensions = span.extensions_mut();
        extensions.insert(SentrySpanData {
            sentry_span,
            parent_sentry_span,
        });
    }

    /// When a span gets closed, finish the underlying sentry span, and set back
    /// its parent as the *current* sentry span.
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(&id) {
            Some(span) => span,
            None => return,
        };

        let mut extensions = span.extensions_mut();
        let SentrySpanData {
            sentry_span,
            parent_sentry_span,
        } = match extensions.remove::<SentrySpanData>() {
            Some(data) => data,
            None => return,
        };

        sentry_span.finish();
        sentry_core::configure_scope(|scope| scope.set_span(parent_sentry_span));
    }

    /// Implement the writing of extra data to span
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let span = match ctx.span(span) {
            Some(s) => s,
            _ => return,
        };

        let mut extensions = span.extensions_mut();
        let span = match extensions.get_mut::<SentrySpanData>() {
            Some(t) => &t.sentry_span,
            _ => return,
        };

        let mut data = FieldVisitor::default();
        values.record(&mut data);

        for (key, value) in data.json_values {
            span.set_data(&key, value);
        }
    }
}

/// Creates a default Sentry layer
pub fn layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Default::default()
}

/// Extracts the message and attributes from a span
fn extract_span_data(attrs: &span::Attributes) -> (Option<String>, BTreeMap<&'static str, Value>) {
    let mut json_values = VISITOR_BUFFER.with_borrow_mut(|debug_buffer| {
        let mut visitor = SpanFieldVisitor {
            debug_buffer,
            json_values: Default::default(),
        };
        attrs.record(&mut visitor);
        visitor.json_values
    });

    // Find message of the span, if any
    let message = json_values.remove("message").and_then(|v| match v {
        Value::String(s) => Some(s),
        _ => None,
    });

    (message, json_values)
}

thread_local! {
    static VISITOR_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Records all span fields into a `BTreeMap`, reusing a mutable `String` as buffer.
struct SpanFieldVisitor<'s> {
    debug_buffer: &'s mut String,
    json_values: BTreeMap<&'static str, Value>,
}

impl SpanFieldVisitor<'_> {
    fn record<T: Into<Value>>(&mut self, field: &Field, value: T) {
        self.json_values.insert(field.name(), value.into());
    }
}

impl Visit for SpanFieldVisitor<'_> {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        self.debug_buffer.reserve(128);
        write!(self.debug_buffer, "{value:?}").unwrap();
        self.json_values
            .insert(field.name(), self.debug_buffer.as_str().into());
        self.debug_buffer.clear();
    }
}
