//! Reloadable OpenTelemetry log layer.
//!
//! This layer stays registered with `tracing-subscriber` while allowing callers
//! to replace the OTel logger provider behind it. It is used by long-lived
//! processes that discover their effective telemetry config after subscriber
//! initialization.

use crate::OtelProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use std::sync::Arc;
use std::sync::RwLock;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

type LoggerBridge = OpenTelemetryTracingBridge<
    SdkLoggerProvider,
    <SdkLoggerProvider as opentelemetry::logs::LoggerProvider>::Logger,
>;

#[derive(Clone, Default)]
pub struct OtelLoggerLayer {
    bridge: Arc<RwLock<Option<LoggerBridge>>>,
}

impl OtelLoggerLayer {
    pub fn from_provider(provider: Option<&OtelProvider>) -> Self {
        Self {
            bridge: Arc::new(RwLock::new(logger_bridge(provider))),
        }
    }

    pub fn replace_provider(&self, provider: Option<&OtelProvider>) {
        let mut bridge = self
            .bridge
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *bridge = logger_bridge(provider);
    }
}

impl<S> Layer<S> for OtelLoggerLayer
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        if !OtelProvider::log_export_filter(event.metadata()) {
            return;
        }

        let bridge = self
            .bridge
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(bridge) = bridge.as_ref() {
            bridge.on_event(event, ctx);
        }
    }
}

fn logger_bridge(provider: Option<&OtelProvider>) -> Option<LoggerBridge> {
    provider
        .and_then(|provider| provider.logger.as_ref())
        .map(OpenTelemetryTracingBridge::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tracing::Subscriber;
    use tracing_subscriber::prelude::*;

    #[derive(Clone, Default)]
    struct CountingLayer {
        events: Arc<Mutex<usize>>,
    }

    impl<S> Layer<S> for CountingLayer
    where
        S: Subscriber,
    {
        fn on_event(&self, _event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut events = self
                .events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *events += 1;
        }
    }

    #[test]
    fn non_otel_events_continue_to_reach_other_layers() {
        let counting_layer = CountingLayer::default();
        let events = Arc::clone(&counting_layer.events);
        let subscriber = tracing_subscriber::registry()
            .with(counting_layer)
            .with(OtelLoggerLayer::default());
        let _guard = tracing::subscriber::set_default(subscriber);

        tracing::info!(target: "codex_core::unrelated", "visible to other layers");

        assert_eq!(
            *events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            1
        );
    }
}
