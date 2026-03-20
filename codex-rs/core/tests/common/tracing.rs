use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::dispatcher::DefaultGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
