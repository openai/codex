use crate::color::is_light;
use crate::line_truncation::line_width;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::terminal_palette::default_bg;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled as _;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

const FOOTER_COMPACT_BREAKPOINT: u16 = 120;
const FOOTER_HINT_LEFT_PADDING: usize = 1;
const FOOTER_HINT_GAP: usize = 3;

#[derive(Clone, Debug)]
pub(crate) struct FooterHint {
    key: String,
    wide_label: String,
    compact_label: String,
    priority: u8,
}

impl FooterHint {
    pub(crate) fn new(
        key: impl Into<String>,
        wide_label: impl Into<String>,
        compact_label: impl Into<String>,
        priority: u8,
    ) -> Self {
        Self {
            key: key.into(),
            wide_label: wide_label.into(),
            compact_label: compact_label.into(),
            priority,
        }
    }
}

#[derive(Clone, Copy)]
enum FooterHintLabelMode {
    Wide,
    Compact,
    KeyOnly,
}

pub(crate) fn footer_hint_line_for_row(hints: &[FooterHint], width: u16) -> Line<'static> {
    if width >= FOOTER_COMPACT_BREAKPOINT
        && let Some(line) = fit_footer_hints(hints, FooterHintLabelMode::Wide, width)
    {
        return line;
    }
    if let Some(line) = fit_footer_hints(hints, FooterHintLabelMode::Compact, width) {
        return line;
    }
    if let Some(line) = fit_footer_hints(hints, FooterHintLabelMode::KeyOnly, width) {
        return line;
    }

    let mut retained = (0..hints.len()).collect::<Vec<_>>();
    retained.sort_by_key(|idx| hints[*idx].priority);
    for retain_count in (1..=retained.len()).rev() {
        let mut candidate_indices = retained[..retain_count].to_vec();
        candidate_indices.sort_unstable();
        let candidate = candidate_indices
            .iter()
            .map(|idx| &hints[*idx])
            .collect::<Vec<_>>();
        if let Some(line) = fit_footer_hint_refs(&candidate, FooterHintLabelMode::KeyOnly, width) {
            return line;
        }
    }
    Line::default()
}

pub(crate) fn render_footer_separator(area: Rect, buf: &mut Buffer, label: String) {
    if area.width == 0 {
        return;
    }

    Line::from("─".repeat(area.width as usize).dim()).render_ref(area, buf);
    if label.is_empty() {
        return;
    }

    let label_width = UnicodeWidthStr::width(label.as_str()) as u16;
    if label_width < area.width {
        let label_area = Rect::new(
            area.x + area.width - label_width - 1,
            area.y,
            label_width,
            1,
        );
        Line::from(label.dim()).render_ref(label_area, buf);
    }
}

pub(crate) fn render_footer_line_with_optional_right(
    area: Rect,
    buf: &mut Buffer,
    left: Line<'static>,
    right: Option<Line<'static>>,
) {
    let Some(right) = right else {
        left.render(area, buf);
        return;
    };
    if area.width == 0 {
        return;
    }

    let right_width = line_width(&right) as u16;
    if right_width > area.width {
        truncate_line_with_ellipsis_if_overflow(right, area.width as usize).render(area, buf);
        return;
    }

    let left_width = line_width(&left) as u16;
    let gap = u16::from(left_width > 0 && right_width > 0);
    let left_area_width = area.width.saturating_sub(right_width).saturating_sub(gap);
    if left_area_width == 0 {
        right.render(
            Rect {
                x: area.x + area.width - right_width,
                y: area.y,
                width: right_width,
                height: 1,
            },
            buf,
        );
        return;
    }

    truncate_line_with_ellipsis_if_overflow(left, left_area_width as usize).render(
        Rect {
            x: area.x,
            y: area.y,
            width: left_area_width,
            height: 1,
        },
        buf,
    );
    right.render(
        Rect {
            x: area.x + area.width - right_width,
            y: area.y,
            width: right_width,
            height: 1,
        },
        buf,
    );
}

fn fit_footer_hints(
    hints: &[FooterHint],
    mode: FooterHintLabelMode,
    width: u16,
) -> Option<Line<'static>> {
    let hint_refs = hints.iter().collect::<Vec<_>>();
    fit_footer_hint_refs(&hint_refs, mode, width)
}

