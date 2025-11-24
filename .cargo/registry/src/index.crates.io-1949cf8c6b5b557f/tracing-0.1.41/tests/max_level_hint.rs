use tracing::Level;
use tracing_mock::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn max_level_hints() {
    // This test asserts that when a subscriber provides us with the global
    // maximum level that it will enable (by implementing the
    // `Subscriber::max_level_hint` method), we will never call
    // `Subscriber::enabled` for events above that maximum level.
    //
    // In this case, we test that by making the `enabled` method assert that no
    // `Metadata` for spans or events at the `TRACE` or `DEBUG` levels.
    let (subscriber, handle) = subscriber::mock()
        .with_max_level_hint(Level::INFO)
        .with_filter(|meta| {
            assert!(
                dbg!(meta).level() <= &Level::INFO,
                "a TRACE or DEBUG event was dynamically filtered: "
            );
            true
        })
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    tracing::info!("doing a thing that you might care about");
    tracing::debug!("charging turboencabulator with interocitor");
    tracing::warn!("extremely serious warning, pay attention");
    tracing::trace!("interocitor charge level is 10%");
    tracing::error!("everything is on fire");
    handle.assert_finished();
}
