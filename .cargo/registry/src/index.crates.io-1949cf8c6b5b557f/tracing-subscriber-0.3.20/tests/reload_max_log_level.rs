#![cfg(all(feature = "env-filter", feature = "tracing-log"))]

use tracing::{self, Level};
use tracing_mock::{expect, subscriber};
use tracing_subscriber::{filter::LevelFilter, prelude::*, reload};

#[test]
fn reload_max_log_level() {
    let (subscriber, finished) = subscriber::mock()
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::DEBUG))
        .event(expect::event().at_level(Level::INFO))
        .only()
        .run_with_handle();
    let (filter, reload_handle) = reload::Layer::new(LevelFilter::INFO);
    subscriber.with(filter).init();

    assert!(log::log_enabled!(log::Level::Info));
    assert!(!log::log_enabled!(log::Level::Debug));
    assert!(!log::log_enabled!(log::Level::Trace));

    log::debug!("i'm disabled");
    log::info!("i'm enabled");

    reload_handle
        .reload(Level::DEBUG)
        .expect("reloading succeeds");

    assert!(log::log_enabled!(log::Level::Info));
    assert!(log::log_enabled!(log::Level::Debug));
    assert!(!log::log_enabled!(log::Level::Trace));

    log::debug!("i'm enabled now");
    log::info!("i'm still enabled, too");

    finished.assert_finished();
}
