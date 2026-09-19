//! Filled tabs preserve active visibility and Unicode clipping on narrow terminals.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

fn rendered_strip(labels: &[&str], active_idx: usize, width: u16) -> (String, Buffer) {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
    let mut buf = Buffer::empty(area);
    buf.set_style(
        area,
        Style::default().red().bg(Color::Yellow).underlined().dim(),
    );
    render_filled_tab_bar(labels, active_idx, area, &mut buf);
    let text = (0..width).map(|x| buf[(x, 0)].symbol()).collect::<String>();
    (text.trim_end().to_owned(), buf)
}

#[test]
fn filled_tabs_window_around_active_tab() {
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (30, 30, 30),
            bg: (255, 255, 255),
        },
        || {
            let labels = [
                "All",
                "Common",
                "Customized (3)",
                "Unbound (4)",
                "App",
                "Composer",
                "Debug",
            ];
            let strips =
                [0, 2, 6].map(|active_idx| rendered_strip(&labels, active_idx, /*width*/ 30).0);
            insta::assert_snapshot!(strips.join("\n"));
            for active_idx in 0..labels.len() {
                let (text, buf) = rendered_strip(&labels, active_idx, /*width*/ 30);
                assert!(text.contains(labels[active_idx]), "{text}");
                let focused = buf
                    .content()
                    .iter()
                    .filter(|cell| cell.bg == active_tab_style().bg.unwrap())
                    .map(|cell| {
                        assert_eq!(cell.modifier, Modifier::BOLD);
                        cell.symbol()
                    })
                    .collect::<String>();
                assert_eq!(focused, format!(" {} ", labels[active_idx]));
            }
        },
    );
}

#[test]
fn filled_tabs_truncate_unicode_without_hiding_active_tab() {
    let labels = ["All", "文件系统 plugins", "Debug"];
    let strips = [1, 4, 9, 16].map(|width| rendered_strip(&labels, /*active_idx*/ 1, width).0);
    insta::assert_snapshot!(strips.join("\n"));
}

// Non-Windows tests have an unprobed default palette; Windows retains its console probe.
#[cfg(not(windows))]
#[test]
fn filled_tab_unknown_palette_uses_readable_underlined_defaults() {
    let (_, buf) = rendered_strip(&["All"], /*active_idx*/ 0, /*width*/ 5);
    assert_eq!(
        buf.content()
            .iter()
            .map(|cell| (cell.fg, cell.bg, cell.modifier))
            .collect::<Vec<_>>(),
        vec![
            (
                Color::Reset,
                Color::Reset,
                Modifier::BOLD | Modifier::UNDERLINED
            );
            5
        ],
    );
}
