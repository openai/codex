use super::*;
use tracing_subscriber::{
    filter::{filter_fn, FilterExt, LevelFilter},
    prelude::*,
};

#[test]
fn and() {
    let (layer, handle) = layer::mock()
        .event(
            event::msg("a very interesting event")
                .at_level(tracing::Level::INFO)
                .with_target("interesting_target"),
        )
        .only()
        .run_with_handle();

    // Enables spans and events with targets starting with `interesting_target`:
    let target_filter = filter::filter_fn(|meta| meta.target().starts_with("interesting_target"));

    // Enables spans and events with levels `INFO` and below:
    let level_filter = LevelFilter::INFO;

    // Combine the two filters together, returning a filter that only enables
    // spans and events that *both* filters will enable:
    let filter = target_filter.and(level_filter);

    let _subscriber = tracing_subscriber::registry()
        .with(layer.with_filter(filter))
        .set_default();

    // This event will *not* be enabled:
    tracing::info!("an event with an uninteresting target");

    // This event *will* be enabled:
    tracing::info!(target: "interesting_target", "a very interesting event");

    // This event will *not* be enabled:
    tracing::debug!(target: "interesting_target", "interesting debug event...");

    handle.assert_finished();
}
