//! example for manually testing the perf of color-eyre in debug vs release

use color_eyre::{
    eyre::Report,
    eyre::{eyre, WrapErr},
    Section,
};
use tracing::instrument;

fn main() -> Result<(), Report> {
    #[cfg(feature = "capture-spantrace")]
    install_tracing();
    color_eyre::install()?;

    time_report();

    Ok(())
}

#[instrument]
fn time_report() {
    time_report_inner()
}

#[instrument]
fn time_report_inner() {
    let start = std::time::Instant::now();
    let report = Err::<(), Report>(eyre!("fake error"))
        .wrap_err("wrapped error")
        .suggestion("try using a file that exists next time")
        .unwrap_err();

    println!("Error: {:?}", report);
    drop(report);
    let end = std::time::Instant::now();

    dbg!(end - start);
}

#[cfg(feature = "capture-spantrace")]
fn install_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}
