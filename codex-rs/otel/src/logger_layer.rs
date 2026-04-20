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
use tracing::subscriber::Interest;
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
    fn register_callsite(&self, metadata: &'static tracing::Metadata<'static>) -> Interest {
        if OtelProvider::log_export_filter(metadata) {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, metadata: &tracing::Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        if !OtelProvider::log_export_filter(metadata) {
            return false;
        }

        self.bridge
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

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