fn fit_footer_hint_refs(
    hints: &[&FooterHint],
    mode: FooterHintLabelMode,
    width: u16,
) -> Option<Line<'static>> {
    let gap_width = FOOTER_HINT_GAP;
    if footer_hints_width(hints, mode, gap_width) > width as usize {
        return None;
    }

    let mut spans = vec![
        " ".repeat(FOOTER_HINT_LEFT_PADDING)
            .set_style(footer_hint_label_style()),
    ];
    for (idx, hint) in hints.iter().enumerate() {
        if idx > 0 {
            spans.push(" ".repeat(gap_width).set_style(footer_hint_label_style()));
        }
        spans.push(hint.key.clone().set_style(footer_hint_key_style()));
        let label = match mode {
            FooterHintLabelMode::Wide => Some(hint.wide_label.as_str()),
            FooterHintLabelMode::Compact => Some(hint.compact_label.as_str()),
            FooterHintLabelMode::KeyOnly => None,
        };
        if let Some(label) = label {
            spans.push(" ".set_style(footer_hint_label_style()));
            spans.push(label.to_string().set_style(footer_hint_label_style()));
        }
    }
    Some(spans.into())
}

fn footer_hint_key_style() -> Style {
    if default_bg().is_some_and(is_light) {
        Style::default().fg(Color::Black)
    } else {
        Style::default()
    }
}

fn footer_hint_label_style() -> Style {
    if default_bg().is_some_and(is_light) {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().dim()
    }
}

fn footer_hints_width(hints: &[&FooterHint], mode: FooterHintLabelMode, gap_width: usize) -> usize {
    FOOTER_HINT_LEFT_PADDING
        + hints
            .iter()
            .enumerate()
            .map(|(idx, hint)| {
                let label_width = match mode {
                    FooterHintLabelMode::Wide => {
                        1 + UnicodeWidthStr::width(hint.wide_label.as_str())
                    }
                    FooterHintLabelMode::Compact => {
                        1 + UnicodeWidthStr::width(hint.compact_label.as_str())
                    }
                    FooterHintLabelMode::KeyOnly => 0,
                };
                let hint_width = UnicodeWidthStr::width(hint.key.as_str()) + label_width;
                if idx == 0 {
                    hint_width
                } else {
                    hint_width + gap_width
                }
            })
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn buffer_text(buf: &Buffer, area: Rect) -> String {
        let mut out = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let symbol = buf[(x, y)].symbol();
                out.push(symbol.chars().next().unwrap_or(' '));
            }
        }
        out
    }

    fn line_text(line: Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn footer_hint_line_uses_wide_labels_when_width_allows() {
        let hints = [FooterHint::new(
            "enter",
            "resume session",
            "resume",
            /*priority*/ 0,
        )];

        let rendered = line_text(footer_hint_line_for_row(&hints, /*width*/ 140));

        assert!(rendered.contains("enter resume session"));
    }

    #[test]
    fn footer_hint_line_compacts_below_breakpoint() {
        let hints = [FooterHint::new(
            "enter",
            "resume session",
            "resume",
            /*priority*/ 0,
        )];

        let rendered = line_text(footer_hint_line_for_row(&hints, /*width*/ 80));

        assert!(rendered.contains("enter resume"));
        assert!(!rendered.contains("resume session"));
    }

    #[test]
    fn footer_hint_line_drops_low_priority_hints_when_narrow() {
        let hints = [
            FooterHint::new("a", "alpha", "alpha", /*priority*/ 0),
            FooterHint::new("b", "bravo", "bravo", /*priority*/ 9),
            FooterHint::new("c", "charlie", "charlie", /*priority*/ 1),
        ];

        let rendered = line_text(footer_hint_line_for_row(&hints, /*width*/ 6));

        assert!(rendered.contains('a'));
        assert!(rendered.contains('c'));
        assert!(!rendered.contains('b'));
    }

    #[test]
    fn footer_line_renders_left_and_right_when_both_fit() {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 1,
        );
        let mut buf = Buffer::empty(area);

        render_footer_line_with_optional_right(
            area,
            &mut buf,
            Line::from("left"),
            Some(Line::from("right")),
        );

        assert_eq!(buffer_text(&buf, area), "left               right");
    }

    #[test]
    fn footer_line_truncates_left_when_right_fits() {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 16, /*height*/ 1,
        );
        let mut buf = Buffer::empty(area);

        render_footer_line_with_optional_right(
            area,
            &mut buf,
            Line::from("long left status"),
            Some(Line::from("ok")),
        );

        assert_eq!(buffer_text(&buf, area), "long left st… ok");
    }

    #[test]
    fn footer_line_renders_right_only_when_space_is_tight() {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 5, /*height*/ 1,
        );
        let mut buf = Buffer::empty(area);

        render_footer_line_with_optional_right(
            area,
            &mut buf,
            Line::from("left"),
            Some(Line::from("right")),
        );

        assert_eq!(buffer_text(&buf, area), "right");
    }

    #[test]
    fn footer_line_truncates_right_when_it_overflows_area() {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 4, /*height*/ 1,
        );
        let mut buf = Buffer::empty(area);

        render_footer_line_with_optional_right(
            area,
            &mut buf,
            Line::from("left"),
            Some(Line::from("status")),
        );

        assert_eq!(buffer_text(&buf, area), "sta…");
    }
}
