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
            for kind in [
                models::AccountKind::Consumer,
                models::AccountKind::Enterprise,
            ] {
                let mut view = fixture::view(kind);
                view.sections[Section::Chats].detail = Some(0);
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
                        for cell in buffer
                            .content
                            .iter()
                            .filter(|cell| !cell.symbol().trim().is_empty())
                        {
                            let foreground = match cell.fg {
                                Color::Rgb(r, g, b) => (r, g, b),
                                Color::White => (255, 255, 255),
                                Color::Black => (0, 0, 0),
                                Color::Reset => colors.fg,
                                _ => continue,
                            };
                            let contrast =
                                (luminance(colors.bg) + 0.05) / (luminance(foreground) + 0.05);
                            assert!(
                                contrast >= 4.5,
                                "{}: {:?} has contrast {contrast}",
                                cell.symbol(),
                                cell.fg
                            );
                        }
                    }
                }
            }
        });
    }
}
