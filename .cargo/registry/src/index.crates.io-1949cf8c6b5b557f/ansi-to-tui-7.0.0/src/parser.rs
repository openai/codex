use crate::code::AnsiCode;
use nom::{
    branch::alt,
    bytes::complete::*,
    character::complete::*,
    character::is_alphabetic,
    combinator::{map_res, opt, recognize, value},
    error,
    error::FromExternalError,
    multi::*,
    sequence::{delimited, preceded, terminated, tuple},
    IResult, Parser,
};
use std::str::FromStr;
use tui::{
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ColorType {
    /// Eight Bit color
    EightBit,
    /// 24-bit color or true color
    TrueColor,
}

#[derive(Debug, Clone, PartialEq)]
struct AnsiItem {
    code: AnsiCode,
    color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq)]
struct AnsiStates {
    pub items: smallvec::SmallVec<[AnsiItem; 2]>,
    pub style: Style,
}

impl From<AnsiStates> for tui::style::Style {
    fn from(states: AnsiStates) -> Self {
        let mut style = states.style;
        if states.items.is_empty() {
            // https://github.com/uttarayan21/ansi-to-tui/issues/40
            // [m should be treated as a reset as well
            style = Style::reset();
        }
        for item in states.items {
            match item.code {
                AnsiCode::Reset => style = Style::reset(),
                AnsiCode::Bold => style = style.add_modifier(Modifier::BOLD),
                AnsiCode::Faint => style = style.add_modifier(Modifier::DIM),
                AnsiCode::Normal => {
                    style = style
                        .remove_modifier(Modifier::BOLD)
                        .remove_modifier(Modifier::DIM);
                }
                AnsiCode::Italic => style = style.add_modifier(Modifier::ITALIC),
                AnsiCode::Underline => style = style.add_modifier(Modifier::UNDERLINED),
                AnsiCode::SlowBlink => style = style.add_modifier(Modifier::SLOW_BLINK),
                AnsiCode::RapidBlink => style = style.add_modifier(Modifier::RAPID_BLINK),
                AnsiCode::Reverse => style = style.add_modifier(Modifier::REVERSED),
                AnsiCode::Conceal => style = style.add_modifier(Modifier::HIDDEN),
                AnsiCode::CrossedOut => style = style.add_modifier(Modifier::CROSSED_OUT),
                AnsiCode::DefaultForegroundColor => style = style.fg(Color::Reset),
                AnsiCode::DefaultBackgroundColor => style = style.bg(Color::Reset),
                AnsiCode::SetForegroundColor => {
                    if let Some(color) = item.color {
                        style = style.fg(color)
                    }
                }
                AnsiCode::SetBackgroundColor => {
                    if let Some(color) = item.color {
                        style = style.bg(color)
                    }
                }
                AnsiCode::ForegroundColor(color) => style = style.fg(color),
                AnsiCode::BackgroundColor(color) => style = style.bg(color),
                _ => (),
            }
        }
        style
    }
}

pub(crate) fn text(mut s: &[u8]) -> IResult<&[u8], Text<'static>> {
    let mut lines = Vec::new();
    let mut last = Style::new();
    while let Ok((_s, (line, style))) = line(last)(s) {
        lines.push(line);
        last = style;
        s = _s;
        if s.is_empty() {
            break;
        }
    }
    Ok((s, Text::from(lines)))
}

#[cfg(feature = "zero-copy")]
pub(crate) fn text_fast(mut s: &[u8]) -> IResult<&[u8], Text<'_>> {
    let mut lines = Vec::new();
    let mut last = Style::new();
    while let Ok((_s, (line, style))) = line_fast(last)(s) {
        lines.push(line);
        last = style;
        s = _s;
        if s.is_empty() {
            break;
        }
    }
    Ok((s, Text::from(lines)))
}

fn line(style: Style) -> impl Fn(&[u8]) -> IResult<&[u8], (Line<'static>, Style)> {
    // let style_: Style = Default::default();
    move |s: &[u8]| -> IResult<&[u8], (Line<'static>, Style)> {
        let (s, mut text) = take_while(|c| c != b'\n')(s)?;
        let (s, _) = opt(tag("\n"))(s)?;
        let mut spans = Vec::new();
        let mut last = style;
        while let Ok((s, span)) = span(last)(text) {
            // Since reset now tracks seperately we can skip the reset check
            last = last.patch(span.style);

            if !span.content.is_empty() {
                spans.push(span);
            }
            text = s;
            if text.is_empty() {
                break;
            }
        }

        Ok((s, (Line::from(spans), last)))
    }
}

