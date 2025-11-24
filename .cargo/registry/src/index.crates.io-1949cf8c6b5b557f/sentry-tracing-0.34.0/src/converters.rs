use std::collections::BTreeMap;
use std::error::Error;

use sentry_core::protocol::{Event, Exception, Mechanism, Thread, Value};
use sentry_core::{event_from_error, Breadcrumb, Level, TransactionOrSpan};
use tracing_core::field::{Field, Visit};
use tracing_core::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::layer::SentrySpanData;

/// Converts a [`tracing_core::Level`] to a Sentry [`Level`]
fn convert_tracing_level(level: &tracing_core::Level) -> Level {
    match level {
        &tracing_core::Level::TRACE | &tracing_core::Level::DEBUG => Level::Debug,
        &tracing_core::Level::INFO => Level::Info,
        &tracing_core::Level::WARN => Level::Warning,
        &tracing_core::Level::ERROR => Level::Error,
    }
}

fn level_to_exception_type(level: &tracing_core::Level) -> &'static str {
    match *level {
        tracing_core::Level::TRACE => "tracing::trace!",
        tracing_core::Level::DEBUG => "tracing::debug!",
        tracing_core::Level::INFO => "tracing::info!",
        tracing_core::Level::WARN => "tracing::warn!",
        tracing_core::Level::ERROR => "tracing::error!",
    }
}

/// Extracts the message and metadata from an event
/// and also optionally from its spans chain.
fn extract_event_data(event: &tracing_core::Event) -> (Option<String>, FieldVisitor) {
    // Find message of the event, if any
    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);
    let message = visitor
        .json_values
        .remove("message")
        // When #[instrument(err)] is used the event does not have a message attached to it.
        // the error message is attached to the field "error".
        .or_else(|| visitor.json_values.remove("error"))
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        });

    (message, visitor)
}

