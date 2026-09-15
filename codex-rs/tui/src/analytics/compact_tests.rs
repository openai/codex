//! Large report totals and axes stay compact while selected-day details retain precision.
use super::*;

#[test]
fn report_headlines_and_axes_compact_large_values() {
    let mut screenshots = Vec::new();
    for (section, unit, total, headline) in [
        (
            Section::Usage,
            models::AccountAnalyticsUnit::Tokens,
            12_280_365_226.0,
            "12.3B tokens",
        ),
        (
            Section::Plugins,
            models::AccountAnalyticsUnit::Count,
            9_749.0,
            "9.7K calls",
        ),
        (
            Section::Skills,
            models::AccountAnalyticsUnit::Count,
            2_901.0,
            "2.9K uses",
        ),
    ] {
        let mut view = fixture::view(models::AccountKind::Enterprise);
        view.section = section;
        view.sections[section].history = Load::Ready(models::AccountAnalyticsHistory {
            unit,
            updated_at: None,
            data: vec![models::AccountAnalyticsDay {
                date: view.end_date,
                total,
                values: vec![models::AccountAnalyticsValue {
                    key: "test".into(),
                    label: "Example".into(),
                    value: total,
                }],
            }],
        });
        let rendered = screen(&mut view, /*width*/ 100, /*height*/ 30);
        assert!(rendered.contains(headline));
        assert!(rendered.contains(&data::amount(total)));
        screenshots.push(rendered);
    }
    insta::assert_snapshot!(screenshots.join("\n"));
}
