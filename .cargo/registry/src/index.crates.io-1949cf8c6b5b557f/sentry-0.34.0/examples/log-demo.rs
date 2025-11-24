use log::{debug, error, info, warn};

fn main() {
    init_log();

    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        debug: true,
        ..Default::default()
    });

    debug!("System is booting");
    info!("System is booting");
    warn!("System is warning");
    error!("Holy shit everything is on fire!");
}

fn init_log() {
    let mut log_builder = pretty_env_logger::formatted_builder();
    log_builder.parse_filters("info");
    let logger = sentry_log::SentryLogger::with_dest(log_builder.build());

    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();
}
