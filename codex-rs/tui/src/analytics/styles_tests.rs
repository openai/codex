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
                    let label = view.tab_label(section);
                    let tab = buffer
                        .content
                        .chunks(usize::from(area.width))
                        .nth(/*n*/ 1)
                        .unwrap()
                        .windows(label.len())
                        .find(|cells| {
                            cells
                                .iter()
                                .map(ratatui::buffer::Cell::symbol)
                                .collect::<String>()
                                == label
                        })
                        .expect("the report tab is visible");
                    let heading = &tab[0];
                    let active = crate::bottom_pane::active_tab_style();
                    assert_eq!(heading.fg, active.fg.unwrap());
                    assert_eq!(heading.bg, active.bg.unwrap());
                    assert!(heading.modifier.contains(Modifier::BOLD));
                    for marker in buffer.content.iter().filter(|cell| cell.symbol() == "▲") {
                        assert_eq!(marker.fg, accent_style().fg.unwrap());
                        assert_eq!(marker.bg, Color::Reset);
                        assert!(marker.modifier.contains(Modifier::BOLD));
                    }
                    if theme == "light" {
                        for (index, cell) in buffer
                            .content
                            .iter()
                            .enumerate()
                            .filter(|(_, cell)| !cell.symbol().trim().is_empty())
                        {
                            // Keyboard hints deliberately keep the requested white key style.
                            // Report text and chart colors still adapt to light backgrounds.
                            if cell.fg == Color::White {
                                let y = area.y + (index / usize::from(area.width)) as u16;
                                assert!(y >= area.bottom() - 2 || (3..5).contains(&y));
                                assert_eq!(cell.modifier, Modifier::BOLD);
                                continue;
                            }
                            let foreground = match cell.fg {
                                Color::Rgb(r, g, b) => (r, g, b),
                                Color::White => (255, 255, 255),
                                Color::Black => (0, 0, 0),
                                Color::Reset => colors.fg,
                                _ => continue,
                            };
                            let background = match cell.bg {
                                Color::Rgb(r, g, b) => (r, g, b),
                                Color::White => (255, 255, 255),
                                Color::Black => (0, 0, 0),
                                Color::Reset => colors.bg,
                                _ => continue,
                            };
                            let fg = luminance(foreground);
                            let bg = luminance(background);
                            let contrast = (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05);
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

#[test]
fn chat_selection_keeps_a_highlighted_trailing_blank_after_wrapping() {
    with_test_default_colors(
        DefaultColors {
            fg: (30, 30, 30),
            bg: (255, 255, 255),
        },
        || {
            for kind in [models::AccountKind::Business, models::AccountKind::Consumer] {
                let mut view = fixture::view(kind);
                view.section = Section::Chats;
                view.tasks = Load::Ready(tasks::Chats {
                    rows: vec![tasks::Chat {
                        title: "Building Analytics".into(),
                        task: Some(
                            serde_json::from_value(serde_json::json!({
                                "thread_id": "padding-fixture",
                                "data_status": "available",
                                "usage_source": "credits",
                                "weekly_limit_percent": 42.0,
                                "five_hour_limit_percent": 15.0,
                                "balance_usage_credits": "35.83",
                                "groups": []
                            }))
                            .unwrap(),
                        ),
                    }],
                    ..tasks::Chats::default()
                });
                for width in [40, 120] {
                    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 30);
                    let mut buffer = Buffer::empty(area);
                    view.render(area, &mut buffer);
                    let selection = crate::bottom_pane::selection_style().bg.unwrap();
                    let selected_rows = buffer
                        .content
                        .chunks(usize::from(width))
                        .skip(usize::from(view.body_area.y))
                        .take(usize::from(view.body_area.height))
                        .filter(|row| row.iter().any(|cell| cell.bg == selection))
                        .collect::<Vec<_>>();
                    assert!(!selected_rows.is_empty());
                    for row in selected_rows {
                        let end = row.iter().rposition(|cell| cell.bg == selection).unwrap();
                        assert_eq!(row[end].symbol(), " ");
                        assert!(
                            row[..end]
                                .iter()
                                .any(|cell| !cell.symbol().trim().is_empty())
                        );
                    }
                }
            }
        },
    );
}
