#![cfg(feature = "registry")]
use tracing::level_filters::LevelFilter;
use tracing::Subscriber;
use tracing_subscriber::prelude::*;

#[test]
fn just_empty_vec() {
    // Just a None means everything is off
    let subscriber = tracing_subscriber::registry().with(Vec::<LevelFilter>::new());
    assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::OFF));
}

#[test]
fn layer_and_empty_vec() {
    let subscriber = tracing_subscriber::registry()
        .with(LevelFilter::INFO)
        .with(Vec::<LevelFilter>::new());
    assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::INFO));
}
