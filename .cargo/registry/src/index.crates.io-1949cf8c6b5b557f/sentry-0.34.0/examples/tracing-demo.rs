use std::thread;
use std::time::Duration;

use tracing_subscriber::prelude::*;

// cargo run --example tracing-demo
fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(sentry_tracing::layer())
        .try_init()
        .unwrap();

    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        traces_sample_rate: 1.0,
        debug: true,
        ..Default::default()
    });

    tracing::debug!("System is booting");
    tracing::info!("System is booting");

    main_span1();
    thread::sleep(Duration::from_millis(100));
}

#[tracing::instrument]
fn main_span1() {
    thread::sleep(Duration::from_millis(50));

    tracing::warn!("System is warning");

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));

        thread_span1("foo");

        tracing::error!("Holy shit everything is on fire!");
    });
    thread::sleep(Duration::from_millis(100));

    main_span2(SomeArgument::default())
}

#[tracing::instrument]
fn thread_span1(_arg: &str) {
    thread::sleep(Duration::from_millis(200));
}

#[tracing::instrument]
fn main_span2(_arg: SomeArgument) {
    thread::sleep(Duration::from_millis(200));
}

#[derive(Debug, Default)]
struct SomeArgument {
    _a: u32,
    _b: bool,
    _c: &'static str,
}
