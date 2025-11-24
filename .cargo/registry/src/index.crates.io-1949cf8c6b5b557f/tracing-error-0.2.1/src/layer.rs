use std::any::{type_name, TypeId};
use std::fmt;
use std::marker::PhantomData;
use tracing::{span, Dispatch, Metadata, Subscriber};
use tracing_subscriber::fmt::format::{DefaultFields, FormatFields};
use tracing_subscriber::{
    fmt::FormattedFields,
    layer::{self, Layer},
    registry::LookupSpan,
};

/// A subscriber [`Layer`] that enables capturing [`SpanTrace`]s.
///
/// Optionally, this type may be constructed with a [field formatter] to use
/// when formatting the fields of each span in a trace. When no formatter is
/// provided, the [default format] is used instead.
///
/// [`Layer`]: tracing_subscriber::layer::Layer
/// [`SpanTrace`]: super::SpanTrace
/// [field formatter]: tracing_subscriber::fmt::FormatFields
/// [default format]: tracing_subscriber::fmt::format::DefaultFields
pub struct ErrorLayer<S, F = DefaultFields> {
    format: F,

    get_context: WithContext,
    _subscriber: PhantomData<fn(S)>,
}

// this function "remembers" the types of the subscriber and the formatter,
// so that we can downcast to something aware of them without knowing those
// types at the callsite.
pub(crate) struct WithContext(
    fn(&Dispatch, &span::Id, f: &mut dyn FnMut(&'static Metadata<'static>, &str) -> bool),
);

impl<S, F> Layer<S> for ErrorLayer<S, F>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    F: for<'writer> FormatFields<'writer> + 'static,
{
    /// Notifies this layer that a new span was constructed with the given
    /// `Attributes` and `Id`.
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: layer::Context<'_, S>) {
        let span = ctx.span(id).expect("span must already exist!");
        if span.extensions().get::<FormattedFields<F>>().is_some() {
            return;
        }
        let mut fields = FormattedFields::<F>::new(String::new());
        if self.format.format_fields(fields.as_writer(), attrs).is_ok() {
            span.extensions_mut().insert(fields);
        }
    }

    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        match id {
            id if id == TypeId::of::<Self>() => Some(self as *const _ as *const ()),
            id if id == TypeId::of::<WithContext>() => {
                Some(&self.get_context as *const _ as *const ())
            }
            _ => None,
        }
    }
}

impl<S, F> ErrorLayer<S, F>
where
    F: for<'writer> FormatFields<'writer> + 'static,
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    /// Returns a new `ErrorLayer` with the provided [field formatter].
    ///
    /// [field formatter]: tracing_subscriber::fmt::FormatFields
    pub fn new(format: F) -> Self {
        Self {
            format,
            get_context: WithContext(Self::get_context),
            _subscriber: PhantomData,
        }
    }

    fn get_context(
        dispatch: &Dispatch,
        id: &span::Id,
        f: &mut dyn FnMut(&'static Metadata<'static>, &str) -> bool,
    ) {
        let subscriber = dispatch
            .downcast_ref::<S>()
            .expect("subscriber should downcast to expected type; this is a bug!");
        let span = subscriber
            .span(id)
            .expect("registry should have a span for the current ID");
        for span in span.scope() {
            let cont = if let Some(fields) = span.extensions().get::<FormattedFields<F>>() {
                f(span.metadata(), fields.fields.as_str())
            } else {
                f(span.metadata(), "")
            };
            if !cont {
                break;
            }
        }
    }
}

impl WithContext {
    pub(crate) fn with_context(
        &self,
        dispatch: &Dispatch,
        id: &span::Id,
        mut f: impl FnMut(&'static Metadata<'static>, &str) -> bool,
    ) {
        (self.0)(dispatch, id, &mut f)
    }
}

impl<S> Default for ErrorLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn default() -> Self {
        Self::new(DefaultFields::default())
    }
}

impl<S, F: fmt::Debug> fmt::Debug for ErrorLayer<S, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorLayer")
            .field("format", &self.format)
            .field("subscriber", &format_args!("{}", type_name::<S>()))
            .finish()
    }
}
