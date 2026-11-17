//! Draws a US QWERTY tenkeyless keyboard with highlighted shortcut keys.

use crate::color::blend;
use crate::color::is_light;
use crate::style::readable_color_on;
use crate::style::selection_style;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::rgb_color;
use crossterm::event::KeyCode;
use crossterm::event::ModifierKeyCode::*;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::text::Line;
use std::collections::HashSet;

const ROWS: usize = 6;
const NAV_START: usize = 52;
const INNER_WIDTH: usize = NAV_START + 16;
pub(super) const WIDTH: u16 = INNER_WIDTH as u16 + 4;
const ROOMY_WIDTH: u16 = 92;

#[derive(Clone, Copy)]
enum Geometry {
    Compact,
    Roomy,
}

impl Geometry {
    fn for_width(width: u16) -> Self {
        if width >= ROOMY_WIDTH {
            Self::Roomy
        } else {
            Self::Compact
        }
    }

    fn inner_width(self) -> usize {
        match self {
            Self::Compact => INNER_WIDTH,
            Self::Roomy => usize::from(ROOMY_WIDTH) - 4,
        }
    }

    fn nav_start(self) -> usize {
        match self {
            Self::Compact => NAV_START,
            Self::Roomy => 69,
        }
    }
}
const UNSHIFTED: &str = r"`1234567890-=[]\;',./";
const SHIFTED: &str = r#"~!@#$%^&*()_+{}|:"<>?"#;

pub(super) fn unshift(ch: char) -> Option<char> {
    SHIFTED
        .find(ch)
        .map(|index| char::from(UNSHIFTED.as_bytes()[index]))
}

#[derive(Clone, Copy)]
struct Cap<'a>(KeyCode, &'a str, usize);

