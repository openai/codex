//! # [Ratatui] `LineGauge` example
//!
//! The latest version of this example is available in the [widget examples] folder in the
//! repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui/ratatui
//! [widget examples]: https://github.com/ratatui/ratatui/blob/main/ratatui-widgets/examples
//! [examples readme]: https://github.com/ratatui/ratatui/blob/main/examples/README.md

use core::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint::{Length, Min};
use ratatui::layout::{Layout, Rect};
use ratatui::style::palette::tailwind;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{LineGauge, Paragraph, Widget};

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| App::default().run(terminal))
}

#[derive(Debug, Default, Clone, Copy)]
struct App {
    state: AppState,
    progress_columns: u16,
    progress: f64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum AppState {
    #[default]
    Start,
    Stop,
    Quit,
}

impl App {
    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while self.state != AppState::Quit {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            self.handle_events()?;
            self.update(terminal.size()?.width);
        }
        Ok(())
    }

    fn update(&mut self, terminal_width: u16) {
        if self.state != AppState::Start {
            return;
        }

        self.progress_columns = (self.progress_columns + 1).clamp(0, terminal_width);
        self.progress = f64::from(self.progress_columns) / f64::from(terminal_width);
    }

    fn handle_events(&mut self) -> Result<()> {
        let timeout = Duration::from_secs_f32(1.0 / 20.0);
        if event::poll(timeout)? {
            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Char(' ') => self.toggle_start(),
                    KeyCode::Char('r') => self.reset(),
                    KeyCode::Char('q') => self.state = AppState::Quit,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn toggle_start(&mut self) {
        self.state = if self.state == AppState::Start {
            AppState::Stop
        } else {
            AppState::Start
        };
    }

    const fn reset(&mut self) {
        self.progress = 0.0;
        self.progress_columns = 0;
        self.state = AppState::Stop;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Length(3), Min(0)]);
        let [header_area, main_area] = area.layout(&layout);

        let gauges_layout = Layout::vertical([Length(2); 3]);
        let [gauge1_area, gauge4_area, gauge6_area] = main_area.layout(&gauges_layout);

        header(header_area, buf);

        self.render_gauge1(gauge1_area, buf);
        self.render_gauge2(gauge4_area, buf);
        self.render_gauge3(gauge6_area, buf);
    }
}

fn header(area: Rect, buf: &mut Buffer) {
    let [p1_area, p2_area] = area.layout(&Layout::vertical([Length(1), Min(1)]));
    Paragraph::new("LineGauge Example")
        .bold()
        .centered()
        .render(p1_area, buf);

    Paragraph::new("(Press 'SPACE' to start/stop progress, 'r' to reset progress, 'q' to quit)")
        .centered()
        .render(p2_area, buf);
}

impl App {
    fn render_gauge1(&self, area: Rect, buf: &mut Buffer) {
        LineGauge::default()
            .filled_style(Style::default().fg(tailwind::LIME.c400))
            .unfilled_style(Style::default().fg(tailwind::LIME.c800))
            .ratio(self.progress)
            .render(area, buf);
    }

    fn render_gauge2(&self, area: Rect, buf: &mut Buffer) {
        LineGauge::default()
            .filled_symbol("⣿")
            .unfilled_symbol("⣿")
            .filled_style(Style::default().fg(tailwind::CYAN.c400))
            .unfilled_style(Style::default().fg(tailwind::CYAN.c800))
            .ratio(self.progress)
            .render(area, buf);
    }

    fn render_gauge3(&self, area: Rect, buf: &mut Buffer) {
        LineGauge::default()
            .filled_symbol("▰")
            .unfilled_symbol("▱")
            .filled_style(Style::default().fg(tailwind::BLUE.c400))
            .unfilled_style(Style::default().fg(tailwind::BLUE.c800))
            .ratio(self.progress)
            .render(area, buf);
    }
}