fn extract_event_data_with_context<S>(
    event: &tracing_core::Event,
    ctx: Option<Context<S>>,
) -> (Option<String>, FieldVisitor)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, mut visitor) = extract_event_data(event);

    // Add the context fields of every parent span.
    let current_span = ctx.as_ref().and_then(|ctx| {
        event
            .parent()
            .and_then(|id| ctx.span(id))
            .or_else(|| ctx.lookup_current())
    });
    if let Some(span) = current_span {
        for span in span.scope() {
            let name = span.name();
            let ext = span.extensions();
            if let Some(span_data) = ext.get::<SentrySpanData>() {
                match &span_data.sentry_span {
                    TransactionOrSpan::Span(span) => {
                        for (key, value) in span.data().iter() {
                            if key != "message" {
                                let key = format!("{}:{}", name, key);
                                visitor.json_values.insert(key, value.clone());
                            }
                        }
                    }
                    TransactionOrSpan::Transaction(transaction) => {
                        for (key, value) in transaction.data().iter() {
                            if key != "message" {
                                let key = format!("{}:{}", name, key);
                                visitor.json_values.insert(key, value.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    (message, visitor)
}

/// Records all fields of [`tracing_core::Event`] for easy access
#[derive(Default)]
pub(crate) struct FieldVisitor {
    pub json_values: BTreeMap<String, Value>,
    pub exceptions: Vec<Exception>,
}

impl FieldVisitor {
    fn record<T: Into<Value>>(&mut self, field: &Field, value: T) {
        self.json_values
            .insert(field.name().to_owned(), value.into());
    }
}

impl Visit for FieldVisitor {
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

    fn record_error(&mut self, _field: &Field, value: &(dyn Error + 'static)) {
        let event = event_from_error(value);
        for exception in event.exception {
            self.exceptions.push(exception);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record(field, format!("{value:?}"));
    }
}

/// Creates a [`Breadcrumb`] from a given [`tracing_core::Event`]
pub fn breadcrumb_from_event(event: &tracing_core::Event) -> Breadcrumb {
    let (message, visitor) = extract_event_data(event);
    Breadcrumb {
        category: Some(event.metadata().target().to_owned()),
        ty: "log".into(),
        level: convert_tracing_level(event.metadata().level()),
        message,
        data: visitor.json_values,
        ..Default::default()
    }
}

fn tags_from_event(fields: &mut BTreeMap<String, Value>) -> BTreeMap<String, String> {
    let mut tags = BTreeMap::new();

    fields.retain(|key, value| {
        let Some(key) = key.strip_prefix("tags.") else {
            return true;
        };
        let string = match value {
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => std::mem::take(s),
            // remove null entries since empty tags are not allowed
            Value::Null => return false,
            // keep entries that cannot be represented as simple string
            Value::Array(_) | Value::Object(_) => return true,
        };

        tags.insert(key.to_owned(), string);

        false
    });

    tags
}

fn contexts_from_event(
    event: &tracing_core::Event,
    fields: BTreeMap<String, Value>,
) -> BTreeMap<String, sentry_core::protocol::Context> {
    let event_meta = event.metadata();
    let mut location_map = BTreeMap::new();
    if let Some(module_path) = event_meta.module_path() {
        location_map.insert("module_path".to_string(), module_path.into());
    }
    if let Some(file) = event_meta.file() {
        location_map.insert("file".to_string(), file.into());
    }
    if let Some(line) = event_meta.line() {
        location_map.insert("line".to_string(), line.into());
    }

    let mut context = BTreeMap::new();
    if !fields.is_empty() {
        context.insert(
            "Rust Tracing Fields".to_string(),
            sentry_core::protocol::Context::Other(fields),
        );
    }
    if !location_map.is_empty() {
        context.insert(
            "Rust Tracing Location".to_string(),
            sentry_core::protocol::Context::Other(location_map),
        );
    }
    context
}

/// Creates an [`Event`] from a given [`tracing_core::Event`]
pub fn event_from_event<'context, S>(
    event: &tracing_core::Event,
    ctx: impl Into<Option<Context<'context, S>>>,
) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (message, mut visitor) = extract_event_data_with_context(event, ctx.into());

    Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        tags: tags_from_event(&mut visitor.json_values),
        contexts: contexts_from_event(event, visitor.json_values),
        ..Default::default()
    }
}

/// Creates an exception [`Event`] from a given [`tracing_core::Event`]
pub fn exception_from_event<'context, S>(
    event: &tracing_core::Event,
    ctx: impl Into<Option<Context<'context, S>>>,
) -> Event<'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // Exception records in Sentry need a valid type, value and full stack trace to support
    // proper grouping and issue metadata generation. tracing_core::Record does not contain sufficient
    // information for this. However, it may contain a serialized error which we can parse to emit
    // an exception record.
    let (mut message, visitor) = extract_event_data_with_context(event, ctx.into());
    let FieldVisitor {
        mut exceptions,
        mut json_values,
    } = visitor;

    // If there are both a message and an exception, then add the message as synthetic wrapper
    // around the exception to support proper grouping. If configured, also add the current stack
    // trace to this exception directly, since it points to the place where the exception is
    // captured.
    if !exceptions.is_empty() && message.is_some() {
        #[allow(unused_mut)]
        let mut thread = Thread::default();

        #[cfg(feature = "backtrace")]
        if let Some(client) = sentry_core::Hub::current().client() {
            if client.options().attach_stacktrace {
                thread = sentry_backtrace::current_thread(true);
            }
        }

        let exception = Exception {
            ty: level_to_exception_type(event.metadata().level()).to_owned(),
            value: message.take(),
            module: event.metadata().module_path().map(str::to_owned),
            stacktrace: thread.stacktrace,
            raw_stacktrace: thread.raw_stacktrace,
            thread_id: thread.id,
            mechanism: Some(Mechanism {
                synthetic: Some(true),
                ..Mechanism::default()
            }),
        };

        exceptions.push(exception);
    }

    if let Some(exception) = exceptions.last_mut() {
        "tracing".clone_into(
            &mut exception
                .mechanism
                .get_or_insert_with(Mechanism::default)
                .ty,
        );
    }

    Event {
        logger: Some(event.metadata().target().to_owned()),
        level: convert_tracing_level(event.metadata().level()),
        message,
        exception: exceptions.into(),
        tags: tags_from_event(&mut json_values),
        contexts: contexts_from_event(event, json_values),
        ..Default::default()
    }
}
