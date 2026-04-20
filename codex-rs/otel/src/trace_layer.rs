//! Reloadable OpenTelemetry trace layer.
//!
//! This layer stays registered with `tracing-subscriber` while allowing callers
//! to replace the OTel tracer behind it. It is used by long-lived processes that
//! discover their effective telemetry config after subscriber initialization.

use crate::OtelProvider;
use opentelemetry_sdk::trace::Tracer;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::Event;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing::span::Record;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

type TraceLayer<S> = tracing_opentelemetry::OpenTelemetryLayer<S, Tracer>;
type ReplaceProviderFn = dyn Fn(Option<&OtelProvider>) + Send + Sync;

thread_local! {
    static ENTERED_SPANS: RefCell<Vec<Id>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Default)]
pub struct OtelTraceLayer<S> {
    state: Arc<RwLock<OtelTraceLayerState<S>>>,
}

struct OtelTraceLayerState<S> {
    current: Option<Arc<TraceLayer<S>>>,
    retired: Vec<Arc<TraceLayer<S>>>,
    active_spans: HashMap<Id, Arc<TraceLayer<S>>>,
}

impl<S> OtelTraceLayerState<S> {
    fn prune_inactive_retired(&mut self) {
        let active_layers = self
            .active_spans
            .values()
            .cloned()
            .collect::<Vec<Arc<TraceLayer<S>>>>();
        self.retired.retain(|retired| {
            active_layers
                .iter()
                .any(|active| Arc::ptr_eq(active, retired))
        });
    }
}

impl<S> Default for OtelTraceLayerState<S> {
    fn default() -> Self {
        Self {
            current: None,
            retired: Vec::new(),
            active_spans: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct OtelTraceLayerHandle {
    replace_provider: Arc<ReplaceProviderFn>,
    restore_provider: Arc<ReplaceProviderFn>,
    shutdown: Arc<dyn Fn() + Send + Sync>,
}

impl OtelTraceLayerHandle {
    pub fn replace_provider(&self, provider: Option<&OtelProvider>) {
        (self.replace_provider)(provider);
    }

    pub fn restore_provider(&self, provider: Option<&OtelProvider>) {
        (self.restore_provider)(provider);
    }

    pub fn shutdown(&self) {
        (self.shutdown)();
    }
}

impl<S> OtelTraceLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    pub fn from_provider(provider: Option<&OtelProvider>) -> (Self, OtelTraceLayerHandle) {
        let state = Arc::new(RwLock::new(OtelTraceLayerState {
            current: trace_layer(provider),
            retired: Vec::new(),
            active_spans: HashMap::new(),
        }));
        let layer = Self {
            state: Arc::clone(&state),
        };
        let replace_state = Arc::clone(&state);
        let restore_state = Arc::clone(&state);
        let shutdown_state = Arc::clone(&state);
        let handle = OtelTraceLayerHandle {
            replace_provider: Arc::new(move |provider| {
                replace_trace_layer(&replace_state, trace_layer(provider));
            }),
            restore_provider: Arc::new(move |provider| {
                restore_trace_layer(&restore_state, trace_layer(provider));
            }),
            shutdown: Arc::new(move || {
                let mut state = shutdown_state
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.current = None;
                state.retired.clear();
            }),
        };
        (layer, handle)
    }

    fn current_layer(&self) -> Option<Arc<TraceLayer<S>>> {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current
            .clone()
    }

    fn record_span_layer(&self, id: &Id, layer: Arc<TraceLayer<S>>) {
        self.state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active_spans
            .insert(id.clone(), layer);
    }

    fn layer_for_span(&self, id: &Id) -> Option<Arc<TraceLayer<S>>> {
        let state = self
            .state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .active_spans
            .get(id)
            .cloned()
            .or_else(|| state.current.clone())
    }

    fn remove_span_layer(&self, id: &Id) -> Option<Arc<TraceLayer<S>>> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let layer = state
            .active_spans
            .remove(id)
            .or_else(|| state.current.clone());
        state.prune_inactive_retired();
        layer
    }
}

impl<S> Layer<S> for OtelTraceLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if !OtelProvider::trace_export_filter(attrs.metadata()) {
            return;
        }

        if let Some(layer) = self.current_layer() {
            layer.on_new_span(attrs, id, ctx);
            self.record_span_layer(id, layer);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(layer) = self.layer_for_span(id) {
            layer.on_record(id, values, ctx);
        }
    }

