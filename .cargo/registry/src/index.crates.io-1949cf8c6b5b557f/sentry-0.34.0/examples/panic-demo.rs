fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        debug: true,
        ..Default::default()
    });

    {
        let _guard = sentry::Hub::current().push_scope();
        sentry::configure_scope(|scope| {
            scope.set_tag("foo", "bar");
        });
        panic!("Holy shit everything is on fire!");
    }
}
