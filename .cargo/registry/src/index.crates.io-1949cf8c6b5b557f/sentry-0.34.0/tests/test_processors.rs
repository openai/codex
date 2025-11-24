#![cfg(feature = "test")]
#![allow(clippy::unnecessary_wraps)]
use std::sync::Arc;

#[test]
fn test_event_processors() {
    let events = sentry::test::with_captured_events(|| {
        sentry::configure_scope(|scope| {
            scope.set_tag("worker", "worker1");
            scope.add_event_processor(|mut event| {
                event.user = Some(sentry::User {
                    email: Some("foo@example.com".into()),
                    ..Default::default()
                });
                Some(event)
            });
        });
        sentry::capture_message("Hello World!", sentry::Level::Warning);
    });

    assert_eq!(events.len(), 1);
    let event = events.into_iter().next().unwrap();

    assert_eq!(
        event.user,
        Some(sentry::User {
            email: Some("foo@example.com".into()),
            ..Default::default()
        })
    );
}

#[test]
fn test_before_callbacks() {
    fn before_send(
        mut evt: sentry::protocol::Event<'static>,
    ) -> Option<sentry::protocol::Event<'static>> {
        evt.logger = Some("muh_logger".into());
        Some(evt)
    }

    fn before_breadcrumb(mut crumb: sentry::Breadcrumb) -> Option<sentry::Breadcrumb> {
        crumb.message = Some(format!("{} aha!", crumb.message.unwrap()));
        Some(crumb)
    }

    let events = sentry::test::with_captured_events_options(
        || {
            sentry::add_breadcrumb(sentry::Breadcrumb {
                message: Some("Testing".into()),
                ..Default::default()
            });
            sentry::capture_message("Hello World!", sentry::Level::Warning);
        },
        sentry::ClientOptions {
            before_send: Some(Arc::new(Box::new(before_send))),
            before_breadcrumb: Some(Arc::new(Box::new(before_breadcrumb))),
            ..Default::default()
        },
    );

    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.logger.as_ref().unwrap(), "muh_logger");
    assert_eq!(
        event.breadcrumbs[0].message.as_ref().unwrap(),
        "Testing aha!"
    );
}

#[test]
fn test_before_event_callback_drop() {
    fn before_send(
        _evt: sentry::protocol::Event<'static>,
    ) -> Option<sentry::protocol::Event<'static>> {
        None
    }

    let events = sentry::test::with_captured_events_options(
        || {
            sentry::add_breadcrumb(sentry::Breadcrumb {
                message: Some("Testing".into()),
                ..Default::default()
            });
            sentry::capture_message("Hello World!", sentry::Level::Warning);
        },
        sentry::ClientOptions {
            before_send: Some(Arc::new(Box::new(before_send))),
            ..Default::default()
        },
    );

    assert_eq!(events.len(), 0);
}

#[test]
fn test_before_breadcrumb_callback_drop() {
    fn before_breadcrumb(_crumb: sentry::Breadcrumb) -> Option<sentry::Breadcrumb> {
        None
    }

    let events = sentry::test::with_captured_events_options(
        || {
            sentry::add_breadcrumb(sentry::Breadcrumb {
                message: Some("Testing".into()),
                ..Default::default()
            });
            sentry::capture_message("Hello World!", sentry::Level::Warning);
        },
        sentry::ClientOptions {
            before_breadcrumb: Some(Arc::new(Box::new(before_breadcrumb))),
            ..Default::default()
        },
    );

    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.message.as_ref().unwrap(), "Hello World!");
    assert_eq!(event.breadcrumbs.len(), 0);
}
