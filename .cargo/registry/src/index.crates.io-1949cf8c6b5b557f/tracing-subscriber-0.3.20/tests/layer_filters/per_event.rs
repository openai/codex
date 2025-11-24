use tracing::Level;
use tracing_mock::{expect, layer};
use tracing_subscriber::{field::Visit, layer::Filter, prelude::*};

struct FilterEvent;

impl<S> Filter<S> for FilterEvent {
    fn enabled(
        &self,
        _meta: &tracing::Metadata<'_>,
        _cx: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        true
    }

    fn event_enabled(
        &self,
        event: &tracing::Event<'_>,
        _cx: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        struct ShouldEnable(bool);
        impl Visit for ShouldEnable {
            fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
                if field.name() == "enable" {
                    self.0 = value;
                }
            }

            fn record_debug(
                &mut self,
                _field: &tracing_core::Field,
                _value: &dyn core::fmt::Debug,
            ) {
            }
        }
        let mut should_enable = ShouldEnable(false);
        event.record(&mut should_enable);
        should_enable.0
    }
}

#[test]
fn per_layer_event_field_filtering() {
    let (expect, handle) = layer::mock()
        .event(expect::event().at_level(Level::TRACE))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(expect.with_filter(FilterEvent))
        .set_default();

    tracing::trace!(enable = true, "hello trace");
    tracing::debug!("hello debug");
    tracing::info!(enable = true, "hello info");
    tracing::warn!(enable = false, "hello warn");
    tracing::error!("hello error");

    handle.assert_finished();
}
