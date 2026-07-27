use super::Context;
use super::OTelSdkResult;
use super::OtelProvider;
use super::SdkTracerProvider;
use super::Span;
use super::SpanData;
use super::SpanProcessor;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Debug, Default)]
struct ShutdownState {
    force_flushes: AtomicUsize,
    shutdowns: AtomicUsize,
}

#[derive(Debug)]
struct ControlledSpanProcessor {
    state: Arc<ShutdownState>,
}

impl SpanProcessor for ControlledSpanProcessor {
    fn on_start(&self, _span: &mut Span, _context: &Context) {}

    fn on_end(&self, _span: SpanData) {}

    fn force_flush(&self) -> OTelSdkResult {
        self.state
            .force_flushes
            .fetch_add(/*val*/ 1, Ordering::Relaxed);
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        self.state.shutdowns.fetch_add(/*val*/ 1, Ordering::Relaxed);
        Ok(())
    }
}

fn test_provider() -> (OtelProvider, Arc<ShutdownState>) {
    let state = Arc::new(ShutdownState::default());
    let processor = ControlledSpanProcessor {
        state: Arc::clone(&state),
    };
    let tracer_provider = SdkTracerProvider::builder()
        .with_span_processor(processor)
        .build();

    (
        OtelProvider {
            logger: None,
            tracer_provider: Some(tracer_provider),
            tracer: None,
            metrics: None,
            shutdown_started: AtomicBool::default(),
        },
        state,
    )
}

#[test]
fn explicit_shutdown_and_drop_shut_down_exporters_once_without_force_flush() {
    let (provider, state) = test_provider();

    provider.shutdown();
    provider.shutdown();
    drop(provider);

    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}

#[test]
fn drop_shuts_down_exporters_without_force_flush() {
    let (provider, state) = test_provider();

    drop(provider);

    assert_eq!(state.shutdowns.load(Ordering::Relaxed), 1);
    assert_eq!(state.force_flushes.load(Ordering::Relaxed), 0);
}
