//! Check rendered foregrounds after color conversion, including mid-tone backgrounds.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::widgets::Widget;

#[test]
fn selection_text_stays_readable_after_palette_conversion() {
    for background in [
        (255, 255, 255),
        (245, 240, 220),
        (130, 130, 130),
        super::super::CHATGPT_BLUE_100,
        (132, 184, 248),
        (18, 20, 30),
    ] {
        for level in [StdoutColorLevel::TrueColor, StdoutColorLevel::Ansi256] {
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 1,
            );
            let mut buffer = Buffer::empty(area);
            buffer.set_style(area, Style::default().dim());
            Line::from("› resume")
                .style(selection_style(Some(background), level))
                .render(area, &mut buffer);
            let cell = &buffer[(0, 0)];
            let resolve = |color| match color {
                Color::Rgb(r, g, b) => (r, g, b),
                Color::Indexed(index) => XTERM_COLORS[usize::from(index)],
                _ => panic!("expected an explicit color: {color:?}"),
            };
            assert!(ratio(resolve(cell.fg), resolve(cell.bg)) >= MIN_TEXT_CONTRAST);
            let preferred = if is_light(background) {
                super::super::CHATGPT_BLUE_100
            } else {
                super::super::CHATGPT_BLUE_200
            };
            assert!(
                ratio(resolve(cell.bg), background)
                    >= ratio(resolve(best_color_for_level(preferred, level)), background)
            );
            assert_eq!(cell.modifier, Modifier::BOLD);
        }
    }
}

#[test]
fn selection_uses_terminal_defaults_without_a_known_palette() {
    for (background, level) in [
        (None, StdoutColorLevel::TrueColor),
        (Some((255, 255, 255)), StdoutColorLevel::Ansi16),
        (Some((255, 255, 255)), StdoutColorLevel::Unknown),
    ] {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 1, /*height*/ 1,
        );
        let mut buffer = Buffer::empty(area);
        buffer.set_style(area, Style::default().red().bg(Color::Yellow).dim());
        Line::from("›")
            .style(selection_style(background, level))
            .render(area, &mut buffer);
        let cell = &buffer[(0, 0)];
        assert_eq!(
            (cell.fg, cell.bg, cell.modifier),
            (
                Color::Reset,
                Color::Reset,
                Modifier::BOLD | Modifier::REVERSED
            )
        );
    }
}

#[test]
fn foreground_meets_minimum_after_palette_conversion() {
    for background in [
        (255, 255, 255),
        (245, 240, 220),
        (130, 130, 130),
        (18, 20, 30),
    ] {
        for preferred in [
            super::super::LIGHT_BG_ACCENT_RGB,
            super::super::UI_ACCENT,
            (139, 98, 20),
            (196, 167, 103),
        ] {
            for level in [StdoutColorLevel::TrueColor, StdoutColorLevel::Ansi256] {
                let color = foreground(preferred, Some(background), level);
                let rgb = match color {
                    Color::Rgb(r, g, b) => (r, g, b),
                    Color::Indexed(index) => XTERM_COLORS[usize::from(index)],
                    _ => panic!("expected an explicit color: {color:?}"),
                };
                assert!(
                    ratio(rgb, background) >= MIN_TEXT_CONTRAST,
                    "{rgb:?} on {background:?}"
                );
            }
        }
    }
}

#[test]
fn shaded_surface_contrast_uses_its_rendered_palette_background() {
    for background in [
        (255, 255, 255),
        (225, 220, 205),
        (130, 130, 130),
        (95, 95, 95),
        (18, 20, 30),
    ] {
        for level in [StdoutColorLevel::TrueColor, StdoutColorLevel::Ansi256] {
            let color = super::super::user_message_accent_color_for(Some(background), level);
            let surface = crate::terminal_palette::best_color_for_level(
                super::super::user_message_bg_rgb(background),
                level,
            );
            let resolve = |color| match color {
                Color::Rgb(r, g, b) => (r, g, b),
                Color::Indexed(index) => XTERM_COLORS[usize::from(index)],
                _ => panic!("expected explicit palette color"),
            };
            assert!(ratio(resolve(color), resolve(surface)) >= MIN_TEXT_CONTRAST);
        }
    }
}

#[test]
fn repeated_colors_follow_palette_changes_and_remain_bounded() {
    let preferred = (95, 175, 255);
    let backgrounds = [None, Some((24, 24, 24)), Some((245, 245, 245))];
    let levels = [
        StdoutColorLevel::TrueColor,
        StdoutColorLevel::Ansi256,
        StdoutColorLevel::Ansi16,
        StdoutColorLevel::Unknown,
    ];
    let expected =
        backgrounds.map(|background| levels.map(|level| foreground(preferred, background, level)));
    for index in 0..MAX_CACHED_FOREGROUNDS * 2 {
        let varied = (index as u8, (index / 256) as u8, 42);
        foreground(
            varied,
            /*background*/ None,
            StdoutColorLevel::TrueColor,
        );
        assert_eq!(
            backgrounds
                .map(|background| { levels.map(|level| foreground(preferred, background, level)) }),
            expected,
        );
        FOREGROUNDS.with(|cache| assert!(cache.borrow().len() <= MAX_CACHED_FOREGROUNDS));
    }
    assert_ne!(expected[1][0], expected[2][0]);
    assert_eq!(expected[0][0], rgb_color(preferred));
    assert_eq!(expected[0][2..], [Color::Reset, Color::Reset]);
}
