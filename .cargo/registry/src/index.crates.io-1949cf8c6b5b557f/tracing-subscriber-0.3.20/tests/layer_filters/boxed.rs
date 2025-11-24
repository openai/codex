use super::*;
use tracing_mock::layer::MockLayer;
use tracing_subscriber::{filter, prelude::*, Layer};

fn layer() -> (MockLayer, subscriber::MockHandle) {
    layer::mock().only().run_with_handle()
}

fn filter<S>() -> filter::DynFilterFn<S> {
    // Use dynamic filter fn to disable interest caching and max-level hints,
    // allowing us to put all of these tests in the same file.
    filter::dynamic_filter_fn(|_, _| false)
}

/// reproduces https://github.com/tokio-rs/tracing/issues/1563#issuecomment-921363629
#[test]
fn box_works() {
    let (layer, handle) = layer();
    let layer = Box::new(layer.with_filter(filter()));

    let _guard = tracing_subscriber::registry().with(layer).set_default();

    for i in 0..2 {
        tracing::info!(i);
    }

    handle.assert_finished();
}

/// the same as `box_works` but with a type-erased `Box`.
#[test]
fn dyn_box_works() {
    let (layer, handle) = layer();
    let layer: Box<dyn Layer<_> + Send + Sync + 'static> = Box::new(layer.with_filter(filter()));

    let _guard = tracing_subscriber::registry().with(layer).set_default();

    for i in 0..2 {
        tracing::info!(i);
    }

    handle.assert_finished();
}
