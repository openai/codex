#![cfg(feature = "test")]

#[test]
fn test_log() {
    let logger = sentry_log::SentryLogger::new();

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();

    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        log::info!("Hello World!");
        log::error!("Shit's on fire yo");
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
}

#[test]
fn test_slog() {
    let drain = sentry_slog::SentryDrain::new(slog::Discard);
    let root = slog::Logger::root(drain, slog::o!());

    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
        });

        slog::info!(root, "Hello World!");
        slog::error!(root, "Shit's on fire yo");
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(event.tags["worker"], "worker1");
    assert_eq!(event.level, sentry::Level::Error);
    assert_eq!(event.breadcrumbs[0].level, sentry::Level::Info);
    assert_eq!(event.breadcrumbs[0].message, Some("Hello World!".into()));
}
