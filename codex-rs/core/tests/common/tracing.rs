use anyhow::Context;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::dispatcher::DefaultGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Receives named trace events on a test's current-thread runtime.
pub struct NamedTraceEvents {
    events: tokio::sync::mpsc::UnboundedReceiver<()>,
    _subscriber: DefaultGuard,
}

impl NamedTraceEvents {
    pub fn subscribe(name: &'static str) -> Self {
        struct EventLayer {
            name: &'static str,
            sender: tokio::sync::mpsc::UnboundedSender<()>,
        }
        impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for EventLayer {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _: tracing_subscriber::layer::Context<'_, S>,
            ) {
                if event.metadata().name() == self.name {
                    let _ = self.sender.send(());
                }
            }
        }

        let (sender, events) = tokio::sync::mpsc::unbounded_channel();
        let subscriber = tracing_subscriber::registry()
            .with(EventLayer { name, sender })
            .set_default();
        Self {
            events,
            _subscriber: subscriber,
        }
    }

    pub async fn next(&mut self) -> anyhow::Result<()> {
        tokio::time::timeout(std::time::Duration::from_secs(10), self.events.recv())
            .await
            .context("timed out waiting for trace event")?
            .context("trace collector closed")
    }
}

pub struct TestTracingContext {
    _provider: SdkTracerProvider,
    _guard: DefaultGuard,
}

pub fn install_test_tracing(tracer_name: &str) -> TestTracingContext {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer(tracer_name.to_string());
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));

    TestTracingContext {
        _provider: provider,
        _guard: subscriber.set_default(),
    }
}
