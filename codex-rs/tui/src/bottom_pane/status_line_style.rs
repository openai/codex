//! Theme-derived styling for the configurable footer statusline.

use ratatui::prelude::Stylize;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

use super::status_line_setup::StatusLineItem;
use crate::render::highlight::foreground_style_for_scopes;

const STATUS_LINE_SEPARATOR: &str = " · ";
const STATUS_LINE_COLOR_SATURATION_PERCENT: u16 = 85;
const STATUS_LINE_COLOR_BRIGHTNESS_PERCENT: u16 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatusLineAccent {
    Model,
    Path,
    Branch,
    State,
    Usage,
    Limit,
    Metadata,
    Mode,
    Thread,
    Progress,
}

impl StatusLineAccent {
    fn for_item(item: StatusLineItem) -> Self {
        match item {
            StatusLineItem::ModelName | StatusLineItem::ModelWithReasoning => Self::Model,
            StatusLineItem::CurrentDir | StatusLineItem::ProjectRoot => Self::Path,
            StatusLineItem::GitBranch => Self::Branch,
            StatusLineItem::Status => Self::State,
            StatusLineItem::ContextRemaining
            | StatusLineItem::ContextUsed
            | StatusLineItem::ContextWindowSize
            | StatusLineItem::UsedTokens
            | StatusLineItem::TotalInputTokens
            | StatusLineItem::TotalOutputTokens => Self::Usage,
            StatusLineItem::FiveHourLimit | StatusLineItem::WeeklyLimit => Self::Limit,
            StatusLineItem::CodexVersion | StatusLineItem::SessionId => Self::Metadata,
            StatusLineItem::FastMode => Self::Mode,
            StatusLineItem::ThreadTitle => Self::Thread,
            StatusLineItem::TaskProgress => Self::Progress,
        }
    }

    fn scopes(self) -> &'static [&'static str] {
        match self {
            Self::Model => &["entity.name.type", "support.type", "variable"],
            Self::Path => &["string", "markup.underline.link"],
            Self::Branch => &["entity.name.function", "entity.name.tag"],
            Self::State => &["keyword.control", "keyword"],
            Self::Usage => &["constant.numeric", "constant"],
            Self::Limit => &["constant.language", "storage.type"],
            Self::Metadata => &["comment", "constant.other"],
            Self::Mode => &["storage.modifier", "keyword.operator"],
            Self::Thread => &["markup.heading", "entity.name.section"],
            Self::Progress => &["markup.inserted", "constant.numeric"],
        }
    }

    fn fallback_style(self) -> Style {
        match self {
            Self::Model | Self::State | Self::Metadata | Self::Mode => Style::default().cyan(),
            Self::Path | Self::Usage | Self::Progress => Style::default().green(),
            Self::Branch | Self::Limit | Self::Thread => Style::default().magenta(),
        }
    }
}

pub(crate) fn status_line_from_segments<I>(
    segments: I,
    use_theme_colors: bool,
) -> Option<Line<'static>>
where
    I: IntoIterator<Item = (StatusLineItem, String)>,
{
    status_line_from_segments_with_resolver(segments, use_theme_colors, |accent| {
        foreground_style_for_scopes(accent.scopes())
    })
}

fn status_line_from_segments_with_resolver<I, F>(
    segments: I,
    use_theme_colors: bool,
    theme_style_for_accent: F,
) -> Option<Line<'static>>
where
    I: IntoIterator<Item = (StatusLineItem, String)>,
    F: Fn(StatusLineAccent) -> Option<Style>,
{
    let mut spans = Vec::new();
    for (item, text) in segments {
        if !spans.is_empty() {
            spans.push(STATUS_LINE_SEPARATOR.dim());
        }
        let style = if use_theme_colors {
            let accent = StatusLineAccent::for_item(item);
            soften_status_line_style(
                theme_style_for_accent(accent).unwrap_or_else(|| accent.fallback_style()),
            )
        } else {
            Style::default().dim()
        };
        spans.push(Span::styled(text, style));
    }

    (!spans.is_empty()).then(|| Line::from(spans))
}

fn soften_status_line_style(mut style: Style) -> Style {
    if let Some(fg) = style.fg {
        style.fg = Some(soften_status_line_color(fg));
    }
    style
}

