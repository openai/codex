//! Render both account layouts with explicit terminal palettes, including color and selection styles.

use super::*;
use crate::analytics::sections::Section;
use crate::style::accent_style;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;

fn luminance(rgb: (u8, u8, u8)) -> f64 {
    let [r, g, b] = [rgb.0, rgb.1, rgb.2].map(|channel| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(/*n*/ 2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

#[test]
fn analytics_sections_adapt_to_terminal_colors() {
    for (theme, colors) in [
        (
            "light",
            DefaultColors {
                fg: (0, 0, 0),
                bg: (255, 255, 255),
            },
        ),
        (
            "dark",
            DefaultColors {
                fg: (230, 230, 230),
                bg: (16, 16, 16),
            },
        ),
    ] {
        with_test_default_colors(colors, || {
            for business in [false, true] {
                let mut view = fixture::view(models::AccountKind::Consumer);
                if business {
                    view = fixture::view(models::AccountKind::Enterprise);
                }
                // Exercise the reported credit range and preserve exact amounts below the plot.
                let mut credits =
                    fixture::history(/*report*/ 1, /*range*/ 0, /*group*/ 0);
                for day in &mut credits.data {
                    day.total *= 40.0;
                    for value in &mut day.values {
                        value.value *= 40.0;
                    }
                }
                view.sections[Section::Credits].history = Load::Ready(credits);
                for section in view.visible_sections() {
                    let section = *section;
                    view.section = section;
                    let area = Rect::new(
                        /*x*/ 0, /*y*/ 0, /*width*/ 120, /*height*/ 42,
                    );
                    let mut buffer = Buffer::empty(area);
                    view.render(area, &mut buffer);
                    let heading = buffer
                        .content
                        .iter()
                        .find(|cell| cell.symbol() == "▎")
                        .unwrap();
                    assert_eq!(heading.fg, accent_style().fg.unwrap());
                    assert!(heading.modifier.contains(Modifier::BOLD));
                    for marker in buffer.content.iter().filter(|cell| cell.symbol() == "▲") {
                        assert_eq!(marker.fg, accent_style().fg.unwrap());
                        assert_eq!(marker.bg, Color::Reset);
                        assert!(marker.modifier.contains(Modifier::BOLD));
                    }
                    if theme == "light" {
                        for cell in &buffer.content {
                            if !cell.symbol().trim().is_empty()
                                && let Color::Rgb(r, g, b) = cell.fg
                            {
                                let contrast =
                                    (luminance(colors.bg) + 0.05) / (luminance((r, g, b)) + 0.05);
                                assert!(
                                    contrast >= 4.5,
                                    "{}: {:?} has contrast {contrast}",
                                    cell.symbol(),
                                    cell.fg
                                );
                            }
                        }
                    }
                    // Preserve every symbol and style while sharing repeated styles in a palette.
                    let mut palette = Vec::new();
                    let mut text = Vec::new();
                    let mut runs = Vec::new();
                    for (y, row) in buffer.content.chunks(usize::from(area.width)).enumerate() {
                        text.push(format!(
                            "{:?}",
                            row.iter()
                                .map(ratatui::buffer::Cell::symbol)
                                .collect::<String>()
                        ));
                        let mut previous = None;
                        let mut changes = Vec::new();
                        for (x, cell) in row.iter().enumerate() {
                            let style = format!(
                                "{:?}|{:?}|{:?}|{:?}",
                                cell.fg, cell.bg, cell.underline_color, cell.modifier
                            );
                            let index = palette
                                .iter()
                                .position(|candidate| *candidate == style)
                                .unwrap_or_else(|| {
                                    palette.push(style);
                                    palette.len() - 1
                                });
                            if previous != Some(index) {
                                changes.push(format!("{x}:{index}"));
                                previous = Some(index);
                            }
                        }
                        runs.push(format!("{y}: {}", changes.join(" ")));
                    }
                    let palette = palette
                        .iter()
                        .enumerate()
                        .map(|(index, style)| format!("{index}: {style}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let snapshot = format!(
                        "{}x{}\nText:\n{}\nPalette (foreground|background|underline|modifiers):\n{palette}\nRows (column:palette):\n{}",
                        area.width,
                        area.height,
                        text.join("\n"),
                        runs.join("\n")
                    );
                    insta::assert_snapshot!(
                        format!(
                            "analytics_{theme}_{}_{}",
                            if business { "business" } else { "consumer" },
                            section as usize + 1
                        ),
                        snapshot
                    );
                }
            }
        });
    }
}
