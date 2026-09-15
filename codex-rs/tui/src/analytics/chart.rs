//! Daily history with a stable chart and a compact, separated breakdown table.
//! Chart series rank by the full range; table rows rank by the selected day and retain series colors.
//! Missing and zero days retain the same row count; only explicit expansion changes height.

use super::AnalyticsView;
use super::data;
use super::plot;
use super::render::categories;
use super::render::columns;
use super::styles::number;
use super::styles::secondary_style;
use super::styles::series_colors;
use crate::analytics::models::AccountAnalyticsUnit;
use crate::analytics::models::AccountAnalyticsValue;
use crate::analytics::sections::Section;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;

impl AnalyticsView {
    pub(super) fn day_has_details(&self, section: Section, day_index: usize) -> bool {
        let Some(history) = self.sections[section].history.ready() else {
            return false;
        };
        let date = self
            .section_date_range(section)
            .start()
            .iter_days()
            .nth(day_index);
        history.data.iter().any(|day| {
            date.is_some_and(|date| day.date == date)
                && day.values.iter().any(|value| value.value != 0.0)
        })
    }

    pub(super) fn history_lines(
        &self,
        section: Section,
        width: usize,
        height: usize,
    ) -> plot::Chart {
        let state = &self.sections[section].history;
        let Some(history) = state.ready() else {
            return plot::Chart {
                lines: state
                    .message()
                    .into_iter()
                    .map(|message| message.to_string().set_style(secondary_style()).into())
                    .collect(),
                bands: 0,
            };
        };
        if history.data.is_empty() {
            return plot::Chart {
                lines: vec![
                    "No data reported for this range."
                        .set_style(secondary_style())
                        .into(),
                ],
                bands: 0,
            };
        }
        let range = self.section_date_range(section);
        // At most 30 daily slots; index by date if report ranges grow.
        let days = range
            .start()
            .iter_days()
            .take_while(|date| date <= range.end())
            .map(|date| {
                let day = history.data.iter().find(|day| day.date == date);
                (date, day)
            })
            .collect::<Vec<_>>();
        let cursor = self.sections[section].cursor.min(days.len() - 1);
        let selected_date = days[cursor].0.format("%b %-d").to_string();
        let day = days[cursor].1;
        let expanded = self.zoomed && self.sections[section].detail.is_some();
        let colors = series_colors();
        let mut categories = categories(history);
        if matches!(section, Section::Plugins | Section::Skills) {
            for category in &mut categories {
                category.label = super::tool_panel::tool_name(&category.label);
            }
        }
        if self.sections[section].group == 2 {
            for category in &mut categories {
                category.label = self.model_name(&category.label).to_string();
            }
        }
        let unit = match section {
            Section::Usage if history.unit == AccountAnalyticsUnit::Tokens => "tokens",
            Section::Summary => "tokens",
            Section::Activity => "messages",
            Section::Plugins => "calls",
            Section::Skills => "uses",
            Section::Usage | Section::Credits | Section::Chats | Section::Plan => "credits",
        };
        let empty_breakdown = matches!(
            section,
            Section::Plugins | Section::Activity | Section::Skills
        ) && day.is_none_or(|day| {
            day.total == 0.0 && day.values.iter().all(|value| value.value == 0.0)
        });
        let relative = history.unit == AccountAnalyticsUnit::RelativeUsage;
        let format_amount = if history.unit == AccountAnalyticsUnit::Credits {
            data::credit_amount
        } else {
            data::amount
        };
        let total: f64 = history.data.iter().map(|day| day.total).sum();
        let formatted_total = if total.abs() >= 1_000.0 {
            data::compact_amount(total)
        } else {
            format_amount(total)
        };
        let mut lines = vec![if relative {
            "Total usage · relative units"
                .set_style(secondary_style())
                .into()
        } else {
            number(format!(
                "{} {}{unit}",
                formatted_total,
                if section == Section::Activity {
                    "reported "
                } else {
                    ""
                }
            ))
            .into()
        }];
        if section == Section::Usage && !self.business() && self.zoomed {
            lines.push(
                "Approximate · may be delayed up to 6 hours"
                    .set_style(secondary_style())
                    .into(),
            );
        }
        if self.sections[section].group == 3 {
            lines.push(
                "All usage by how each turn started"
                    .set_style(secondary_style())
                    .into(),
            );
        }
        let has_activity = history
            .data
            .iter()
            .any(|day| day.total != 0.0 || day.values.iter().any(|value| value.value != 0.0));
        let mut bands = 0;
        if has_activity {
            let chart = plot::chart(&days, &categories, cursor, width, height, history.unit);
            bands = chart.bands;
            lines.extend(chart.lines);
        } else {
            lines.push(
                if matches!(
                    section,
                    Section::Plugins | Section::Activity | Section::Skills
                ) {
                    format!("No {unit} reported in this range.")
                        .set_style(secondary_style())
                        .into()
                } else {
                    "No activity reported in this range."
                        .set_style(secondary_style())
                        .into()
                },
            );
        }

        let table_width = width.min(/*other*/ 56);
        lines.push(Line::default());
        if self.zoomed {
            lines.push("─".repeat(table_width).dim().into());
        }
        let table_start = lines.len();
        let summary = match day {
            None => " · Not reported".set_style(secondary_style()),
            Some(day) if day.total == 0.0 && day.values.iter().all(|value| value.value == 0.0) => {
                " · No activity (0)".set_style(secondary_style())
            }
            Some(day) if relative => number(format!(" · {} usage units", data::amount(day.total))),
            Some(day) => number(format!(
                " · {} {unit}",
                if expanded {
                    day.total.to_string()
                } else {
                    format_amount(day.total)
                }
            )),
        };
        lines.push(truncate_line_with_ellipsis_if_overflow(
            vec![selected_date.clone().bold(), summary].into(),
            width,
        ));
        if self.zoomed {
            lines.push(columns(
                match section {
                    Section::Plugins => "Plugin",
                    Section::Skills => "Skill",
                    Section::Chats => "Chat",
                    Section::Summary
                    | Section::Usage
                    | Section::Credits
                    | Section::Activity
                    | Section::Plan => self.group_label(section, self.sections[section].group),
                }
                .bold()
                .into(),
                if history.unit == AccountAnalyticsUnit::Tokens {
                    "Tokens"
                } else if section == Section::Usage {
                    "Share"
                } else if section == Section::Activity {
                    "Messages"
                } else if section == Section::Plugins {
                    "Calls"
                } else if section == Section::Skills {
                    "Uses"
                } else {
                    "Credits"
                }
                .bold()
                .into(),
                table_width,
            ));
        }
        let mut values = categories
            .iter()
            .enumerate()
            .map(|(index, category)| {
                (
                    index,
                    AccountAnalyticsValue {
                        value: day
                            .and_then(|day| {
                                day.values.iter().find(|value| value.key == category.key)
                            })
                            .map_or(/*default*/ 0.0, |value| value.value),
                        ..category.clone()
                    },
                )
            })
            .collect::<Vec<_>>();
        values.sort_by(|(_, a), (_, b)| b.value.abs().total_cmp(&a.value.abs()));
        let remainder = if !expanded && values.len() > 4 {
            let remaining = values.split_off(/*at*/ 3);
            Some((
                format!("{} more", remaining.len()),
                remaining.iter().map(|(_, value)| value.value).sum::<f64>(),
            ))
        } else {
            None
        };
        let rows = values
            .iter()
            .map(|(index, value)| {
                (
                    Line::from(vec![
                        "● ".fg(colors[(*index).min(/*other*/ 3)]),
                        value.label.clone().into(),
                    ]),
                    value.value,
                )
            })
            .chain(
                remainder.map(|(label, value)| (Line::from(vec!["… ".dim(), label.dim()]), value)),
            );
        for (label, value) in rows.take(if self.zoomed { usize::MAX } else { 2 }) {
            let amount = match day {
                None => "—".into(),
                Some(day)
                    if section == Section::Usage
                        && history.unit != AccountAnalyticsUnit::Tokens =>
                {
                    format!(
                        "{:.1}%",
                        if day.total == 0.0 {
                            0.0
                        } else {
                            value / day.total * 100.0
                        }
                    )
                }
                Some(_) if expanded => value.to_string(),
                Some(_) => format_amount(value),
            };
            lines.push(columns(
                truncate_line_with_ellipsis_if_overflow(
                    label,
                    table_width.saturating_sub(amount.len() + 2),
                ),
                if day.is_none() || value == 0.0 {
                    amount.dim()
                } else {
                    amount.into()
                }
                .into(),
                table_width,
            ));
        }
        if empty_breakdown {
            // Preserve the measured table allocation when the selected day has no breakdown.
            let table_end = lines.len();
            lines.truncate(table_start);
            lines.push(truncate_line_with_ellipsis_if_overflow(
                if day.is_none() {
                    format!("Data not reported for {selected_date}.")
                } else {
                    format!("No {unit} reported for {selected_date}.")
                }
                .set_style(secondary_style())
                .into(),
                width,
            ));
            lines.resize(table_end, Line::default());
        }
        if let Some(updated) = history
            .updated_at
            .and_then(|value| chrono::DateTime::from_timestamp(value, /*nsecs*/ 0))
        {
            lines.push(
                format!("Updated {} UTC", updated.format("%b %-d %H:%M"))
                    .set_style(secondary_style())
                    .into(),
            );
        }
        plot::Chart { lines, bands }
    }
}
