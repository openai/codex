#![cfg(feature = "fmt")]
use tracing_subscriber::filter::LevelFilter;

#[test]
fn fmt_sets_max_level_hint() {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();
    assert_eq!(LevelFilter::current(), LevelFilter::DEBUG);
}
