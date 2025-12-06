//! # [Ratatui] Original Demo example
//!
//! The latest version of this example is available in the [examples] folder in the repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui/ratatui
//! [examples]: https://github.com/ratatui/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui/ratatui/blob/main/examples/README.md

use std::error::Error;
use std::time::Duration;

use clap::Parser;

mod app;
#[cfg(feature = "crossterm")]
mod crossterm;
#[cfg(all(not(windows), feature = "termion"))]
mod termion;
#[cfg(feature = "termwiz")]
mod termwiz;

mod ui;

/// Demo
#[derive(Debug, Parser)]
struct Cli {
    /// time in ms between two ticks.
    #[arg(short, long, default_value_t = 250)]
    tick_rate: u64,

    /// whether unicode symbols are used to improve the overall look of the app
    #[arg(short, long, default_value_t = true)]
    unicode: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let tick_rate = Duration::from_millis(cli.tick_rate);
    #[cfg(feature = "crossterm")]
    crate::crossterm::run(tick_rate, cli.unicode)?;
    #[cfg(all(not(windows), feature = "termion", not(feature = "crossterm")))]
    crate::termion::run(tick_rate, cli.unicode)?;
    #[cfg(all(
        feature = "termwiz",
        not(feature = "crossterm"),
        not(feature = "termion")
    ))]
    crate::termwiz::run(tick_rate, cli.unicode)?;
    Ok(())
}
