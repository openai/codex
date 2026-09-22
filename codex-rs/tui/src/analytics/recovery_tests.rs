//! Restored dashboard, visible controls, and responsive report regression coverage.

use super::*;

#[test]
fn report_controls_show_dates_and_update_time() {
    let mut view = fixture::view(models::AccountKind::Business);
    view.section = Section::Usage;
    view.sections[Section::Usage].group = 6;
    fixture::seed_reports(&mut view);
    view.end_date = "2027-01-02".parse().unwrap();
    let output = screen(&mut view, /*width*/ 40, /*height*/ 16);
    assert!(output.contains("12/27/2026–01/02/2027"));
    assert!(output.contains("g Token type · m All models"));
    assert!(output.contains("Updated · Sep 2 16:00 UTC"));
}

#[test]
fn twelve_hour_metadata_keeps_utc_across_renderers() {
    let mut view = fixture::view(models::AccountKind::Business);
    view.clock_format = crate::clock_format::ClockFormat::TwelveHour;
    view.section = Section::Usage;
    let controls = screen(&mut view, /*width*/ 40, /*height*/ 16);
    view.zoomed = false;
    view.tasks = Load::Ready(tasks::Chats {
        rows: vec![tasks::Chat {
            title: "Clock fixture".into(),
            task: None,
        }],
        updated_at: Some("2026-09-02T16:00:00Z".parse().unwrap()),
        ..tasks::Chats::default()
    });
    let mut output = controls
        .lines()
        .filter(|line| line.contains("Updated"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for lines in [
        view.history_lines(Section::Usage, /*width*/ 40, /*height*/ 16)
            .lines,
        view.help_lines(/*width*/ 40),
        view.task_lines(/*width*/ 80).0,
    ] {
        output.extend(
            lines
                .iter()
                .map(ToString::to_string)
                .filter(|line| line.contains("Updated") || line.contains("Report updated")),
        );
    }
    insta::assert_snapshot!(output.join("\n"));
}

#[test]
fn tall_reports_use_available_height_and_wide_reports_put_details_beside_plot() {
    for (width, height) in [(80, 60), (180, 60)] {
        let mut view = fixture::view(models::AccountKind::Business);
        view.section = Section::Usage;
        let output = screen(&mut view, width, height);
        let plot_rows = output.lines().filter(|line| line.contains('█')).count();
        assert!(
            plot_rows > 14,
            "only {plot_rows} plot rows at {width} columns"
        );
        assert!(output.contains("731,000 tokens"));
        assert!(!output.contains("scroll"));
        if width == 180 {
            assert!(
                output
                    .lines()
                    .any(|line| line.contains("4.4M tokens") && line.contains('─'))
            );
            insta::assert_snapshot!(output);
        }
    }
}
