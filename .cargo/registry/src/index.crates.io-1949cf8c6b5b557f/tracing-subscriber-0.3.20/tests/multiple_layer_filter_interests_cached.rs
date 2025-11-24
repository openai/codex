#![cfg(feature = "registry")]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{Level, Subscriber};
use tracing_mock::{expect, layer};
use tracing_subscriber::{filter, prelude::*};

#[test]
fn multiple_layer_filter_interests_are_cached() {
    // This layer will return Interest::always for INFO and lower.
    let seen_info = Arc::new(Mutex::new(HashMap::new()));
    let seen_info2 = seen_info.clone();
    let filter = filter::filter_fn(move |meta| {
        *seen_info
            .lock()
            .unwrap()
            .entry(*meta.level())
            .or_insert(0usize) += 1;
        meta.level() <= &Level::INFO
    });
    let seen_info = seen_info2;

    let (info_layer, info_handle) = layer::named("info")
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();
    let info_layer = info_layer.with_filter(filter);

    // This layer will return Interest::always for WARN and lower.
    let seen_warn = Arc::new(Mutex::new(HashMap::new()));
    let seen_warn2 = seen_warn.clone();
    let filter = filter::filter_fn(move |meta| {
        *seen_warn
            .lock()
            .unwrap()
            .entry(*meta.level())
            .or_insert(0usize) += 1;
        meta.level() <= &Level::WARN
    });
    let seen_warn = seen_warn2;

    let (warn_layer, warn_handle) = layer::named("warn")
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();
    let warn_layer = warn_layer.with_filter(filter);

    let subscriber = tracing_subscriber::registry()
        .with(warn_layer)
        .with(info_layer);
    assert!(subscriber.max_level_hint().is_none());

    let _subscriber = subscriber.set_default();

    fn events() {
        tracing::trace!("hello trace");
        tracing::debug!("hello debug");
        tracing::info!("hello info");
        tracing::warn!("hello warn");
        tracing::error!("hello error");
    }

    events();
    {
        let lock = seen_info.lock().unwrap();
        for (&level, &count) in lock.iter() {
            if level == Level::INFO {
                continue;
            }
            assert_eq!(
                count, 1,
                "level {:?} should have been seen 1 time by the INFO layer (after first set of events)",
                level
            );
        }

        let lock = seen_warn.lock().unwrap();
        for (&level, &count) in lock.iter() {
            if level == Level::INFO {
                continue;
            }
            assert_eq!(
                count, 1,
                "level {:?} should have been seen 1 time by the WARN layer (after first set of events)",
                level
            );
        }
    }

    events();
    {
        let lock = seen_info.lock().unwrap();
        for (&level, &count) in lock.iter() {
            if level == Level::INFO {
                continue;
            }
            assert_eq!(
                count, 1,
                "level {:?} should have been seen 1 time by the INFO layer (after second set of events)",
                level
            );
        }

        let lock = seen_warn.lock().unwrap();
        for (&level, &count) in lock.iter() {
            if level == Level::INFO {
                continue;
            }
            assert_eq!(
                count, 1,
                "level {:?} should have been seen 1 time by the WARN layer (after second set of events)",
                level
            );
        }
    }

    info_handle.assert_finished();
    warn_handle.assert_finished();
}
