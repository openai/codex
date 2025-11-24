fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        debug: true,
        ..Default::default()
    });

    sentry::configure_scope(|scope| {
        scope.add_event_processor(|mut event| {
            event.request = Some(sentry::protocol::Request {
                url: Some("https://example.com/".parse().unwrap()),
                method: Some("GET".into()),
                ..Default::default()
            });
            Some(event)
        });
    });

    sentry::configure_scope(|scope| {
        scope.set_fingerprint(Some(["a-message"].as_ref()));
        scope.set_tag("foo", "bar");
    });

    let id = sentry::capture_message("An HTTP request failed.", sentry::Level::Error);
    println!("sent event {id}");
}
