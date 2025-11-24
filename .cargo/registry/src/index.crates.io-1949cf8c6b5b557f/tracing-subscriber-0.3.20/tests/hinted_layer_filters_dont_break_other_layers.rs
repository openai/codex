#![cfg(feature = "registry")]
use tracing::{Level, Metadata, Subscriber};
use tracing_mock::{
    expect, layer,
    layer::MockLayer,
    subscriber::{self},
};
use tracing_subscriber::{filter::DynFilterFn, layer::Context, prelude::*};

#[test]
fn layer_filters() {
    let (unfiltered, unfiltered_handle) = unfiltered("unfiltered");
    let (filtered, filtered_handle) = filtered("filtered");

    let subscriber = tracing_subscriber::registry()
        .with(unfiltered)
        .with(filtered.with_filter(filter()));
    assert_eq!(subscriber.max_level_hint(), None);
    let _subscriber = subscriber.set_default();

    events();

    unfiltered_handle.assert_finished();
    filtered_handle.assert_finished();
}

#[test]
fn layered_layer_filters() {
    let (unfiltered1, unfiltered1_handle) = unfiltered("unfiltered_1");
    let (unfiltered2, unfiltered2_handle) = unfiltered("unfiltered_2");
    let unfiltered = unfiltered1.and_then(unfiltered2);

    let (filtered1, filtered1_handle) = filtered("filtered_1");
    let (filtered2, filtered2_handle) = filtered("filtered_2");
    let filtered = filtered1
        .with_filter(filter())
        .and_then(filtered2.with_filter(filter()));

    let subscriber = tracing_subscriber::registry()
        .with(unfiltered)
        .with(filtered);
    assert_eq!(subscriber.max_level_hint(), None);
    let _subscriber = subscriber.set_default();

    events();

    unfiltered1_handle.assert_finished();
    unfiltered2_handle.assert_finished();
    filtered1_handle.assert_finished();
    filtered2_handle.assert_finished();
}

#[test]
fn out_of_order() {
    let (unfiltered1, unfiltered1_handle) = unfiltered("unfiltered_1");
    let (unfiltered2, unfiltered2_handle) = unfiltered("unfiltered_2");

    let (filtered1, filtered1_handle) = filtered("filtered_1");
    let (filtered2, filtered2_handle) = filtered("filtered_2");

    let subscriber = tracing_subscriber::registry()
        .with(unfiltered1)
        .with(filtered1.with_filter(filter()))
        .with(unfiltered2)
        .with(filtered2.with_filter(filter()));
    assert_eq!(subscriber.max_level_hint(), None);
    let _subscriber = subscriber.set_default();

    events();

    unfiltered1_handle.assert_finished();
    unfiltered2_handle.assert_finished();
    filtered1_handle.assert_finished();
    filtered2_handle.assert_finished();
}

#[test]
fn mixed_layered() {
    let (unfiltered1, unfiltered1_handle) = unfiltered("unfiltered_1");
    let (unfiltered2, unfiltered2_handle) = unfiltered("unfiltered_2");
    let (filtered1, filtered1_handle) = filtered("filtered_1");
    let (filtered2, filtered2_handle) = filtered("filtered_2");

    let layered1 = filtered1.with_filter(filter()).and_then(unfiltered1);
    let layered2 = unfiltered2.and_then(filtered2.with_filter(filter()));

    let subscriber = tracing_subscriber::registry().with(layered1).with(layered2);
    assert_eq!(subscriber.max_level_hint(), None);
    let _subscriber = subscriber.set_default();

    events();

    unfiltered1_handle.assert_finished();
    unfiltered2_handle.assert_finished();
    filtered1_handle.assert_finished();
    filtered2_handle.assert_finished();
}

fn events() {
    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!("hello info");
    tracing::warn!("hello warn");
    tracing::error!("hello error");
}

fn filter<S>() -> DynFilterFn<S> {
    DynFilterFn::new(
        (|metadata: &Metadata<'_>, _: &tracing_subscriber::layer::Context<'_, S>| {
            metadata.level() <= &Level::INFO
        }) as fn(&Metadata<'_>, &Context<'_, S>) -> bool,
    )
    .with_max_level_hint(Level::INFO)
}

fn unfiltered(name: &str) -> (MockLayer, subscriber::MockHandle) {
    layer::named(name)
        .event(expect::event().at_level(Level::TRACE))
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle()
}

fn filtered(name: &str) -> (MockLayer, subscriber::MockHandle) {
    layer::named(name)
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle()
}
