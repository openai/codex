//! Completion metadata rendering, date selection, and muted styling coverage.

use super::*;
use chrono::NaiveDateTime;
use chrono::TimeZone;
use chrono::Timelike;
use codex_otel::RuntimeMetricTotals;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn completed_at() -> DateTime<Local> {
    let datetime = NaiveDateTime::parse_from_str("2000-09-06 14:32:00", "%Y-%m-%d %H:%M:%S")
        .expect("valid completion time");
    Local
        .from_local_datetime(&datetime)
        .single()
        .expect("unambiguous completion time")
}

#[test]
fn completion_label_shows_duration_only_above_sixty_seconds() {
    let completed_at = completed_at();
    let labels = [
        None,
        Some(0),
        Some(12),
        Some(60),
        Some(61),
        Some(125),
        Some(3_605),
    ]
    .into_iter()
    .map(|elapsed_seconds| {
        FinalMessageSeparator::new(elapsed_seconds, /*runtime_metrics*/ None)
            .with_completed_at(completed_at)
            .label(completed_at.date_naive())
            .expect("completion label")
    })
    .collect::<Vec<_>>();

    insta::assert_snapshot!(labels.join("\n"), @"
    2:32 PM
    2:32 PM
    2:32 PM
    2:32 PM
    Worked for 1m 1s · 2:32 PM
    Worked for 2m 5s · 2:32 PM
    Worked for 1h 0m 5s · 2:32 PM
    ");
}

#[test]
fn completion_label_includes_date_when_viewed_on_another_day() {
    let completed_at = completed_at();
    let cell = FinalMessageSeparator::new(
        /*elapsed_seconds*/ Some(125),
        /*runtime_metrics*/ None,
    )
    .with_completed_at(completed_at);
    let tomorrow = completed_at.date_naive().succ_opt().expect("next day");

    insta::assert_snapshot!(cell.label(tomorrow).expect("completion label"), @"Worked for 2m 5s · Sep 6 at 2:32 PM");
    let next_year = tomorrow.with_year(/*year*/ 2001).expect("valid next year");
    insta::assert_snapshot!(cell.label(next_year).expect("completion label"), @"Worked for 2m 5s · Sep 6, 2000 at 2:32 PM");
}

#[test]
fn completion_uses_twelve_hour_time_at_midnight_noon_and_afternoon() {
    let labels = [0, 12, 15].map(|hour| {
        let completed_at = completed_at().with_hour(hour).expect("valid hour");
        FinalMessageSeparator::new(/*elapsed_seconds*/ None, /*runtime_metrics*/ None)
            .with_completed_at(completed_at)
            .label(completed_at.date_naive())
            .expect("completion label")
    });
    insta::assert_snapshot!(labels.join("\n"), @"
    12:32 AM
    12:32 PM
    3:32 PM
    ");
}

#[test]
fn completion_without_metadata_has_no_visible_or_raw_lines() {
    let cell =
        FinalMessageSeparator::new(/*elapsed_seconds*/ None, /*runtime_metrics*/ None);
    assert_eq!(cell.display_lines(/*width*/ 80), Vec::<Line>::new());
    assert_eq!(cell.raw_lines(), Vec::<Line>::new());
}

#[test]
fn completion_without_timestamp_retains_known_elapsed_duration() {
    let cell = FinalMessageSeparator::new(
        /*elapsed_seconds*/ Some(125),
        /*runtime_metrics*/ None,
    );
    insta::assert_snapshot!(cell.raw_lines()[0].to_string(), @"Worked for 2m 5s");
}

#[test]
fn completion_wraps_metadata_and_preserves_unwrapped_raw_text() {
    let cell = FinalMessageSeparator::new(
        /*elapsed_seconds*/ Some(125),
        /*runtime_metrics*/ None,
    )
    .with_completed_at(completed_at())
    .with_runtime_metrics(Some(RuntimeMetricsSummary {
        tool_calls: RuntimeMetricTotals {
            count: 3,
            duration_ms: 2_450,
        },
        ..RuntimeMetricsSummary::default()
    }));
    let lines = cell.display_lines(/*width*/ 24);
    let rendered = lines.iter().map(ToString::to_string).collect::<Vec<_>>();

    insta::assert_snapshot!(rendered.join("\n"), @r"
    Worked for 2m 5s · Sep
    6, 2000 at 2:32 PM ·
    Local tools: 3 calls
    (2.5s)
    ");
    insta::assert_snapshot!(cell.raw_lines()[0].to_string(), @"Worked for 2m 5s · Sep 6, 2000 at 2:32 PM · Local tools: 3 calls (2.5s)");
    for width in [0, 1, 5, 24] {
        assert!(
            cell.display_lines(width)
                .iter()
                .all(|line| line.width() <= usize::from(width))
        );
    }
}

#[test]
fn completion_renders_with_dim_default_colors() {
    let cell: Box<dyn HistoryCell> = Box::new(
        FinalMessageSeparator::new(/*elapsed_seconds*/ None, /*runtime_metrics*/ None)
            .with_completed_at(completed_at()),
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    cell.render(area, &mut buffer);
    let text_width = cell.display_lines(area.width)[0].width();
    let styles = (0..text_width)
        .map(|x| {
            let rendered = &buffer[(x as u16, 0)];
            (rendered.fg, rendered.bg, rendered.modifier)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        styles,
        vec![(Color::Reset, Color::Reset, Modifier::DIM); text_width]
    );
}

#[test]
fn completion_rendering_uses_the_captured_display_date() {
    let completed_at = completed_at();
    let mut cell =
        FinalMessageSeparator::new(/*elapsed_seconds*/ None, /*runtime_metrics*/ None)
            .with_completed_at(completed_at);
    cell.display_date = completed_at.date_naive();

    insta::assert_snapshot!(cell.display_lines(/*width*/ 80)[0].to_string(), @"  2:32 PM");
    assert_eq!(cell.raw_lines(), vec![Line::from("2:32 PM")]);
}