const fn key(code: KeyCode, label: &'static str) -> Cap<'static> {
    Cap(code, label, label.len() + 2)
}

fn keyboard_styles() -> [Style; 4] {
    let terminal_default = Style::default().bg(Color::Reset);
    let fallback = [
        terminal_default,
        terminal_default,
        terminal_default,
        selection_style(),
    ];
    let (Some(background), Some(foreground)) = (default_bg(), default_fg()) else {
        return fallback;
    };
    let panel_rgb = blend(foreground, background, /*alpha*/ 0.07);
    let (panel_fill, cap_fill, bound_fill) = match best_color(panel_rgb) {
        Color::Reset => {
            return fallback;
        }
        Color::Indexed(_) => {
            // Use distinct xterm swatches so palette reduction cannot merge the surfaces.
            let (panel, alternate, cap, bound) = if is_light(background) {
                (
                    (228, 228, 228),
                    (218, 218, 218),
                    (198, 198, 198),
                    (175, 215, 255),
                )
            } else {
                ((48, 48, 48), (58, 58, 58), (78, 78, 78), (0, 95, 135))
            };
            let panel = best_color(panel);
            let panel = if panel == best_color(background) {
                best_color(alternate)
            } else {
                panel
            };
            (panel, best_color(cap), best_color(bound))
        }
        panel => (
            panel,
            best_color(blend(foreground, background, /*alpha*/ 0.22)),
            best_color(blend((35, 100, 140), panel_rgb, /*alpha*/ 0.63)),
        ),
    };
    let panel = Style::default().bg(panel_fill);
    let border = panel.fg(readable_color_on(
        rgb_color(blend(foreground, background, /*alpha*/ 0.60)),
        Some(panel_fill),
    ));
    let cap = Style::default()
        .bg(cap_fill)
        .fg(readable_color_on(rgb_color(foreground), Some(cap_fill)));
    let bound = if [panel_fill, cap_fill].contains(&bound_fill) {
        selection_style()
    } else {
        Style::default()
            .bg(bound_fill)
            .fg(readable_color_on(
                rgb_color((190, 228, 255)),
                Some(bound_fill),
            ))
            .bold()
    };
    [panel, border, cap, bound]
}
fn cap(
    row: &mut Line<'static>,
    Cap(code, label, width): Cap<'_>,
    actions: &mut HashSet<KeyCode>,
    styles: &[Style; 4],
    geometry: Geometry,
) {
    let style = if actions.remove(&code) {
        styles[3]
    } else {
        styles[2]
    };
    let width = match geometry {
        Geometry::Compact => width,
        Geometry::Roomy => match code {
            KeyCode::Char(' ') => 12,
            KeyCode::F(1..=4) => label.len() + 2,
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => 6,
            _ => label.chars().count() + 3,
        },
    };
    let face_width = width - 1;
    row.push_span(format!("{label:^face_width$}").set_style(style));
    row.push_span(" ");
}

fn framed(content: Line<'static>, panel: Style, frame: Style, geometry: Geometry) -> Line<'static> {
    let padding = geometry.inner_width() - content.width();
    let mut line = Line::from("│ ".set_style(frame)).style(panel);
    line.extend(content.spans);
    line.push_span(" ".repeat(padding));
    line.push_span(" │".set_style(frame));
    line
}

// Leaves keys without a keycap in actions so the guide can point to its list.
pub(super) fn lines(actions: &mut HashSet<KeyCode>, width: u16) -> Vec<Line<'static>> {
    let geometry = Geometry::for_width(width);
    let styles = keyboard_styles();
    let [panel, frame, _, _] = styles;
    let rail = "─".repeat(geometry.inner_width() + 2);
    let mut lines = vec![Line::from(format!("┌{rail}┐").set_style(frame))];
    for index in 0..ROWS {
        let mut row = Line::default();
        let (left, letters, right): (&[Cap<'_>], &str, &[Cap<'_>]) = match index {
            0 => (&[], "", &[]),
            1 => (&[], "`1234567890-=", &[Cap(KeyCode::Backspace, "Bksp", 9)]),
            2 => (&[Cap(KeyCode::Tab, "Tab", 6)], "qwertyuiop[]\\", &[]),
            3 => (
                &[Cap(KeyCode::CapsLock, "Caps", 7)],
                "asdfghjkl;'",
                &[Cap(KeyCode::Enter, "Enter", 8)],
            ),
            4 => (
                &[Cap(KeyCode::Modifier(LeftShift), "Shift", 8)],
                "zxcvbnm,./",
                &[Cap(KeyCode::Modifier(RightShift), "Shift", 10)],
            ),
            _ => (
                &[
                    Cap(KeyCode::Modifier(LeftControl), "Ctrl", 5),
                    Cap(KeyCode::Modifier(LeftSuper), "Super", 6),
                    key(KeyCode::Modifier(LeftAlt), "Alt"),
                    Cap(KeyCode::Char(' '), "Space", 11),
                    key(KeyCode::Modifier(RightAlt), "Alt"),
                    Cap(KeyCode::Modifier(RightSuper), "Super", 6),
                    Cap(KeyCode::Menu, "Menu", 5),
                    Cap(KeyCode::Modifier(RightControl), "Ctrl", 5),
                ],
                "",
                &[],
            ),
        };
        if index == 0 {
            cap(
                &mut row,
                key(KeyCode::Esc, "Esc"),
                actions,
                &styles,
                geometry,
            );
            for (first, gap) in [(1, "   "), (5, "  "), (9, " ")] {
                if matches!(geometry, Geometry::Compact) {
                    row.push_span(gap);
                }
                for number in first..first + 4 {
                    let label = format!("F{number}");
                    cap(
                        &mut row,
                        Cap(KeyCode::F(number), &label, label.len() + 1),
                        actions,
                        &styles,
                        geometry,
                    );
                }
            }
        }
        for &key in left {
            cap(&mut row, key, actions, &styles, geometry);
        }
        for letter in letters.chars() {
            let mut label = [0; 4];
            let label = letter.to_ascii_uppercase().encode_utf8(&mut label);
            let width = if letter == '\\' { 6 } else { 3 };
            cap(
                &mut row,
                Cap(KeyCode::Char(letter), label, width),
                actions,
                &styles,
                geometry,
            );
        }
        for &key in right {
            cap(&mut row, key, actions, &styles, geometry);
        }

        let nav: &[Cap<'_>] = match index {
            0 => &[
                Cap(KeyCode::PrintScreen, "PrtSc", 6),
                Cap(KeyCode::ScrollLock, "ScrLk", 6),
                Cap(KeyCode::Pause, "Pau", 4),
            ],
            1 => &[
                key(KeyCode::Insert, "Ins"),
                key(KeyCode::Home, "Home"),
                key(KeyCode::PageUp, "PgU"),
            ],
            2 => &[
                key(KeyCode::Delete, "Del"),
                Cap(KeyCode::End, "End", 6),
                key(KeyCode::PageDown, "PgD"),
            ],
            4 => &[Cap(KeyCode::Up, "↑", 6)],
            5 => &[
                Cap(KeyCode::Left, "←", 5),
                Cap(KeyCode::Down, "↓", 6),
                Cap(KeyCode::Right, "→", 5),
            ],
            _ => &[],
        };
        if !nav.is_empty() {
            let nav_start = match (geometry, index) {
                (Geometry::Roomy, 0) => 66,
                (Geometry::Compact, 4) => geometry.nav_start() + 5,
                (Geometry::Roomy, 4) => geometry.nav_start() + 6,
                _ => geometry.nav_start(),
            };
            row.push_span(" ".repeat(nav_start - row.width()));
            for &key in nav {
                cap(&mut row, key, actions, &styles, geometry);
            }
        }
        lines.push(framed(row, panel, frame, geometry));
        lines.push(framed(Line::default(), panel, frame, geometry));
        if index == 0 {
            lines.push(Line::from(format!("├{rail}┤").set_style(frame)));
        }
    }
    lines.push(Line::from(format!("└{rail}┘").set_style(frame)));
    lines
}
