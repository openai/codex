#![cfg(feature = "registry")]
mod boxed;
mod downcast_raw;
mod filter_scopes;
mod option;
mod per_event;
mod targets;
mod trees;
mod vec;

use tracing::{level_filters::LevelFilter, Level};
use tracing_mock::{expect, layer, subscriber};
use tracing_subscriber::{filter, prelude::*, Layer};

#[test]
fn basic_layer_filters() {
    let (trace_layer, trace_handle) = layer::named("trace")
        .event(expect::event().at_level(Level::TRACE))
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let (debug_layer, debug_handle) = layer::named("debug")
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let (info_layer, info_handle) = layer::named("info")
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(trace_layer.with_filter(LevelFilter::TRACE))
        .with(debug_layer.with_filter(LevelFilter::DEBUG))
        .with(info_layer.with_filter(LevelFilter::INFO))
        .set_default();

    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!("hello info");

    trace_handle.assert_finished();
    debug_handle.assert_finished();
    info_handle.assert_finished();
}

#[test]
fn basic_layer_filter_spans() {
    let (trace_layer, trace_handle) = layer::named("trace")
        .new_span(expect::span().at_level(Level::TRACE))
        .new_span(expect::span().at_level(Level::DEBUG))
        .new_span(expect::span().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let (debug_layer, debug_handle) = layer::named("debug")
        .new_span(expect::span().at_level(Level::DEBUG))
        .new_span(expect::span().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let (info_layer, info_handle) = layer::named("info")
        .new_span(expect::span().at_level(Level::INFO))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(trace_layer.with_filter(LevelFilter::TRACE))
        .with(debug_layer.with_filter(LevelFilter::DEBUG))
        .with(info_layer.with_filter(LevelFilter::INFO))
        .set_default();

    tracing::trace_span!("hello trace");
    tracing::debug_span!("hello debug");
    tracing::info_span!("hello info");

    trace_handle.assert_finished();
    debug_handle.assert_finished();
    info_handle.assert_finished();
}

#[test]
fn global_filters_subscribers_still_work() {
    let (expect, handle) = layer::mock()
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(expect)
        .with(LevelFilter::INFO)
        .set_default();

    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!("hello info");
    tracing::warn!("hello warn");
    tracing::error!("hello error");

    handle.assert_finished();
}

#[test]
fn global_filter_interests_are_cached() {
    let (expect, handle) = layer::mock()
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(expect.with_filter(filter::filter_fn(|meta| {
            assert!(
                meta.level() <= &Level::INFO,
                "enabled should not be called for callsites disabled by the global filter"
            );
            meta.level() <= &Level::WARN
        })))
        .with(LevelFilter::INFO)
        .set_default();

    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!("hello info");
    tracing::warn!("hello warn");
    tracing::error!("hello error");

    handle.assert_finished();
}

#[test]
fn global_filters_affect_subscriber_filters() {
    let (expect, handle) = layer::named("debug")
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(expect.with_filter(LevelFilter::DEBUG))
        .with(LevelFilter::INFO)
        .set_default();

    tracing::trace!("hello trace");
    tracing::debug!("hello debug");
    tracing::info!("hello info");
    tracing::warn!("hello warn");
    tracing::error!("hello error");

    handle.assert_finished();
}

#[test]
fn filter_fn() {
    let (all, all_handle) = layer::named("all_targets")
        .event(expect::event().with_fields(expect::msg("hello foo")))
        .event(expect::event().with_fields(expect::msg("hello bar")))
        .only()
        .run_with_handle();

    let (foo, foo_handle) = layer::named("foo_target")
        .event(expect::event().with_fields(expect::msg("hello foo")))
        .only()
        .run_with_handle();

    let (bar, bar_handle) = layer::named("bar_target")
        .event(expect::event().with_fields(expect::msg("hello bar")))
        .only()
        .run_with_handle();

    let _subscriber = tracing_subscriber::registry()
        .with(all)
        .with(foo.with_filter(filter::filter_fn(|meta| meta.target().starts_with("foo"))))
        .with(bar.with_filter(filter::filter_fn(|meta| meta.target().starts_with("bar"))))
        .set_default();

    tracing::trace!(target: "foo", "hello foo");
    tracing::trace!(target: "bar", "hello bar");

    foo_handle.assert_finished();
    bar_handle.assert_finished();
    all_handle.assert_finished();
}
