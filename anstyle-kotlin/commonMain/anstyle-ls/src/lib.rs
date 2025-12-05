//! `anstyle_ls::parse` parses a color configuration string (in `LS_COLORS` syntax)
//! into an `anstyle::Style`:
//!
//! # Examples
//!
//! ```rust
//! let style = anstyle_ls::parse("34;03").unwrap();
//! assert_eq!(style, anstyle::AnsiColor::Blue.on_default() | anstyle::Effects::ITALIC);
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Parse a string in `LS_COLORS`'s color configuration syntax into an
/// `ansi_term::Style`.
pub fn parse(code: &str) -> Option<anstyle::Style> {
    if code.is_empty() || code == "0" || code == "00" {
        return None;
    }

    let mut parts: std::collections::VecDeque<u8> = code
        .split(';')
        .map(|c| c.parse::<u8>().ok())
        .collect::<Option<_>>()?;

    let mut effects = anstyle::Effects::new();
    let mut fg_color: Option<anstyle::Color> = None;
    let mut bg_color: Option<anstyle::Color> = None;
    let mut underline_color: Option<anstyle::Color> = None;

    while let Some(part) = parts.pop_front() {
        match part {
            0 => {
                effects = Default::default();
                fg_color = Default::default();
                bg_color = Default::default();
                underline_color = Default::default();
            }
            1 => effects |= anstyle::Effects::BOLD,
            2 => effects |= anstyle::Effects::DIMMED,
            3 => effects |= anstyle::Effects::ITALIC,
            4 => effects |= anstyle::Effects::UNDERLINE,
            5 => effects |= anstyle::Effects::BLINK,
            6 => effects |= anstyle::Effects::BLINK,
            7 => effects |= anstyle::Effects::INVERT,
            8 => effects |= anstyle::Effects::HIDDEN,
            9 => effects |= anstyle::Effects::STRIKETHROUGH,
            22 => {
                effects = effects
                    .remove(anstyle::Effects::BOLD)
                    .remove(anstyle::Effects::DIMMED);
            }
            23 => {
                effects = effects.remove(anstyle::Effects::ITALIC);
            }
            24 => {
                effects = effects.remove(anstyle::Effects::UNDERLINE);
            }
            25 => {
                effects = effects.remove(anstyle::Effects::BLINK);
            }
            27 => {
                effects = effects.remove(anstyle::Effects::INVERT);
            }
            28 => {
                effects = effects.remove(anstyle::Effects::HIDDEN);
            }
            29 => {
                effects = effects.remove(anstyle::Effects::STRIKETHROUGH);
            }
            30 => fg_color = Some(anstyle::AnsiColor::Black.into()),
            31 => fg_color = Some(anstyle::AnsiColor::Red.into()),
            32 => fg_color = Some(anstyle::AnsiColor::Green.into()),
            33 => fg_color = Some(anstyle::AnsiColor::Yellow.into()),
            34 => fg_color = Some(anstyle::AnsiColor::Blue.into()),
            35 => fg_color = Some(anstyle::AnsiColor::Magenta.into()),
            36 => fg_color = Some(anstyle::AnsiColor::Cyan.into()),
            37 => fg_color = Some(anstyle::AnsiColor::White.into()),
            38 => match (parts.pop_front(), parts.pop_front()) {
                (Some(5), Some(color)) => fg_color = Some(anstyle::Ansi256Color(color).into()),
                (Some(2), Some(red)) => match (parts.pop_front(), parts.pop_front()) {
                    (Some(green), Some(blue)) => {
                        fg_color = Some(anstyle::RgbColor(red, green, blue).into());
                    }
                    _ => {
                        break;
                    }
                },
                _ => {
                    break;
                }
            },
            39 => fg_color = None,
            40 => bg_color = Some(anstyle::AnsiColor::Black.into()),
            41 => bg_color = Some(anstyle::AnsiColor::Red.into()),
            42 => bg_color = Some(anstyle::AnsiColor::Green.into()),
            43 => bg_color = Some(anstyle::AnsiColor::Yellow.into()),
            44 => bg_color = Some(anstyle::AnsiColor::Blue.into()),
            45 => bg_color = Some(anstyle::AnsiColor::Magenta.into()),
            46 => bg_color = Some(anstyle::AnsiColor::Cyan.into()),
            47 => bg_color = Some(anstyle::AnsiColor::White.into()),
            48 => match (parts.pop_front(), parts.pop_front()) {
                (Some(5), Some(color)) => bg_color = Some(anstyle::Ansi256Color(color).into()),
                (Some(2), Some(red)) => match (parts.pop_front(), parts.pop_front()) {
                    (Some(green), Some(blue)) => {
                        bg_color = Some(anstyle::RgbColor(red, green, blue).into());
                    }
                    _ => {
                        break;
                    }
                },
                _ => {
                    break;
                }
            },
            49 => bg_color = None,
            58 => match (parts.pop_front(), parts.pop_front()) {
                (Some(5), Some(color)) => {
                    underline_color = Some(anstyle::Ansi256Color(color).into());
                }
                (Some(2), Some(red)) => match (parts.pop_front(), parts.pop_front()) {
                    (Some(green), Some(blue)) => {
                        underline_color = Some(anstyle::RgbColor(red, green, blue).into());
                    }
                    _ => {
                        break;
                    }
                },
                _ => {
                    break;
                }
            },
            59 => underline_color = None,
            90 => fg_color = Some(anstyle::AnsiColor::BrightBlack.into()),
            91 => fg_color = Some(anstyle::AnsiColor::BrightRed.into()),
            92 => fg_color = Some(anstyle::AnsiColor::BrightGreen.into()),
            93 => fg_color = Some(anstyle::AnsiColor::BrightYellow.into()),
            94 => fg_color = Some(anstyle::AnsiColor::BrightBlue.into()),
            95 => fg_color = Some(anstyle::AnsiColor::BrightMagenta.into()),
            96 => fg_color = Some(anstyle::AnsiColor::BrightCyan.into()),
            97 => fg_color = Some(anstyle::AnsiColor::BrightWhite.into()),
            100 => bg_color = Some(anstyle::AnsiColor::BrightBlack.into()),
            101 => bg_color = Some(anstyle::AnsiColor::BrightRed.into()),
            102 => bg_color = Some(anstyle::AnsiColor::BrightGreen.into()),
            103 => bg_color = Some(anstyle::AnsiColor::BrightYellow.into()),
            104 => bg_color = Some(anstyle::AnsiColor::BrightBlue.into()),
            105 => bg_color = Some(anstyle::AnsiColor::BrightMagenta.into()),
            106 => bg_color = Some(anstyle::AnsiColor::BrightCyan.into()),
            107 => bg_color = Some(anstyle::AnsiColor::BrightWhite.into()),
            _ => {}
        }
    }

    Some(
        anstyle::Style::new()
            .fg_color(fg_color)
            .bg_color(bg_color)
            .underline_color(underline_color)
            .effects(effects),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_style(code: &str, expected: impl Into<anstyle::Style>) {
        let actual = parse(code).unwrap();
        let expected = expected.into();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_simple() {
        assert_style("31", anstyle::AnsiColor::Red.on_default());
        assert_style(
            "47",
            anstyle::Style::new().bg_color(Some(anstyle::AnsiColor::White.into())),
        );
        assert_style("91", anstyle::AnsiColor::BrightRed.on_default());
        assert_style(
            "107",
            anstyle::Style::new().bg_color(Some(anstyle::AnsiColor::BrightWhite.into())),
        );
        assert_style(
            "32;40",
            anstyle::AnsiColor::Green.on(anstyle::AnsiColor::Black),
        );
    }

    #[test]
    fn parse_reject() {
        assert_eq!(None, parse("a"));
        assert_eq!(None, parse("1;"));
        assert_eq!(None, parse("33; 42"));
    }

    #[test]
    fn parse_font_style() {
        assert_style("00;31", anstyle::AnsiColor::Red.on_default());
        assert_style(
            "03;34",
            anstyle::AnsiColor::Blue.on_default() | anstyle::Effects::ITALIC,
        );
        assert_style(
            "06;34",
            anstyle::AnsiColor::Blue.on_default() | anstyle::Effects::BLINK,
        );
        assert_style(
            "01;36",
            anstyle::AnsiColor::Cyan.on_default() | anstyle::Effects::BOLD,
        );
        assert_style("01;03", anstyle::Effects::BOLD | anstyle::Effects::ITALIC);
    }

    #[test]
    fn ignore_unsupported_styles() {
        assert_style("14;31", anstyle::AnsiColor::Red.on_default());
    }

    #[test]
    fn support_reset_of_styles() {
        assert_style(
            "01;31",
            anstyle::AnsiColor::Red.on_default() | anstyle::Effects::BOLD,
        );
        assert_style("01;31;22", anstyle::AnsiColor::Red.on_default());
    }

    #[test]
    fn parse_font_style_backwards() {
        assert_style(
            "34;03",
            anstyle::AnsiColor::Blue.on_default() | anstyle::Effects::ITALIC,
        );
        assert_style(
            "36;01",
            anstyle::AnsiColor::Cyan.on_default() | anstyle::Effects::BOLD,
        );
        assert_style("31;00", anstyle::Style::new());
    }

    #[test]
    fn parse_8_bit_colors() {
        assert_style("38;5;115", anstyle::Ansi256Color(115).on_default());
        assert_style("00;38;5;115", anstyle::Ansi256Color(115).on_default());
        assert_style(
            "01;38;5;119",
            anstyle::Ansi256Color(119).on_default() | anstyle::Effects::BOLD,
        );
        assert_style(
            "38;5;119;01",
            anstyle::Ansi256Color(119).on_default() | anstyle::Effects::BOLD,
        );
        assert_style(
            "58;5;115",
            anstyle::Style::new().underline_color(Some(anstyle::Ansi256Color(115).into())),
        );
        assert_style(
            "00;58;5;115",
            anstyle::Style::new().underline_color(Some(anstyle::Ansi256Color(115).into())),
        );
        assert_style(
            "01;58;5;119",
            anstyle::Style::new().underline_color(Some(anstyle::Ansi256Color(119).into()))
                | anstyle::Effects::BOLD,
        );
    }

    #[test]
    fn parse_24_bit_colors() {
        assert_style(
            "38;2;115;3;100",
            anstyle::RgbColor(115, 3, 100).on_default(),
        );
        assert_style(
            "38;2;115;3;100;3",
            anstyle::RgbColor(115, 3, 100).on_default() | anstyle::Effects::ITALIC,
        );
        assert_style(
            "48;2;100;200;0;1;38;2;0;10;20",
            anstyle::RgbColor(0, 10, 20).on(anstyle::RgbColor(100, 200, 0))
                | anstyle::Effects::BOLD,
        );
        assert_style(
            "48;2;100;200;0;1;38;2;0;10;20;58;2;64;64;64",
            (anstyle::RgbColor(0, 10, 20).on(anstyle::RgbColor(100, 200, 0))
                | anstyle::Effects::BOLD)
                .underline_color(Some(anstyle::RgbColor(64, 64, 64).into())),
        );
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
