use super::*;
use tracing::Subscriber;
use tracing_subscriber::{
    filter::{self, LevelFilter},
    prelude::*,
    Layer,
};

fn filter_out_everything<S>() -> filter::DynFilterFn<S> {
    // Use dynamic filter fn to disable interest caching and max-level hints,
    // allowing us to put all of these tests in the same file.
    filter::dynamic_filter_fn(|_, _| false)
}

#[test]
fn option_some() {
    let (layer, handle) = layer::mock().only().run_with_handle();
    let layer = layer.with_filter(Some(filter_out_everything()));

    let _guard = tracing_subscriber::registry().with(layer).set_default();

    for i in 0..2 {
        tracing::info!(i);
    }

    handle.assert_finished();
}

#[test]
fn option_none() {
    let (layer, handle) = layer::mock()
        .event(expect::event())
        .event(expect::event())
        .only()
        .run_with_handle();
    let layer = layer.with_filter(None::<filter::DynFilterFn<_>>);

    let _guard = tracing_subscriber::registry().with(layer).set_default();

    for i in 0..2 {
        tracing::info!(i);
    }

    handle.assert_finished();
}

#[test]
fn option_mixed() {
    let (layer, handle) = layer::mock()
        .event(expect::event())
        .only()
        .run_with_handle();
    let layer = layer
        .with_filter(filter::dynamic_filter_fn(|meta, _ctx| {
            meta.target() == "interesting"
        }))
        .with_filter(None::<filter::DynFilterFn<_>>);

    let _guard = tracing_subscriber::registry().with(layer).set_default();

    tracing::info!(target: "interesting", x="foo");
    tracing::info!(target: "boring", x="bar");

    handle.assert_finished();
}

/// The lack of a max level hint from a `None` filter should result in no max
/// level hint when combined with other filters/layer.
#[test]
fn none_max_level_hint() {
    let (layer_some, handle_none) = layer::mock()
        .event(expect::event())
        .event(expect::event())
        .only()
        .run_with_handle();
    let subscribe_none = layer_some.with_filter(None::<filter::DynFilterFn<_>>);
    assert!(subscribe_none.max_level_hint().is_none());

    let (layer_filter_fn, handle_filter_fn) = layer::mock()
        .event(expect::event())
        .only()
        .run_with_handle();
    let max_level = Level::INFO;
    let layer_filter_fn = layer_filter_fn.with_filter(
        filter::dynamic_filter_fn(move |meta, _| meta.level() <= &max_level)
            .with_max_level_hint(max_level),
    );
    assert_eq!(layer_filter_fn.max_level_hint(), Some(LevelFilter::INFO));

    let subscriber = tracing_subscriber::registry()
        .with(subscribe_none)
        .with(layer_filter_fn);
    // The absence of a hint from the `None` filter upgrades the `INFO` hint
    // from the filter fn layer.
    assert!(subscriber.max_level_hint().is_none());

    let _guard = subscriber.set_default();
    tracing::info!(target: "interesting", x="foo");
    tracing::debug!(target: "sometimes_interesting", x="bar");

    handle_none.assert_finished();
    handle_filter_fn.assert_finished();
}

/// The max level hint from inside a `Some(filter)` filter should be propagated
/// and combined with other filters/layers.
#[test]
fn some_max_level_hint() {
    let (layer_some, handle_some) = layer::mock()
        .event(expect::event())
        .event(expect::event())
        .only()
        .run_with_handle();
    let layer_some = layer_some.with_filter(Some(
        filter::dynamic_filter_fn(move |meta, _| meta.level() <= &Level::DEBUG)
            .with_max_level_hint(Level::DEBUG),
    ));
    assert_eq!(layer_some.max_level_hint(), Some(LevelFilter::DEBUG));

    let (layer_filter_fn, handle_filter_fn) = layer::mock()
        .event(expect::event())
        .only()
        .run_with_handle();
    let layer_filter_fn = layer_filter_fn.with_filter(
        filter::dynamic_filter_fn(move |meta, _| meta.level() <= &Level::INFO)
            .with_max_level_hint(Level::INFO),
    );
    assert_eq!(layer_filter_fn.max_level_hint(), Some(LevelFilter::INFO));

    let subscriber = tracing_subscriber::registry()
        .with(layer_some)
        .with(layer_filter_fn);
    // The `DEBUG` hint from the `Some` filter upgrades the `INFO` hint from the
    // filter fn layer.
    assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::DEBUG));

    let _guard = subscriber.set_default();
    tracing::info!(target: "interesting", x="foo");
    tracing::debug!(target: "sometimes_interesting", x="bar");

    handle_some.assert_finished();
    handle_filter_fn.assert_finished();
}