#[allow(clippy::disallowed_methods)]
fn soften_status_line_color(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let luma = weighted_luma(r, g, b);
            Color::Rgb(
                soften_rgb_channel(r, luma),
                soften_rgb_channel(g, luma),
                soften_rgb_channel(b, luma),
            )
        }
        Color::LightRed => Color::Red,
        Color::LightGreen => Color::Green,
        Color::LightYellow => Color::Yellow,
        Color::LightBlue => Color::Blue,
        Color::LightMagenta => Color::Magenta,
        Color::LightCyan => Color::Cyan,
        Color::White => Color::Gray,
        Color::Reset
        | Color::Black
        | Color::Red
        | Color::Green
        | Color::Yellow
        | Color::Blue
        | Color::Magenta
        | Color::Cyan
        | Color::Gray
        | Color::DarkGray
        | Color::Indexed(_) => color,
    }
}

fn weighted_luma(r: u8, g: u8, b: u8) -> u16 {
    (77 * u16::from(r) + 150 * u16::from(g) + 29 * u16::from(b)) / 256
}

fn soften_rgb_channel(channel: u8, luma: u16) -> u8 {
    let channel = u16::from(channel);
    let softened = (channel * STATUS_LINE_COLOR_SATURATION_PERCENT
        + luma * (100 - STATUS_LINE_COLOR_SATURATION_PERCENT)
        + 50)
        / 100;

    ((softened * STATUS_LINE_COLOR_BRIGHTNESS_PERCENT + 50) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    fn line_text(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn status_line_segments_preserve_order_and_plain_text() {
        let line = status_line_from_segments_with_resolver(
            [
                (StatusLineItem::ModelName, "gpt-5".to_string()),
                (StatusLineItem::CurrentDir, "/repo".to_string()),
                (StatusLineItem::GitBranch, "main".to_string()),
            ],
            /*use_theme_colors*/ true,
            |_| None,
        )
        .expect("status line");

        assert_eq!(line_text(&line), "gpt-5 · /repo · main");
        assert_eq!(line.spans[0].style.fg, Some(Color::Cyan));
        assert!(!line.spans[0].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(line.spans[2].style.fg, Some(Color::Green));
        assert!(!line.spans[2].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(line.spans[4].style.fg, Some(Color::Magenta));
        assert!(!line.spans[4].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn status_line_segments_dim_separators_and_use_theme_styles_first() {
        let line = status_line_from_segments_with_resolver(
            [
                (StatusLineItem::ModelName, "gpt-5".to_string()),
                (StatusLineItem::ContextUsed, "Context 12% used".to_string()),
            ],
            /*use_theme_colors*/ true,
            |accent| match accent {
                StatusLineAccent::Model => Some(Style::default().red()),
                _ => None,
            },
        )
        .expect("status line");

        assert_eq!(line.spans[0].style.fg, Some(Color::Red));
        assert!(!line.spans[0].style.add_modifier.contains(Modifier::DIM));
        assert!(line.spans[1].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(line.spans[2].style.fg, Some(Color::Green));
        assert!(!line.spans[2].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn status_line_segments_soften_rgb_theme_styles_without_dimming_text() {
        let line = status_line_from_segments_with_resolver(
            [(StatusLineItem::ModelName, "gpt-5".to_string())],
            /*use_theme_colors*/ true,
            |_| Some(Style::default().fg(Color::Rgb(255, 0, 0))),
        )
        .expect("status line");

        assert_eq!(line.spans[0].style.fg, Some(Color::Rgb(228, 11, 11)));
        assert!(!line.spans[0].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn status_line_segments_can_disable_theme_colors() {
        let line = status_line_from_segments_with_resolver(
            [
                (StatusLineItem::ModelName, "gpt-5".to_string()),
                (StatusLineItem::ContextUsed, "Context 12% used".to_string()),
            ],
            /*use_theme_colors*/ false,
            |_| Some(Style::default().red()),
        )
        .expect("status line");

        assert_eq!(line_text(&line), "gpt-5 · Context 12% used");
        assert_eq!(line.spans[0].style.fg, None);
        assert!(line.spans[0].style.add_modifier.contains(Modifier::DIM));
        assert!(line.spans[1].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(line.spans[2].style.fg, None);
        assert!(line.spans[2].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn status_line_segments_return_none_when_empty() {
        assert_eq!(
            status_line_from_segments_with_resolver(
                Vec::<(StatusLineItem, String)>::new(),
                /*use_theme_colors*/ true,
                |_| None,
            ),
            None
        );
    }
}