    fn on_follows_from(&self, id: &Id, follows: &Id, ctx: Context<'_, S>) {
        if let Some(layer) = self.layer_for_span(id) {
            layer.on_follows_from(id, follows, ctx);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if !OtelProvider::trace_export_filter(event.metadata()) {
            return;
        }

        let span_layer = event
            .parent()
            .and_then(|id| self.layer_for_span(id))
            .or_else(|| {
                ctx.event_span(event)
                    .and_then(|span| self.layer_for_span(&span.id()))
            });

        if let Some(layer) = span_layer.or_else(|| self.current_layer()) {
            layer.on_event(event, ctx);
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(layer) = self.layer_for_span(id) {
            layer.on_enter(id, ctx);
            ENTERED_SPANS.with(|spans| spans.borrow_mut().push(id.clone()));
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(layer) = self.layer_for_span(id) {
            layer.on_exit(id, ctx);
            ENTERED_SPANS.with(|spans| {
                let mut spans = spans.borrow_mut();
                if let Some(position) = spans.iter().rposition(|entered_id| entered_id == id) {
                    spans.remove(position);
                }
            });
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        ENTERED_SPANS.with(|spans| spans.borrow_mut().retain(|entered_id| entered_id != &id));
        if let Some(layer) = self.remove_span_layer(&id) {
            layer.on_close(id, ctx);
        }
    }

    fn on_id_change(&self, old: &Id, new: &Id, ctx: Context<'_, S>) {
        if let Some(layer) = self.layer_for_span(old) {
            layer.on_id_change(old, new, ctx);
            let mut state = self
                .state
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(layer) = state.active_spans.remove(old) {
                state.active_spans.insert(new.clone(), layer);
            }
        }
        ENTERED_SPANS.with(|spans| {
            for entered_id in spans.borrow_mut().iter_mut() {
                if entered_id == old {
                    *entered_id = new.clone();
                }
            }
        });
    }

    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        let state = self
            .state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entered_layer = ENTERED_SPANS.with(|spans| {
            spans
                .borrow()
                .iter()
                .rev()
                .find_map(|id| state.active_spans.get(id).cloned())
        });
        let layer = entered_layer.or_else(|| {
            let mut active_layers = Vec::new();
            for layer in state.active_spans.values() {
                if !active_layers
                    .iter()
                    .any(|active| Arc::ptr_eq(active, layer))
                {
                    active_layers.push(Arc::clone(layer));
                }
            }
            if active_layers.len() == 1 {
                active_layers.pop()
            } else {
                None
            }
        })?;

        // SAFETY: the selected layer is owned by `active_spans`, and tracing
        // keeps that span alive while `OpenTelemetrySpanExt` performs its
        // downcast. The entered span stack is maintained by this layer rather
        // than `Span::current()` so downcasting never re-enters tracing.
        unsafe { Layer::<S>::downcast_raw(layer.as_ref(), id) }
    }
}

fn replace_trace_layer<S>(
    state: &RwLock<OtelTraceLayerState<S>>,
    next_layer: Option<Arc<TraceLayer<S>>>,
) {
    let mut state = state
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(current) = state.current.take() {
        state.retired.push(current);
    }
    state.current = next_layer;
    state.prune_inactive_retired();
}

fn restore_trace_layer<S>(
    state: &RwLock<OtelTraceLayerState<S>>,
    restored_layer: Option<Arc<TraceLayer<S>>>,
) {
    let mut state = state
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.current = restored_layer;
    state.prune_inactive_retired();
}

fn trace_layer<S>(provider: Option<&OtelProvider>) -> Option<Arc<TraceLayer<S>>>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    provider
        .and_then(|provider| provider.tracer.clone())
        .map(|tracer| Arc::new(tracing_opentelemetry::layer().with_tracer(tracer)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OtelProvider;
    use crate::current_span_trace_id;
    use crate::span_w3c_trace_context;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::error::OTelSdkResult;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use opentelemetry_sdk::trace::SpanData;
    use opentelemetry_sdk::trace::SpanExporter;
    use std::sync::Mutex;
    use tracing::trace_span;
    use tracing_subscriber::prelude::*;

    #[derive(Clone, Debug, Default)]
    struct RecordingSpanExporter {
        spans: Arc<Mutex<Vec<SpanData>>>,
    }

    impl SpanExporter for RecordingSpanExporter {
        async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
            self.spans
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend(batch);
            Ok(())
        }
    }

    fn provider_with_tracer(scope: &'static str) -> (OtelProvider, Arc<Mutex<Vec<SpanData>>>) {
        let exporter = RecordingSpanExporter::default();
        let spans = Arc::clone(&exporter.spans);
        let tracer_provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter)
            .build();
        let tracer = tracer_provider.tracer(scope);
        let provider = OtelProvider {
            logger: None,
            tracer_provider: Some(tracer_provider),
            tracer: Some(tracer),
            metrics: None,
        };
        (provider, spans)
    }

    #[test]
    fn trace_layer_preserves_span_context_downcasting() {
        let tracer_provider = SdkTracerProvider::builder().build();
        let tracer = tracer_provider.tracer("codex-otel-tests");
        let provider = OtelProvider {
            logger: None,
            tracer_provider: Some(tracer_provider),
            tracer: Some(tracer),
            metrics: None,
        };
        let (layer, _handle) = OtelTraceLayer::from_provider(Some(&provider));
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = trace_span!("test_span");
        let _entered = span.enter();
        let trace_id = current_span_trace_id().expect("trace id");

        assert_eq!(trace_id.len(), 32);
        assert!(trace_id.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(trace_id, "00000000000000000000000000000000");
    }

    #[test]
    fn span_closes_with_original_layer_after_provider_replacement() {
        let (first_provider, first_spans) = provider_with_tracer("first");
        let (second_provider, second_spans) = provider_with_tracer("second");
        let (layer, handle) = OtelTraceLayer::from_provider(Some(&first_provider));
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = trace_span!("old_span");
        {
            let _entered = span.enter();
        }
        handle.replace_provider(Some(&second_provider));
        drop(span);

        assert_eq!(
            first_spans
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            1
        );
        assert_eq!(
            second_spans
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            0
        );
    }

    #[test]
    fn span_events_use_original_layer_after_provider_replacement() {
        let (first_provider, first_spans) = provider_with_tracer("first");
        let (layer, handle) = OtelTraceLayer::from_provider(Some(&first_provider));
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = trace_span!("old_span");
        {
            let _entered = span.enter();
            handle.replace_provider(/*provider*/ None);
            tracing::info!(target: "codex_otel.trace_safe.test", "old_span_event");
        }
        drop(span);

        let spans = first_spans
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].events.len(), 1);
    }

    #[test]
    fn downcast_uses_active_span_layer_after_provider_is_disabled() {
        let (provider, _spans) = provider_with_tracer("first");
        let (layer, handle) = OtelTraceLayer::from_provider(Some(&provider));
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = trace_span!("old_span");
        let _entered = span.enter();
        let before_disable = current_span_trace_id().expect("trace id before disable");

        handle.replace_provider(/*provider*/ None);
        let after_disable = current_span_trace_id().expect("trace id after disable");

        assert_eq!(after_disable, before_disable);
    }

    #[test]
    fn downcast_uses_original_layer_after_provider_replacement() {
        let (first_provider, first_spans) = provider_with_tracer("first");
        let (second_provider, second_spans) = provider_with_tracer("second");
        let (layer, handle) = OtelTraceLayer::from_provider(Some(&first_provider));
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = trace_span!("old_span");
        handle.replace_provider(Some(&second_provider));
        let trace = span_w3c_trace_context(&span).expect("trace context after replacement");
        drop(span);

        assert!(trace.traceparent.is_some());
        assert_eq!(
            first_spans
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            1
        );
        assert_eq!(
            second_spans
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            0
        );
    }

    #[test]
    fn downcast_keeps_active_span_layer_after_shutdown() {
        let (provider, _spans) = provider_with_tracer("first");
        let (layer, handle) = OtelTraceLayer::from_provider(Some(&provider));
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = trace_span!("old_span");
        handle.shutdown();
        let trace = span_w3c_trace_context(&span).expect("trace context after shutdown");

        assert!(trace.traceparent.is_some());
    }

    #[test]
    fn restore_provider_reclaims_provisional_layer_without_active_spans() {
        let (first_provider, _first_spans) = provider_with_tracer("first");
        let (second_provider, _second_spans) = provider_with_tracer("second");
        let (layer, handle): (OtelTraceLayer<tracing_subscriber::Registry>, _) =
            OtelTraceLayer::from_provider(Some(&first_provider));

        handle.replace_provider(Some(&second_provider));
        handle.restore_provider(Some(&first_provider));

        let state = layer
            .state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(state.current.is_some());
        assert!(state.retired.is_empty());
    }
}
