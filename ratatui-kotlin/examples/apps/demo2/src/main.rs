//! # [Ratatui] Demo2 example
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

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

mod app;
mod colors;
mod destroy;
mod tabs;
mod theme;

use std::io::stdout;

use app::App;
use color_eyre::Result;
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::Rect;
use ratatui::{TerminalOptions, Viewport};

pub use self::colors::{RgbSwatch, color_from_oklab};
pub use self::theme::THEME;

fn main() -> Result<()> {
    color_eyre::install()?;
    // this size is to match the size of the terminal when running the demo
    // using vhs in a 1280x640 sized window (github social preview size)
    let viewport = Viewport::Fixed(Rect::new(0, 0, 81, 18));
    let terminal = ratatui::init_with_options(TerminalOptions { viewport });
    execute!(stdout(), EnterAlternateScreen).expect("failed to enter alternate screen");
    let app_result = App::default().run(terminal);
    execute!(stdout(), LeaveAlternateScreen).expect("failed to leave alternate screen");
    ratatui::restore();
    app_result
}