#[cfg(feature = "zero-copy")]
fn line_fast(style: Style) -> impl Fn(&[u8]) -> IResult<&[u8], (Line<'_>, Style)> {
    // let style_: Style = Default::default();
    move |s: &[u8]| -> IResult<&[u8], (Line<'_>, Style)> {
        let (s, mut text) = take_while(|c| c != b'\n')(s)?;
        let (s, _) = opt(tag("\n"))(s)?;
        let mut spans = Vec::new();
        let mut last = style;
        while let Ok((s, span)) = span_fast(last)(text) {
            last = last.patch(span.style);
            // If the spans is empty then it might be possible that the style changes
            // but there is no text change
            if !span.content.is_empty() {
                spans.push(span);
            }
            text = s;
            if text.is_empty() {
                break;
            }
        }

        Ok((s, (Line::from(spans), last)))
    }
}

// fn span(s: &[u8]) -> IResult<&[u8], tui::text::Span> {
fn span(last: Style) -> impl Fn(&[u8]) -> IResult<&[u8], Span<'static>, nom::error::Error<&[u8]>> {
    move |s: &[u8]| -> IResult<&[u8], Span<'static>> {
        let mut last = last;
        let (s, style) = opt(style(last))(s)?;

        #[cfg(feature = "simd")]
        let (s, text) = map_res(take_while(|c| c != b'\x1b' && c != b'\n'), |t| {
            simdutf8::basic::from_utf8(t)
        })(s)?;

        #[cfg(not(feature = "simd"))]
        let (s, text) = map_res(take_while(|c| c != b'\x1b' && c != b'\n'), |t| {
            std::str::from_utf8(t)
        })(s)?;

        if let Some(style) = style.flatten() {
            last = last.patch(style);
        }

        Ok((s, Span::styled(text.to_owned(), last)))
    }
}

#[cfg(feature = "zero-copy")]
fn span_fast(last: Style) -> impl Fn(&[u8]) -> IResult<&[u8], Span<'_>, nom::error::Error<&[u8]>> {
    move |s: &[u8]| -> IResult<&[u8], Span<'_>> {
        let mut last = last;
        let (s, style) = opt(style(last))(s)?;

        #[cfg(feature = "simd")]
        let (s, text) = map_res(take_while(|c| c != b'\x1b' && c != b'\n'), |t| {
            simdutf8::basic::from_utf8(t)
        })(s)?;

        #[cfg(not(feature = "simd"))]
        let (s, text) = map_res(take_while(|c| c != b'\x1b' && c != b'\n'), |t| {
            std::str::from_utf8(t)
        })(s)?;

        if let Some(style) = style.flatten() {
            last = last.patch(style);
        }

        Ok((s, Span::styled(text, last)))
    }
}

fn style(
    style: Style,
) -> impl Fn(&[u8]) -> IResult<&[u8], Option<Style>, nom::error::Error<&[u8]>> {
    move |s: &[u8]| -> IResult<&[u8], Option<Style>> {
        let (s, r) = match opt(ansi_sgr_code)(s)? {
            (s, Some(r)) => (s, Some(r)),
            (s, None) => {
                let (s, _) = any_escape_sequence(s)?;
                (s, None)
            }
        };
        Ok((s, r.map(|r| Style::from(AnsiStates { style, items: r }))))
    }
}

/// A complete ANSI SGR code
fn ansi_sgr_code(
    s: &[u8],
) -> IResult<&[u8], smallvec::SmallVec<[AnsiItem; 2]>, nom::error::Error<&[u8]>> {
    delimited(
        tag("\x1b["),
        fold_many0(ansi_sgr_item, smallvec::SmallVec::new, |mut items, item| {
            items.push(item);
            items
        }),
        char('m'),
    )(s)
}

