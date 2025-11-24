fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        debug: true,
        ..Default::default()
    });

    sentry::with_scope(
        |scope| {
            scope.set_level(Some(sentry::Level::Warning));
            scope.set_fingerprint(Some(["a-message"].as_ref()));
            scope.set_tag("foo", "bar");
        },
        || {
            panic!("Shit's on fire yo. 🔥 🚒");
        },
    );
}
