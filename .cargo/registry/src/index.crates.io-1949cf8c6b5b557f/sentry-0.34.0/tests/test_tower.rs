#![cfg(feature = "test")]

use std::sync::Arc;

use sentry::{
    protocol::{Breadcrumb, Level},
    test::TestTransport,
    ClientOptions, Hub,
};
use sentry_tower::SentryLayer;
use tower::{ServiceBuilder, ServiceExt};

#[test]
fn test_tower_hub() {
    // Create a fake transport for new hubs
    let transport = TestTransport::new();
    let opts = ClientOptions {
        dsn: Some("https://public@sentry.invalid/1".parse().unwrap()),
        transport: Some(Arc::new(transport.clone())),
        ..Default::default()
    };

    let events = sentry::test::with_captured_events(|| {
        // This breadcrumb should be in all subsequent requests
        sentry::add_breadcrumb(Breadcrumb {
            message: Some("Starting service...".to_owned()),
            level: Level::Info,
            ..Default::default()
        });
        sentry::capture_message("Started service", Level::Info);

        #[allow(clippy::redundant_closure)]
        let hub = Arc::new(Hub::with(|hub| Hub::new_from_top(hub)));
        hub.bind_client(Some(Arc::new(opts.into())));

        let service = ServiceBuilder::new()
            .layer(SentryLayer::new(hub))
            .service_fn(|req: String| async move {
                // This breadcrumb should not be seen in any other hub
                sentry::add_breadcrumb(Breadcrumb {
                    message: Some(format!("Got request with arg: {req}")),
                    level: Level::Info,
                    ..Default::default()
                });
                sentry::capture_message("Request failed", Level::Error);
                Err::<(), _>(format!("Can't greet {req}, sorry."))
            });

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let res = rt.block_on(service.oneshot("World".to_owned()));

        assert_eq!(res, Err("Can't greet World, sorry.".to_owned()));
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();
    assert_eq!(event.message, Some("Started service".into()));
    assert_eq!(event.breadcrumbs.len(), 1);
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("Starting service...".into())
    );

    let events = transport.fetch_and_clear_events();
    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();
    assert_eq!(event.message, Some("Request failed".into()));
    assert_eq!(event.breadcrumbs.len(), 2);
    assert_eq!(
        event.breadcrumbs[0].message,
        Some("Starting service...".into())
    );
    assert_eq!(
        event.breadcrumbs[1].message,
        Some("Got request with arg: World".into())
    );
}
