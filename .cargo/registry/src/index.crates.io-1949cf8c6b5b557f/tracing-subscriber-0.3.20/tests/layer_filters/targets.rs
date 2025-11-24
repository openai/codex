use super::*;
use tracing_subscriber::{
    filter::{filter_fn, Targets},
    prelude::*,
};

#[test]
#[cfg_attr(not(feature = "tracing-log"), ignore)]
fn log_events() {
    // Reproduces https://github.com/tokio-rs/tracing/issues/1563
    mod inner {
        pub(super) const MODULE_PATH: &str = module_path!();

        #[tracing::instrument]
        pub(super) fn logs() {
            log::debug!("inner");
        }
    }

    let filter = Targets::new()
        .with_default(LevelFilter::DEBUG)
        .with_target(inner::MODULE_PATH, LevelFilter::WARN);

    let layer =
        tracing_subscriber::layer::Identity::new().with_filter(filter_fn(move |_meta| true));

    let _guard = tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .set_default();

    inner::logs();
}

#[test]
fn inner_layer_short_circuits() {
    // This test ensures that when a global filter short-circuits `Interest`
    // evaluation, we aren't left with a "dirty" per-layer filter state.

    let (layer, handle) = layer::mock()
        .event(expect::event().with_fields(expect::msg("hello world")))
        .only()
        .run_with_handle();

    let filter = Targets::new().with_target("magic_target", LevelFilter::DEBUG);

    let _guard = tracing_subscriber::registry()
        // Note: we don't just use a `LevelFilter` for the global filter here,
        // because it will just return a max level filter, and the chain of
        // `register_callsite` calls that would trigger the bug never happens...
        .with(filter::filter_fn(|meta| meta.level() <= &Level::INFO))
        .with(layer.with_filter(filter))
        .set_default();

    tracing::debug!("skip me please!");
    tracing::info!(target: "magic_target", "hello world");

    handle.assert_finished();
}
