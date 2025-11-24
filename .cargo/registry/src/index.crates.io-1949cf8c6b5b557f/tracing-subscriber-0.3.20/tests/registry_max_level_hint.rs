#![cfg(all(feature = "registry", feature = "fmt"))]
use tracing_subscriber::{filter::LevelFilter, prelude::*};

#[test]
fn registry_sets_max_level_hint() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(LevelFilter::DEBUG)
        .init();
    assert_eq!(LevelFilter::current(), LevelFilter::DEBUG);
}
