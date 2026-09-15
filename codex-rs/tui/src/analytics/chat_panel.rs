//! Bounded chat tables with inline credit breakdowns and a viewport-sized row window.
//! Zero-credit groups are optional; missing dimensions and signed adjustments stay distinct.

use super::AnalyticsView;
use super::data;
use super::render::columns;
use super::styles::number;
use super::styles::secondary_style;
use crate::analytics::sections::Section;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::style::accent_style;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::ops::Range;

impl AnalyticsView {
    pub(super) fn chat_lines(&self, width: usize) -> (Vec<Line<'static>>, Range<usize>) {
        let width = width.clamp(/*min*/ 1, /*max*/ 96);
        let wrap = |lines: Vec<Line<'static>>| word_wrap_lines(lines, RtOptions::new(width));
        let mut lines = wrap(vec![
            "Active in past 30 days · sorted by lifetime credits"
                .set_style(secondary_style())
                .into(),
        ]);
        if let Some(message) = self.chats.message() {
            lines.extend(wrap(vec![
                message.to_string().set_style(secondary_style()).into(),
            ]));
        }
        let Some(chats) = self.chats.ready() else {
            return (lines, 0..1);
        };
        if chats.rows.is_empty() {
            lines.extend(wrap(vec![
                "No recent local chats.".set_style(secondary_style()).into(),
            ]));
            return (lines, 0..1);
        }
        let missing = chats.rows.iter().filter(|row| row.usage.is_none()).count();
        if missing > 0 {
            lines.extend(wrap(vec![
                format!(
                    "{missing} chat estimate{} unavailable",
                    if missing == 1 { "" } else { "s" }
                )
                .set_style(secondary_style())
                .into(),
            ]));
        }
        lines.push(Line::default());
        let show_usd = chats.rows.iter().any(|chat| {
            chat.usage
                .as_ref()
                .is_some_and(|usage| usage.estimated_usage_usd_micros.is_some())
        });
        let stacked_amounts = show_usd && width < 70;
        lines.extend(wrap(vec![columns(
            "  Chat".bold().into(),
            if stacked_amounts {
                "Est. $ / credits"
            } else if show_usd {
                "Est. $ spent  Lifetime credits"
            } else {
                "Lifetime credits"
            }
            .bold()
            .into(),
            width,
        )]));
        let rows = chats
            .rows
            .iter()
            .enumerate()
            .map(|(index, chat)| {
                let selected =
                    self.section == Section::Chats && index == self.sections[Section::Chats].cursor;
                let amount = chat.usage.as_ref().map_or_else(
                    || "—".into(),
                    |usage| data::credits(usage.estimated_usage_credits_micros),
                );
                let amount = if show_usd {
                    let usd = chat
                        .usage
                        .as_ref()
                        .and_then(|usage| usage.estimated_usage_usd_micros)
                        .map(|micros| format!("${:.2}", micros as f64 / 1_000_000.0))
                        .unwrap_or_else(|| "—".into());
                    format!("{usd:>12}  {amount:>16}")
                } else {
                    amount
                };
                let label = format!(
                    "{} {}",
                    if selected { "›" } else { " " },
                    chat.display_title()
                );
                let label: Line<'static> = if selected {
                    label.set_style(accent_style()).into()
                } else {
                    label.into()
                };
                let label_width = if stacked_amounts {
                    width
                } else {
                    width.saturating_sub(amount.len() + 1)
                };
                let truncated = label.width() > label_width;
                let mut row = if stacked_amounts {
                    vec![
                        truncate_line_with_ellipsis_if_overflow(label, label_width),
                        format!("  {}", amount.trim()).into(),
                    ]
                } else {
                    vec![columns(
                        truncate_line_with_ellipsis_if_overflow(label, label_width),
                        if chat.usage.is_none() {
                            amount.set_style(secondary_style()).into()
                        } else if selected {
                            number(amount).into()
                        } else {
                            amount.into()
                        },
                        width,
                    )]
                };
                if self.sections[Section::Chats].detail == Some(index)
                    && let Some(usage) = &chat.usage
                {
                    row.push(
                        format!("  {}", "─".repeat(width.saturating_sub(/*rhs*/ 2)))
                            .dim()
                            .into(),
                    );
                    if truncated {
                        row.extend(word_wrap_lines(
                            [chat.display_title().to_string().bold()],
                            RtOptions::new(width)
                                .initial_indent("    ".into())
                                .subsequent_indent("    ".into()),
                        ));
                    }
                    let zeros = usage
                        .groups
                        .iter()
                        .filter(|group| group.estimated_usage_credits_micros == 0)
                        .count();
                    let mut groups = usage
                        .groups
                        .iter()
                        .filter(|group| {
                            self.show_zero_credit_groups
                                || group.estimated_usage_credits_micros != 0
                        })
                        .collect::<Vec<_>>();
                    groups.sort_by_key(|group| {
                        std::cmp::Reverse(group.estimated_usage_credits_micros)
                    });
                    // On narrow terminals, retain every dimension on a second line.
                    let table = width >= 70;
                    if !groups.is_empty() {
                        row.push(columns(
                            if table {
                                format!(
                                    "  {:<model_width$}  {:<12}  {:<12}",
                                    "Model",
                                    "Effort",
                                    "Speed",
                                    model_width = width - 46
                                )
                                .set_style(secondary_style())
                                .into()
                            } else {
                                "  Model / effort / speed"
                                    .set_style(secondary_style())
                                    .into()
                            },
                            "Credits".bold().into(),
                            width,
                        ));
                    }
                    for group in groups {
                        let model =
                            self.model_name(group.model.as_deref().unwrap_or("Not reported"));
                        let effort = group.reasoning_effort.as_deref().unwrap_or("Not reported");
                        let speed = group.speed.as_deref().unwrap_or("Not reported");
                        let amount = data::credits(group.estimated_usage_credits_micros);
                        let label = if table {
                            // Wrap long model names instead of hiding distinctions between groups.
                            let model_width = width - 46;
                            let models = textwrap::wrap(model, model_width);
                            for model in models.iter().take(models.len().saturating_sub(/*rhs*/ 1))
                            {
                                row.push(format!("  {model}").bold().into());
                            }
                            vec![
                                format!(
                                    "  {:<model_width$}",
                                    models
                                        .last()
                                        .map(std::convert::AsRef::as_ref)
                                        .unwrap_or_default()
                                )
                                .bold(),
                                format!("  {effort:<12}  {speed:<12}").set_style(secondary_style()),
                            ]
                            .into()
                        } else {
                            let label = Line::from(format!("  {model}").bold());
                            if label.width() + amount.len() + 1 > width {
                                row.push(label);
                                "  ".into()
                            } else {
                                label
                            }
                        };
                        row.push(columns(label, amount.into(), width));
                        if !table {
                            row.push(
                                format!("    {effort} · {speed}")
                                    .set_style(secondary_style())
                                    .into(),
                            );
                        }
                    }
                    if usage.groups.is_empty() {
                        row.push(
                            "  Breakdown unavailable"
                                .set_style(secondary_style())
                                .into(),
                        );
                    }
                    if zeros > 0 {
                        row.push(
                            format!(
                                "  {zeros} zero-credit {}{} · a {}",
                                if zeros == 1 { "group" } else { "groups" },
                                if self.show_zero_credit_groups {
                                    ""
                                } else {
                                    " hidden"
                                },
                                if self.show_zero_credit_groups {
                                    "hide zeros"
                                } else {
                                    "show all"
                                },
                            )
                            .set_style(secondary_style())
                            .into(),
                        );
                    }
                    row.push(Line::default());
                }
                wrap(row)
            })
            .collect::<Vec<_>>();
        let coverage = wrap(vec![
            "Local chats · excludes archived chats and subagent usage"
                .set_style(secondary_style())
                .into(),
        ]);
        self.chat_window(lines, rows, coverage, width)
    }

    pub(super) fn chat_window(
        &self,
        mut lines: Vec<Line<'static>>,
        rows: Vec<Vec<Line<'static>>>,
        coverage: Vec<Line<'static>>,
        width: usize,
    ) -> (Vec<Line<'static>>, Range<usize>) {
        let wrap = |lines: Vec<Line<'static>>| word_wrap_lines(lines, RtOptions::new(width));
        let count = rows.len();
        let range_height = wrap(vec![
            format!("Showing {count}–{count} of {count} chats").into(),
        ])
        .len();
        // Measure wrapped details before choosing neighbors so the range describes the rendered rows.
        let budget = if self.zoomed {
            self.viewport_height
                .saturating_sub(lines.len() + coverage.len() + range_height + 4)
        } else {
            usize::MAX
        };
        let row_limit = if self.zoomed { count } else { 5 };
        let cursor = self.sections[Section::Chats].cursor.min(count - 1);
        let mut first = cursor;
        let mut end = cursor + 1;
        let mut used = rows[cursor].len();
        while first > 0 && end - first < row_limit && used + rows[first - 1].len() <= budget {
            first -= 1;
            used += rows[first].len();
        }
        while end < count && end - first < row_limit && used + rows[end].len() <= budget {
            used += rows[end].len();
            end += 1;
        }
        let mut selection = 0..1;
        for (index, row) in rows.into_iter().enumerate().take(end).skip(first) {
            if index == cursor {
                selection = lines.len()..lines.len() + row.len();
            }
            lines.extend(row);
        }
        lines.push(Line::default());
        lines.extend(wrap(vec![
            format!("Showing {}–{end} of {count} chats", first + 1)
                .set_style(secondary_style())
                .into(),
        ]));
        lines.extend(coverage);
        (lines, selection)
    }
}
