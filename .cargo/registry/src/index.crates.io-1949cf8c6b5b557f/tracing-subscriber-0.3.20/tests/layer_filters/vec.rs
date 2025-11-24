use super::*;
use tracing::Subscriber;
use tracing_mock::{expect, layer::MockLayer};

#[test]
fn with_filters_unboxed() {
    let (trace_layer, trace_handle) = layer::named("trace")
        .event(expect::event().at_level(Level::TRACE))
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let trace_layer = trace_layer.with_filter(LevelFilter::TRACE);

    let (debug_layer, debug_handle) = layer::named("debug")
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let debug_layer = debug_layer.with_filter(LevelFilter::DEBUG);

    let (info_layer, info_handle) = layer::named("info")
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let info_layer = info_layer.with_filter(LevelFilter::INFO);

    let _subscriber = tracing_subscriber::registry()
        .with(vec![trace_layer, debug_layer, info_layer])
        .set_default();

    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!("hello info");

    trace_handle.assert_finished();
    debug_handle.assert_finished();
    info_handle.assert_finished();
}

#[test]
fn with_filters_boxed() {
    let (unfiltered_layer, unfiltered_handle) = layer::named("unfiltered")
        .event(expect::event().at_level(Level::TRACE))
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let unfiltered_layer = unfiltered_layer.boxed();

    let (debug_layer, debug_handle) = layer::named("debug")
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let debug_layer = debug_layer.with_filter(LevelFilter::DEBUG).boxed();

    let (target_layer, target_handle) = layer::named("target")
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let target_layer = target_layer
        .with_filter(filter::filter_fn(|meta| meta.target() == "my_target"))
        .boxed();

    let _subscriber = tracing_subscriber::registry()
        .with(vec![unfiltered_layer, debug_layer, target_layer])
        .set_default();

    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!(target: "my_target", "hello my target");

    unfiltered_handle.assert_finished();
    debug_handle.assert_finished();
    target_handle.assert_finished();
}

#[test]
fn mixed_max_level_hint() {
    let unfiltered = layer::named("unfiltered").run().boxed();
    let info = layer::named("info")
        .run()
        .with_filter(LevelFilter::INFO)
        .boxed();
    let debug = layer::named("debug")
        .run()
        .with_filter(LevelFilter::DEBUG)
        .boxed();

    let subscriber = tracing_subscriber::registry().with(vec![unfiltered, info, debug]);

    assert_eq!(subscriber.max_level_hint(), None);
}

#[test]
fn all_filtered_max_level_hint() {
    let warn = layer::named("warn")
        .run()
        .with_filter(LevelFilter::WARN)
        .boxed();
    let info = layer::named("info")
        .run()
        .with_filter(LevelFilter::INFO)
        .boxed();
    let debug = layer::named("debug")
        .run()
        .with_filter(LevelFilter::DEBUG)
        .boxed();

    let subscriber = tracing_subscriber::registry().with(vec![warn, info, debug]);

    assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::DEBUG));
}

#[test]
fn empty_vec() {
    // Just a None means everything is off
    let subscriber = tracing_subscriber::registry().with(Vec::<MockLayer>::new());
    assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::OFF));
}
