#![cfg(feature = "registry")]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{Level, Subscriber};
use tracing_mock::{layer::MockLayer, *};
use tracing_subscriber::{filter, prelude::*};

#[test]
fn vec_layer_filter_interests_are_cached() {
    let mk_filtered = |level: Level, subscriber: MockLayer| {
        let seen = Arc::new(Mutex::new(HashMap::new()));
        let filter = filter::filter_fn({
            let seen = seen.clone();
            move |meta| {
                *seen.lock().unwrap().entry(*meta.level()).or_insert(0usize) += 1;
                meta.level() <= &level
            }
        });
        (subscriber.with_filter(filter).boxed(), seen)
    };

    // This layer will return Interest::always for INFO and lower.
    let (info_layer, info_handle) = layer::named("info")
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .event(expect::event().at_level(Level::INFO))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();
    let (info_layer, seen_info) = mk_filtered(Level::INFO, info_layer);

    // This layer will return Interest::always for WARN and lower.
    let (warn_layer, warn_handle) = layer::named("warn")
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .event(expect::event().at_level(Level::WARN))
        .event(expect::event().at_level(Level::ERROR))
        .only()
        .run_with_handle();
    let (warn_layer, seen_warn) = mk_filtered(Level::WARN, warn_layer);

    let subscriber = tracing_subscriber::registry().with(vec![warn_layer, info_layer]);
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
                "level {:?} should have been seen 1 time by the INFO subscriber (after first set of events)",
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
                "level {:?} should have been seen 1 time by the WARN subscriber (after first set of events)",
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
                "level {:?} should have been seen 1 time by the INFO subscriber (after second set of events)",
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
                "level {:?} should have been seen 1 time by the WARN subscriber (after second set of events)",
                level
            );
        }
    }

    info_handle.assert_finished();
    warn_handle.assert_finished();
}