fn any_escape_sequence(s: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
    // Attempt to consume most escape codes, including a single escape char.
    //
    // Most escape codes begin with ESC[ and are terminated by an alphabetic character,
    // but OSC codes begin with ESC] and are terminated by an ascii bell (\x07)
    // and a truncated/invalid code may just be a standalone ESC or not be terminated.
    //
    // We should try to consume as much of it as possible to match behavior of most terminals;
    // where we fail at that we should at least consume the escape char to avoid infinitely looping

    let (input, garbage) = preceded(
        char('\x1b'),
        opt(alt((
            delimited(char('['), take_till(is_alphabetic), opt(take(1u8))),
            delimited(char(']'), take_till(|c| c == b'\x07'), opt(take(1u8))),
        ))),
    )(s)?;
    Ok((input, garbage))
}

/// An ANSI SGR attribute
fn ansi_sgr_item(s: &[u8]) -> IResult<&[u8], AnsiItem> {
    let (s, c) = u8(s)?;
    let code = AnsiCode::from(c);
    let (s, color) = match code {
        AnsiCode::SetForegroundColor | AnsiCode::SetBackgroundColor => {
            let (s, _) = opt(tag(";"))(s)?;
            let (s, color) = color(s)?;
            (s, Some(color))
        }
        _ => (s, None),
    };
    let (s, _) = opt(tag(";"))(s)?;
    Ok((s, AnsiItem { code, color }))
}

fn color(s: &[u8]) -> IResult<&[u8], Color> {
    let (s, c_type) = color_type(s)?;
    let (s, _) = opt(tag(";"))(s)?;
    match c_type {
        ColorType::TrueColor => {
            let (s, (r, _, g, _, b)) = tuple((u8, tag(";"), u8, tag(";"), u8))(s)?;
            Ok((s, Color::Rgb(r, g, b)))
        }
        ColorType::EightBit => {
            let (s, index) = u8(s)?;
            Ok((s, Color::Indexed(index)))
        }
    }
}

fn color_type(s: &[u8]) -> IResult<&[u8], ColorType> {
    let (s, t) = i64(s)?;
    // NOTE: This isn't opt because a color type must always be followed by a color
    // let (s, _) = opt(tag(";"))(s)?;
    let (s, _) = tag(";")(s)?;
    match t {
        2 => Ok((s, ColorType::TrueColor)),
        5 => Ok((s, ColorType::EightBit)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            s,
            nom::error::ErrorKind::Alt,
        ))),
    }
}

#[test]
fn color_test() {
    let c = color(b"2;255;255;255").unwrap();
    assert_eq!(c.1, Color::Rgb(255, 255, 255));
    let c = color(b"5;255").unwrap();
    assert_eq!(c.1, Color::Indexed(255));
    let err = color(b"10;255");
    assert_ne!(err, Ok(c));
}

#[test]
fn ansi_items_test() {
    let sc = Default::default();
    let t = style(sc)(b"\x1b[38;2;3;3;3m").unwrap().1.unwrap();
    assert_eq!(
        t,
        Style::from(AnsiStates {
            style: sc,
            items: vec![AnsiItem {
                code: AnsiCode::SetForegroundColor,
                color: Some(Color::Rgb(3, 3, 3))
            }]
            .into()
        })
    );
    assert_eq!(
        style(sc)(b"\x1b[38;5;3m").unwrap().1.unwrap(),
        Style::from(AnsiStates {
            style: sc,
            items: vec![AnsiItem {
                code: AnsiCode::SetForegroundColor,
                color: Some(Color::Indexed(3))
            }]
            .into()
        })
    );
    assert_eq!(
        style(sc)(b"\x1b[38;5;3;48;5;3m").unwrap().1.unwrap(),
        Style::from(AnsiStates {
            style: sc,
            items: vec![
                AnsiItem {
                    code: AnsiCode::SetForegroundColor,
                    color: Some(Color::Indexed(3))
                },
                AnsiItem {
                    code: AnsiCode::SetBackgroundColor,
                    color: Some(Color::Indexed(3))
                }
            ]
            .into()
        })
    );
    assert_eq!(
        style(sc)(b"\x1b[38;5;3;48;5;3;1m").unwrap().1.unwrap(),
        Style::from(AnsiStates {
            style: sc,
            items: vec![
                AnsiItem {
                    code: AnsiCode::SetForegroundColor,
                    color: Some(Color::Indexed(3))
                },
                AnsiItem {
                    code: AnsiCode::SetBackgroundColor,
                    color: Some(Color::Indexed(3))
                },
                AnsiItem {
                    code: AnsiCode::Bold,
                    color: None
                }
            ]
            .into()
        })
    );
}
