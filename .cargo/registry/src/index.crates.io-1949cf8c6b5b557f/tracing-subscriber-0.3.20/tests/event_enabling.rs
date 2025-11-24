#![cfg(feature = "registry")]

use std::sync::{Arc, Mutex};
use tracing::{subscriber::with_default, Event, Metadata, Subscriber};
use tracing_subscriber::{layer::Context, prelude::*, registry, Layer};

struct TrackingLayer {
    enabled: bool,
    event_enabled_count: Arc<Mutex<usize>>,
    event_enabled: bool,
    on_event_count: Arc<Mutex<usize>>,
}

impl<C> Layer<C> for TrackingLayer
where
    C: Subscriber + Send + Sync + 'static,
{
    fn enabled(&self, _metadata: &Metadata<'_>, _ctx: Context<'_, C>) -> bool {
        self.enabled
    }

    fn event_enabled(&self, _event: &Event<'_>, _ctx: Context<'_, C>) -> bool {
        *self.event_enabled_count.lock().unwrap() += 1;
        self.event_enabled
    }

    fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, C>) {
        *self.on_event_count.lock().unwrap() += 1;
    }
}

#[test]
fn event_enabled_is_only_called_once() {
    let event_enabled_count = Arc::new(Mutex::default());
    let count = event_enabled_count.clone();
    let subscriber = registry().with(TrackingLayer {
        enabled: true,
        event_enabled_count,
        event_enabled: true,
        on_event_count: Arc::new(Mutex::default()),
    });
    with_default(subscriber, || {
        tracing::error!("hiya!");
    });

    assert_eq!(1, *count.lock().unwrap());
}

#[test]
fn event_enabled_not_called_when_not_enabled() {
    let event_enabled_count = Arc::new(Mutex::default());
    let count = event_enabled_count.clone();
    let subscriber = registry().with(TrackingLayer {
        enabled: false,
        event_enabled_count,
        event_enabled: true,
        on_event_count: Arc::new(Mutex::default()),
    });
    with_default(subscriber, || {
        tracing::error!("hiya!");
    });

    assert_eq!(0, *count.lock().unwrap());
}

#[test]
fn event_disabled_does_disable_event() {
    let on_event_count = Arc::new(Mutex::default());
    let count = on_event_count.clone();
    let subscriber = registry().with(TrackingLayer {
        enabled: true,
        event_enabled_count: Arc::new(Mutex::default()),
        event_enabled: false,
        on_event_count,
    });
    with_default(subscriber, || {
        tracing::error!("hiya!");
    });

    assert_eq!(0, *count.lock().unwrap());
}
