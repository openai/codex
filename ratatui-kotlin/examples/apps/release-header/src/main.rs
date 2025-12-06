//! Generates a terminal banner for Ratatui releases featuring a Ratatui logo, version info, and
//! a list of crates.
//!
//! Used for README.md, documentation, and release materials. Updated for every release starting
//! with v0.30.0 "Bryndza".
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest

use std::io::stdout;
use std::iter::zip;

use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{event, execute};
use ratatui::layout::{Constraint, Flex, Layout, Margin, Rect, Spacing};
use ratatui::style::{Color, Stylize};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Padding, Paragraph, RatatuiLogo};
use ratatui::{DefaultTerminal, Frame, TerminalOptions, Viewport};

const SEMVER: &str = "0.30.0";
const RELEASE_NAME: &str = "Bryndza";

const MAIN_DISHES: [&str; 4] = [
    "> ratatui",
    "> ratatui-core",
    "> ratatui-widgets",
    "> ratatui-macros",
];
const BACKENDS: [&str; 3] = [
    "> ratatui-crossterm",
    "> ratatui-termion",
    "> ratatui-termwiz",
];

const FG_COLOR: Color = Color::Rgb(246, 214, 187); // #F6D6BB
const BG_COLOR: Color = Color::Rgb(20, 20, 50); // #141432
const MENU_BORDER_COLOR: Color = Color::Rgb(255, 255, 160); // #FFFFA0

enum Rainbow {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Indigo,
    Violet,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let viewport = Viewport::Fixed(Rect::new(0, 0, 68, 16));
    let terminal = ratatui::init_with_options(TerminalOptions { viewport });
    execute!(stdout(), EnterAlternateScreen).expect("failed to enter alternate screen");
    let result = run(terminal);
    execute!(stdout(), LeaveAlternateScreen).expect("failed to leave alternate screen");
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
    loop {
        terminal.draw(render)?;
        if event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    let area = frame.area();
    frame.buffer_mut().set_style(area, (FG_COLOR, BG_COLOR));

    let logo_width = 29;
    let menu_width = 23;
    let padding = 2; // Padding between logo and menu
    let menu_borders = 3;
    let height = MAIN_DISHES.len() as u16 + BACKENDS.len() as u16 + menu_borders;
    let width = logo_width + menu_width + padding;
    let center_area = area.centered(Constraint::Length(width), Constraint::Length(height));
    let layout = Layout::horizontal(Constraint::from_lengths([logo_width, padding, menu_width]));
    let [logo_area, _, menu_area] = center_area.layout(&layout);

    render_logo(frame, logo_area);
    render_menu(frame, menu_area);
}

fn render_logo(frame: &mut Frame, area: Rect) {
    let area = area.inner(Margin::new(1, 0));
    let layout = Layout::vertical(Constraint::from_lengths([6, 2, 1])).flex(Flex::End);
    let [shadow_area, logo_area, version_area] = area.layout(&layout);

    // Divide the logo into letter sections for individual coloring, then render a block for each
    // letter with a color based on the row index.
    let letter_layout = Layout::horizontal(Constraint::from_lengths([5, 4, 4, 4, 4, 5, 1]));
    for (row_index, row) in shadow_area.rows().enumerate() {
        for (rainbow, letter_area) in zip(Rainbow::ROYGBIV, row.layout_vec(&letter_layout)) {
            let color = rainbow.gradient_color(row_index);
            frame.render_widget(Block::new().style(color), letter_area);
        }
        // Render the Ratatui logo truncated.
        frame.render_widget(RatatuiLogo::small(), row);
    }

    frame.render_widget(Block::new().style(FG_COLOR), logo_area);
    frame.render_widget(RatatuiLogo::small(), logo_area);
    frame.render_widget(format!("v{SEMVER} \"{RELEASE_NAME}\"").dim(), version_area);
}

impl Rainbow {
    const RED_GRADIENT: [u8; 6] = [41, 43, 50, 68, 104, 156];
    const GREEN_GRADIENT: [u8; 6] = [24, 30, 41, 65, 105, 168];
    const BLUE_GRADIENT: [u8; 6] = [55, 57, 62, 78, 113, 166];
    const AMBIENT_GRADIENT: [u8; 6] = [17, 18, 20, 25, 40, 60];

    const ROYGBIV: [Self; 7] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Indigo,
        Self::Violet,
    ];

    fn gradient_color(&self, row: usize) -> Color {
        let ambient = Self::AMBIENT_GRADIENT[row];
        let red = Self::RED_GRADIENT[row];
        let green = Self::GREEN_GRADIENT[row];
        let blue = Self::BLUE_GRADIENT[row];
        let blue_sat = Self::AMBIENT_GRADIENT[row].saturating_mul(6 - row as u8);
        let (r, g, b) = match self {
            Self::Red => (red, ambient, blue_sat),
            Self::Orange => (red, green / 2, blue_sat),
            Self::Yellow => (red, green, blue_sat),
            Self::Green => (ambient, green, blue_sat),
            Self::Blue => (ambient, ambient, blue.max(blue_sat)),
            Self::Indigo => (blue, ambient, blue.max(blue_sat)),
            Self::Violet => (red, ambient, blue.max(blue_sat)),
        };
        Color::Rgb(r, g, b)
    }
}

fn render_menu(frame: &mut Frame, area: Rect) {
    let layout = Layout::vertical(Constraint::from_lengths([
        MAIN_DISHES.len() as u16 + 2,
        BACKENDS.len() as u16 + 2,
    ]))
    .spacing(Spacing::Overlap(1)); // Overlap to merge borders
    let [main_dishes_area, backends_area] = area.layout(&layout);

    render_menu_block(frame, main_dishes_area, "Main Courses", &MAIN_DISHES);
    render_menu_block(frame, backends_area, "Pairings", &BACKENDS);
}

fn render_menu_block(frame: &mut Frame, area: Rect, title: &str, menu_items: &[&str]) {
    let menu_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(MENU_BORDER_COLOR)
        .padding(Padding::horizontal(1))
        .merge_borders(MergeStrategy::Fuzzy)
        .title(title);

    let menu_lines: Vec<Line> = menu_items.iter().map(|&item| Line::from(item)).collect();
    frame.render_widget(Paragraph::new(menu_lines).block(menu_block), area);
}
