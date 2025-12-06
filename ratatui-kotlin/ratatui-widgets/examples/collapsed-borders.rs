//! # [Ratatui] [`Block`] with collapsed borders example
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

use std::collections::HashMap;

use color_eyre::Result;
use crossterm::event;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect, Spacing};
use ratatui::style::{Color, Stylize};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        let mut selected_pane = Pane::Top;
        loop {
            terminal.draw(|frame| render(frame, selected_pane))?;
            if let Some(key_event) = event::read()?.as_key_press_event() {
                match key_event.code {
                    event::KeyCode::Up => selected_pane = Pane::Top,
                    event::KeyCode::Left => selected_pane = Pane::Left,
                    event::KeyCode::Right => selected_pane = Pane::Right,
                    event::KeyCode::Down => selected_pane = Pane::Bottom,
                    event::KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
    })
}

// Derive Eq, PartialEq, Hash for Pane to use as HashMap key
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
enum Pane {
    Top,
    Left,
    Right,
    Bottom,
}

/// Render the UI with various blocks.
fn render(frame: &mut Frame, selected_pane: Pane) {
    let [title, blocks] = frame.area().layout(&Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
    ]));

    render_title(frame, title);
    render_blocks(selected_pane, frame, blocks);
}

fn render_title(frame: &mut Frame<'_>, area: Rect) {
    let title = Line::from_iter([
        "Block With Collapsed Borders".bold(),
        " (Press 'q' to quit)".into(),
    ]);
    frame.render_widget(title.centered(), area);
}

fn render_blocks(selected_pane: Pane, frame: &mut Frame<'_>, area: Rect) {
    // The recipe to achieve collapsed borders is as follows:
    // 1. Use `MergeStrategy::Exact` (or `MergeStrategy::Fuzzy`) to merge borders of adjacent
    //    blocks.
    // 2. Use a layout with `Spacing::Overlap(1)` to ensure that the borders overlap
    // 3. Use `BorderType::Thick` for the selected pane to make it visually distinct.
    // 4. Render the selected pane last so it appears on top of the others.
    let [top, middle, bottom] =
        area.layout(&Layout::vertical([Constraint::Fill(1); 3]).spacing(Spacing::Overlap(1)));
    let [left, right] =
        middle.layout(&Layout::horizontal([Constraint::Fill(1); 2]).spacing(Spacing::Overlap(1)));

    // Store pane areas and titles in a single HashMap indexed by Pane. A real application might
    // store actual data or widgets in these areas instead (and use `WidgetRef` to handle
    // heterogeneous widgets).
    let mut panes = HashMap::new();
    panes.insert(Pane::Top, (top, "Top Block"));
    panes.insert(Pane::Left, (left, "Left Block"));
    panes.insert(Pane::Right, (right, "Right Block"));
    panes.insert(Pane::Bottom, (bottom, "Bottom Block"));

    // Render all panes except the selected one first
    for (&pane, &(area, title)) in &panes {
        if pane != selected_pane {
            // MergeStrategy::Exact causes the borders to collapse
            let block = Block::bordered()
                .merge_borders(MergeStrategy::Exact)
                .title(title);
            frame.render_widget(block, area);
        }
    }
    // Render the selected pane last (so it appears on top) with a thick border
    if let Some(&(area, title)) = panes.get(&selected_pane) {
        // MergeStrategy::Exact causes the borders to collapse
        let block = Block::bordered()
            .merge_borders(MergeStrategy::Exact)
            .border_type(BorderType::Thick)
            .border_style(Color::Yellow)
            .title(title);
        frame.render_widget(block, area);
    }
}
