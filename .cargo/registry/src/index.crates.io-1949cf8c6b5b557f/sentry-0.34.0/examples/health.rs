fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        // release health requires a release to be set
        release: sentry::release_name!(),
        debug: true,
        // session tracking is enabled by default, but we want to explicitly
        // create the session
        auto_session_tracking: false,
        ..Default::default()
    });

    let handle = std::thread::spawn(|| {
        // this session will be set to crashed
        sentry::start_session();
        std::thread::sleep(std::time::Duration::from_secs(3));
        panic!("oh no!");
    });

    sentry::start_session();

    sentry::capture_message(
        "anything with a level >= Error will increase the error count",
        sentry::Level::Error,
    );

    // or any error that has an explicit exception attached
    let err = "NaN".parse::<usize>().unwrap_err();
    sentry::capture_error(&err);

    std::thread::sleep(std::time::Duration::from_secs(2));

    // this session will have an error count of 2, but otherwise have
    // a clean exit.
    sentry::end_session();

    handle.join().ok();
}
