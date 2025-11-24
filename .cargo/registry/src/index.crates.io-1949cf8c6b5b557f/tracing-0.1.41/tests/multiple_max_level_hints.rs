#![cfg(feature = "std")]

use tracing::Level;
use tracing_mock::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn multiple_max_level_hints() {
    // This test ensures that when multiple subscribers are active, their max
    // level hints are handled correctly. The global max level should be the
    // maximum of the level filters returned by the two `Subscriber`'s
    // `max_level_hint` method.
    //
    // In this test, we create a subscriber whose max level is `INFO`, and
    // another whose max level is `DEBUG`. We then add an assertion to both of
    // those subscribers' `enabled` method that no metadata for `TRACE` spans or
    // events are filtered, since they are disabled by the global max filter.

    fn do_events() {
        tracing::info!("doing a thing that you might care about");
        tracing::debug!("charging turboencabulator with interocitor");
        tracing::warn!("extremely serious warning, pay attention");
        tracing::trace!("interocitor charge level is 10%");
        tracing::error!("everything is on fire");
    }

    let (subscriber1, handle1) = subscriber::mock()
        .named("subscriber1")
        .with_max_level_hint(Level::INFO)
        .with_filter(|meta| {
            let level = dbg!(meta.level());
            assert!(
                level <= &Level::DEBUG,
                "a TRACE event was dynamically filtered by subscriber1"
            );
            level <= &Level::INFO
        })
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();
    let (subscriber2, handle2) = subscriber::mock()
        .named("subscriber2")
        .with_max_level_hint(Level::DEBUG)
        .with_filter(|meta| {
            let level = dbg!(meta.level());
            assert!(
                level <= &Level::DEBUG,
                "a TRACE event was dynamically filtered by subscriber2"
            );
            level <= &Level::DEBUG
        })
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();

    let dispatch1 = tracing::Dispatch::new(subscriber1);

    tracing::dispatcher::with_default(&dispatch1, do_events);
    handle1.assert_finished();

    let dispatch2 = tracing::Dispatch::new(subscriber2);
    tracing::dispatcher::with_default(&dispatch2, do_events);
    handle2.assert_finished();
}
