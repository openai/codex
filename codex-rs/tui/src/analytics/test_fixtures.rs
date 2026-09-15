//! Bounded fictional dashboard data with raw daily units.
//! Charts and their totals share the same daily series; no account activity is read.

use crate::analytics::sections::Section;
pub(super) const END_DATE: chrono::NaiveDate =
    match chrono::NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 9, /*day*/ 2) {
        Some(date) => date,
        None => panic!("invalid test date"),
    };

pub(super) const TOOL_DAILY: [[u32; 7]; 2] = [[34, 23, 29, 26, 22, 23, 29], [8, 6, 9, 8, 7, 5, 11]];

pub(super) const GROUPS: &[(&str, [&str; 4], [u32; 4])] = &[
    (
        "Surface",
        ["Desktop app", "CLI", "IDE extension", "Web"],
        [35, 36, 17, 12],
    ),
    (
        "Feature",
        ["Tasks", "Code review", "Images", "Other"],
        [65, 15, 12, 8],
    ),
    (
        "Model",
        ["GPT-5.6-Sol", "GPT-5.6-Terra", "GPT-5.5", "Other"],
        [60, 25, 10, 5],
    ),
    (
        "Task start",
        ["User messages", "Goals", "Automations", "Other"],
        [29, 10, 5, 5],
    ),
    (
        "Speed",
        ["Standard", "Fast", "Priority", "Unknown"],
        [65, 30, 5, 0],
    ),
    (
        "Reasoning",
        ["Low", "Medium", "High", "Other"],
        [10, 30, 50, 10],
    ),
    (
        "Token type",
        ["Uncached input", "Cached input", "Output", "Other"],
        [20, 70, 10, 0],
    ),
];

pub(super) fn daily(section: Section, range: usize) -> Vec<u32> {
    let (earlier, recent) = match section {
        Section::Usage => (40, [20, 73, 64, 91, 81, 100, 86]),
        Section::Credits => (35, [0, 41, 57, 85, 75, 109, 64]),
        Section::Activity => (34, [62, 83, 107, 75, 59, 85, 103]),
        _ => return Vec::new(),
    };
    let mut values = vec![earlier; if range == 0 { 0 } else { 23 }];
    values.extend(recent);
    values
}

pub(super) fn breakdown(total: u32, group: usize) -> [u32; 4] {
    let mut values = GROUPS[group].2.map(|weight| total * weight / 100);
    if group != 3 {
        values[3] = total - values[..3].iter().sum::<u32>();
    }
    values
}

/// Fixtures use the private report types and the same presentation path as live reports.
pub(super) fn history(
    report: usize,
    range: usize,
    group: usize,
) -> crate::analytics::models::AccountAnalyticsHistory {
    use crate::analytics::models::AccountAnalyticsDay;
    use crate::analytics::models::AccountAnalyticsHistory;
    use crate::analytics::models::AccountAnalyticsUnit;
    use crate::analytics::models::AccountAnalyticsValue;
    let section = [
        Section::Usage,
        Section::Credits,
        Section::Activity,
        Section::Plugins,
        Section::Skills,
    ][report];
    let series = if report < 3 {
        daily(section, range)
    } else {
        let mut series = vec![if report == 3 { 20 } else { 5 }; if range == 0 { 0 } else { 23 }];
        series.extend(TOOL_DAILY[report - 3]);
        series
    };
    let start = END_DATE - chrono::Days::new(if range == 0 { 6 } else { 29 });
    AccountAnalyticsHistory {
        unit: if report == 0 {
            AccountAnalyticsUnit::RelativeUsage
        } else if report == 1 {
            AccountAnalyticsUnit::Credits
        } else {
            AccountAnalyticsUnit::Count
        },
        updated_at: None,
        data: series
            .into_iter()
            .enumerate()
            .map(|(day, total)| {
                let labels = if report < 3 {
                    GROUPS[group].1
                } else if report == 3 {
                    ["Figma", "Slack", "Notion", "GitHub"]
                } else {
                    ["Code review", "Planning", "Debugging", "Testing"]
                };
                let values = breakdown(total, if report < 3 { group } else { 0 });
                let pairs = if report == 0 && group == 0 {
                    vec![
                        ("Codex", total * 65 / 100),
                        ("Work", total - total * 65 / 100),
                    ]
                } else {
                    labels.into_iter().zip(values).collect()
                };
                AccountAnalyticsDay {
                    date: start + chrono::Days::new(day as u64),
                    total: f64::from(total),
                    values: pairs
                        .into_iter()
                        .map(|(label, value)| AccountAnalyticsValue {
                            key: label.into(),
                            label: label.into(),
                            value: f64::from(value),
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

pub(super) fn tokens(range: usize, grouping: usize) -> super::models::AccountAnalyticsHistory {
    let response = serde_json::json!({"data_freshness_ts":"2026-09-02T16:00:00Z", "data":
        daily(Section::Usage, range).into_iter().enumerate().map(|(index, total)| serde_json::json!({
            "date": (END_DATE - chrono::Days::new(if range == 0 { 6 } else { 29 }) + chrono::Days::new(index as u64)).to_string(),
            "groups": [{"dimensions":{"model":"GPT-5.5"}, "uncached_text_input_tokens": total * 1000, "cached_text_input_tokens":total * 7000, "text_output_tokens":total * 500}]
        })).collect::<Vec<_>>()
    });
    match super::tokens::history(
        match serde_json::from_value(response) {
            Ok(response) => response,
            Err(_) => panic!("invalid test token response"),
        },
        super::data::GROUPINGS[grouping],
        END_DATE - chrono::Days::new(if range == 0 { 6 } else { 29 }),
        END_DATE,
    ) {
        Ok(history) => history,
        Err(_) => panic!("invalid test token history"),
    }
}

/// Build a fixed account state for rendering tests; never opens a connection.
pub(super) fn view(kind: super::models::AccountKind) -> super::AnalyticsView {
    let mut view = super::AnalyticsView::new(crate::keymap::RuntimeKeymap::defaults().list);
    view.end_date = END_DATE;
    view.account = super::data::Load::Ready(kind);
    view.start_reports();
    seed_reports(&mut view);
    view
}

/// Populate explicit report state after a test chooses its account, period, and grouping.
pub(super) fn seed_reports(view: &mut super::AnalyticsView) {
    use super::data::Load;
    for section in view.visible_sections() {
        if let Some(report) = section.report() {
            let range = view.ranges[view.range_group(*section) as usize];
            let group = view.sections[*section].group;
            view.sections[*section].history =
                Load::Ready(if *section == Section::Usage && view.business() {
                    tokens(range, group)
                } else {
                    history(report as usize, range, group)
                });
        }
    }
}
