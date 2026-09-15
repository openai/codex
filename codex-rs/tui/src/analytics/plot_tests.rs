//! Chart scales, signed bands, missing days, narrow viewports, and terminal palettes.

use super::*;
use crate::analytics::models::AccountAnalyticsHistory;
use crate::analytics::render::categories;
use crate::analytics::styles::series_colors;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use pretty_assertions::assert_eq;

fn value(key: &str, value: f64) -> AccountAnalyticsValue {
    AccountAnalyticsValue {
        key: key.into(),
        label: key.into(),
        value,
    }
}

fn display(chart: &Chart) -> String {
    let text = chart
        .lines
        .iter()
        .map(|line| line.to_string().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    let mut palette = Vec::new();
    let mut rows = Vec::new();
    for (row, line) in chart.lines.iter().enumerate() {
        let mut previous = None;
        let mut runs = Vec::new();
        let mut column = 0;
        for span in &line.spans {
            let style = format!("{:?}", span.style);
            let index = palette
                .iter()
                .position(|candidate| *candidate == style)
                .unwrap_or_else(|| {
                    palette.push(style);
                    palette.len() - 1
                });
            if previous != Some(index) {
                runs.push(format!("{column}:{index}"));
                previous = Some(index);
            }
            column += span.width();
        }
        rows.push(format!("{row}: {}", runs.join(" ")));
    }
    let palette = palette
        .iter()
        .enumerate()
        .map(|(index, style)| format!("{index}: {style}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{text}\n\nPalette:\n{palette}\nRows (column:palette):\n{}",
        rows.join("\n")
    )
}

#[test]
fn numeric_axes_use_readable_intervals() {
    use AccountAnalyticsUnit::Count;
    use AccountAnalyticsUnit::Credits;
    use AccountAnalyticsUnit::RelativeUsage;
    use AccountAnalyticsUnit::Tokens;
    let cases = [
        (82.0, Count, 100.0),
        (4_400.0, Credits, 5_000.0),
        (4_999.0, Credits, 5_000.0),
        (5_001.0, Credits, 6_000.0),
        (49.0, Count, 50.0),
        (1.0, Count, 2.0),
        (3.0, Count, 4.0),
        (1_337.0, Credits, 1_500.0),
        (0.07, Credits, 0.08),
        (0.000004, Credits, 0.000004),
        (82.0, RelativeUsage, 100.0),
        (1.0, Tokens, 2.0),
    ];
    assert_eq!(
        cases.map(|(peak, unit, _)| axis_max(peak, unit)),
        cases.map(|(_, _, expected)| expected)
    );
}

#[test]
fn stacked_parts_keep_the_remainder_and_full_denominator() {
    let values = vec![
        value("a", /*value*/ 4.0),
        value("b", /*value*/ 2.0),
        value("c", /*value*/ 1.0),
        value("d", /*value*/ 6.0),
        value("e", /*value*/ -1.0),
    ];
    let categories = vec![values[1].clone(), values[0].clone(), values[2].clone()];
    assert_eq!(parts(&values, &categories), [2.0, 4.0, 1.0, 5.0]);
    let day = AccountAnalyticsDay {
        date: "2026-01-15".parse().unwrap(),
        total: 20.0,
        values,
    };
    let date = day.date;
    assert_eq!(peak(&[(date, Some(&day))], &categories), 20.0);
    let refund = AccountAnalyticsDay {
        total: -1.0,
        values: vec![value("a", /*value*/ 4.0), value("b", /*value*/ -5.0)],
        ..day
    };
    assert_eq!(peak(&[(date, Some(&refund))], &categories), 9.0);
}

#[test]
fn signed_chart_on_light_terminal() {
    signed_chart(
        "light",
        DefaultColors {
            fg: (0, 0, 0),
            bg: (255, 255, 255),
        },
    );
}

#[test]
fn signed_chart_on_dark_terminal() {
    signed_chart(
        "dark",
        DefaultColors {
            fg: (230, 230, 230),
            bg: (16, 16, 16),
        },
    );
}

fn signed_chart(theme: &str, colors: DefaultColors) {
    let history = AccountAnalyticsHistory {
        unit: AccountAnalyticsUnit::Credits,
        updated_at: None,
        data: vec![
            AccountAnalyticsDay {
                date: "2026-01-13".parse().unwrap(),
                total: 8.0,
                values: vec![value("a", /*value*/ 10.0), value("b", /*value*/ -2.0)],
            },
            AccountAnalyticsDay {
                date: "2026-01-14".parse().unwrap(),
                total: -3.0,
                values: vec![value("a", /*value*/ 1.0), value("b", /*value*/ -4.0)],
            },
            AccountAnalyticsDay {
                date: "2026-01-15".parse().unwrap(),
                total: 0.0,
                values: vec![value("a", /*value*/ 3.0), value("b", /*value*/ -3.0)],
            },
        ],
    };
    let days = history
        .data
        .iter()
        .map(|day| (day.date, Some(day)))
        .collect::<Vec<_>>();
    let categories = categories(&history);
    with_test_default_colors(colors, || {
        let rendered = chart(
            &days,
            &categories,
            /*cursor*/ 1,
            /*width*/ 44,
            /*height*/ 4,
            history.unit,
        );
        assert_eq!(rendered.bands, 2);
        insta::assert_snapshot!(format!("signed_chart_{theme}"), display(&rendered));
    });
}

#[test]
fn missing_zero_and_narrow_charts_keep_the_selection_visible() {
    let zero = AccountAnalyticsDay {
        date: "2026-01-14".parse().unwrap(),
        total: 0.0,
        values: vec![value("a", /*value*/ 0.0)],
    };
    let used = AccountAnalyticsDay {
        date: "2026-01-15".parse().unwrap(),
        total: 5.0,
        values: vec![value("a", /*value*/ 5.0)],
    };
    let days = [
        ("2026-01-13".parse().unwrap(), None),
        (zero.date, Some(&zero)),
        (used.date, Some(&used)),
    ];
    with_test_default_colors(
        DefaultColors {
            fg: (230, 230, 230),
            bg: (16, 16, 16),
        },
        || {
            let mut snapshots = Vec::new();
            for (width, cursor) in [(36, 0), (36, 1), (8, 2), (1, 2)] {
                let rendered = chart(
                    &days,
                    &used.values,
                    cursor,
                    width,
                    /*height*/ 3,
                    AccountAnalyticsUnit::Count,
                );
                assert_eq!(rendered.bands, 1);
                assert!(rendered.lines.iter().all(|line| line.width() <= width));
                assert_eq!(
                    rendered
                        .lines
                        .iter()
                        .flat_map(|line| &line.spans)
                        .filter(|span| span.content == "▲")
                        .count(),
                    1
                );
                if cursor == 0 {
                    let dash = rendered
                        .lines
                        .iter()
                        .find_map(|line| line.to_string().find('—'));
                    let cursor = rendered
                        .lines
                        .iter()
                        .find_map(|line| line.to_string().find('▲'));
                    assert_eq!(dash, cursor);
                }
                snapshots.push(format!(
                    "width={width}, cursor={cursor}\n{}",
                    display(&rendered)
                ));
            }
            insta::assert_snapshot!(snapshots.join("\n\n"));
        },
    );
}

#[test]
fn zero_credit_history_uses_a_readable_scale() {
    let history = crate::analytics::normalize::history(
        serde_json::from_str(r#"{"data":[]}"#).unwrap(),
        crate::analytics::models::AccountAnalyticsReport::Credits,
        crate::analytics::models::AccountAnalyticsGrouping::Surface,
        "2026-01-09".parse().unwrap(),
        "2026-01-15".parse().unwrap(),
    )
    .unwrap()
    .unwrap();
    let days = history
        .data
        .iter()
        .map(|day| (day.date, Some(day)))
        .collect::<Vec<_>>();
    let rendered = chart(
        &days,
        &[],
        /*cursor*/ 6,
        /*width*/ 44,
        /*height*/ 4,
        history.unit,
    );
    assert_eq!(axis_max(peak(&days, &[]), history.unit), 1.0);
    insta::assert_snapshot!(display(&rendered));
}

#[test]
fn partial_cells_use_the_midpoint_and_descend_from_the_baseline() {
    let categories = vec![value("a", /*value*/ 0.8), value("b", /*value*/ 0.95)];
    let mut snapshots = Vec::new();
    for sign in [1.0, -1.0] {
        let day = AccountAnalyticsDay {
            date: "2026-01-14".parse().unwrap(),
            total: 1.75 * sign,
            values: categories
                .iter()
                .map(|value| AccountAnalyticsValue {
                    value: value.value * sign,
                    ..value.clone()
                })
                .collect(),
        };
        let days = [
            (day.date, Some(&day)),
            ("2026-01-15".parse().unwrap(), None),
        ];
        let rendered = chart(
            &days,
            &categories,
            /*cursor*/ 1,
            /*width*/ 12,
            /*height*/ 1,
            AccountAnalyticsUnit::Credits,
        );
        let cell = &rendered.lines[if sign > 0.0 { 1 } else { 3 }].spans[1];
        assert_eq!(cell.style.fg, Some(series_colors()[1]));
        assert_eq!(cell.content.as_ref(), if sign > 0.0 { "▇" } else { "▁" });
        assert_eq!(
            cell.style
                .add_modifier
                .contains(ratatui::style::Modifier::REVERSED),
            sign < 0.0
        );
        snapshots.push(format!("sign={sign}\n{}", display(&rendered)));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn oversized_axes_are_hidden_instead_of_truncated() {
    let day = AccountAnalyticsDay {
        date: "2026-01-15".parse().unwrap(),
        total: 100_000_000.0,
        values: vec![value("a", /*value*/ 100_000_000.0)],
    };
    let days = [(day.date, Some(&day))];
    let mut snapshots = Vec::new();
    for width in [8, 30, 35, 44] {
        let rendered = chart(
            &days,
            &day.values,
            /*cursor*/ 0,
            width,
            /*height*/ 4,
            AccountAnalyticsUnit::Tokens,
        );
        let baseline = rendered.lines[5].to_string();
        assert_eq!(baseline.starts_with('─'), width < 36);
        if width == 8 {
            assert!(rendered.lines[0].to_string().trim().is_empty());
        }
        snapshots.push(format!("width={width}\n{}", display(&rendered)));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn selected_callout_keeps_the_authoritative_daily_total() {
    let day = AccountAnalyticsDay {
        date: "2026-01-15".parse().unwrap(),
        total: 20.0,
        values: vec![value("a", /*value*/ 8.0), value("b", /*value*/ 4.0)],
    };
    let days = [(day.date, Some(&day))];
    let rendered = chart(
        &days,
        &day.values,
        /*cursor*/ 0,
        /*width*/ 44,
        /*height*/ 4,
        AccountAnalyticsUnit::Credits,
    );
    assert!(display(&rendered).contains("20.00"));
    assert!(!display(&rendered).contains("12.00"));
    insta::assert_snapshot!(display(&rendered));
}

#[test]
fn narrow_long_range_charts_never_truncate_dates() {
    let start: NaiveDate = "2026-01-01".parse().unwrap();
    let history = start
        .iter_days()
        .take(/*n*/ 30)
        .map(|date| AccountAnalyticsDay {
            date,
            total: 1.0,
            values: vec![value("a", /*value*/ 1.0)],
        })
        .collect::<Vec<_>>();
    let days = history
        .iter()
        .map(|day| (day.date, Some(day)))
        .collect::<Vec<_>>();
    let mut snapshots = Vec::new();
    for (width, expected) in [(1, ""), (2, "15"), (5, "15"), (6, "Jan 15")] {
        let rendered = chart(
            &days,
            &history[0].values,
            /*cursor*/ 14,
            width,
            /*height*/ 3,
            AccountAnalyticsUnit::Count,
        );
        assert_eq!(rendered.lines.last().unwrap().to_string().trim(), expected);
        snapshots.push(format!("width={width}\n{}", display(&rendered)));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn tiny_segments_keep_their_series_color_beside_a_large_day() {
    let large = AccountAnalyticsDay {
        date: "2026-01-14".parse().unwrap(),
        total: 100.0,
        values: vec![value("a", /*value*/ 100.0)],
    };
    let mut snapshots = Vec::new();
    for sign in [1.0, -1.0] {
        let tiny = AccountAnalyticsDay {
            date: "2026-01-15".parse().unwrap(),
            total: 0.01 * sign,
            values: vec![value("a", 0.01 * sign)],
        };
        let days = [(large.date, Some(&large)), (tiny.date, Some(&tiny))];
        let rendered = chart(
            &days,
            &large.values,
            /*cursor*/ 0,
            /*width*/ 44,
            /*height*/ 4,
            AccountAnalyticsUnit::Credits,
        );
        let glyph = if sign > 0.0 { "▁" } else { "▇" };
        let cells = rendered
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.content == glyph)
            .collect::<Vec<_>>();
        assert!(!cells.is_empty());
        assert!(
            cells
                .iter()
                .all(|cell| cell.style.fg == Some(series_colors()[0]))
        );
        snapshots.push(format!("sign={sign}\n{}", display(&rendered)));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn duplicate_message_dates_render_the_full_other_remainder() {
    use crate::analytics::models::AccountAnalyticsGrouping;
    use crate::analytics::models::AccountAnalyticsReport;
    let date = "2026-01-15".parse().unwrap();
    let response = serde_json::from_value(serde_json::json!({"data": [
        {"date": "2026-01-15", "totals": {"turns": 10}, "clients": [{"client_id": "CODEX_CLI", "turns": 8}]},
        {"date": "2026-01-15", "totals": {"turns": 10}, "clients": [{"client_id": "CODEX_CLI", "turns": 8}]}
    ]})).unwrap();
    let history = crate::analytics::normalize::history(
        response,
        AccountAnalyticsReport::Messages,
        AccountAnalyticsGrouping::Surface,
        date,
        date,
    )
    .unwrap()
    .unwrap();
    let day = &history.data[0];
    let rendered = chart(
        &[(date, Some(day))],
        &day.values,
        /*cursor*/ 0,
        /*width*/ 44,
        /*height*/ 10,
        AccountAnalyticsUnit::Count,
    );
    insta::assert_snapshot!(display(&rendered));
}
