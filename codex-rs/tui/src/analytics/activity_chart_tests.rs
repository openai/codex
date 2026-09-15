//! Calendar, aggregation, and chart rendering regressions.
use super::*;
use codex_backend_client::TokenUsageProfileDailyBucket as AccountTokenUsageDailyBucket;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

fn graph_width(width: u16) -> u16 {
    if width == u16::MAX {
        return width;
    }
    (CHART_LEFT_WIDTH + shown_columns(width) * 2 - 1) as u16
}

#[test]
fn duplicate_dates_sum_and_negative_values_clamp() {
    let today =
        NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
    let buckets = vec![
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-29".to_string(),
            tokens: 10,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-29".to_string(),
            tokens: 5,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-28".to_string(),
            tokens: -4,
        },
    ];

    let values = daily_values(&buckets, today);

    assert_eq!(values.iter().sum::<i64>(), 15);
}

#[test]
fn bar_levels_fill_from_bottom() {
    let levels = bar_levels(&[0, 10]);

    assert_eq!(&levels[..DAY_COUNT], &[0; DAY_COUNT]);
    assert_eq!(&levels[DAY_COUNT..], &[4; DAY_COUNT]);
}

#[test]
fn token_activity_view_aliases_parse() {
    assert_eq!(TokenActivityView::parse(""), Some(TokenActivityView::Daily));
    assert_eq!(
        TokenActivityView::parse("day"),
        Some(TokenActivityView::Daily)
    );
    assert_eq!(
        TokenActivityView::parse("week"),
        Some(TokenActivityView::Weekly)
    );
    assert_eq!(
        TokenActivityView::parse("cumulative"),
        Some(TokenActivityView::Cumulative)
    );
    assert_eq!(TokenActivityView::parse("year"), None);
}

#[test]
fn daily_graph_snapshot_uses_distinct_empty_and_active_cells() {
    let today =
        NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
    let buckets = vec![
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-25".to_string(),
            tokens: 1,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-29".to_string(),
            tokens: 4,
        },
    ];

    let rendered = chart_lines(TokenActivityView::Daily, &buckets, today, /*width*/ 22)
        .into_iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @r"
         Apr     May
    Su □ □ □ □ □ □ □ □ □
    Mo □ □ □ □ □ □ □ □ ■
    Tu □ □ □ □ □ □ □ □ □
    We □ □ □ □ □ □ □ □ □
    Th □ □ □ □ □ □ □ □ □
    Fr □ □ □ □ □ □ □ □ ■
    Sa □ □ □ □ □ □ □ □

      Less □ ■ ■ ■ ■ More
    ");
}

#[test]
fn daily_graph_snapshot_stays_left_aligned_in_wide_terminal() {
    assert_eq!(graph_width(/*width*/ 160), 107);
    assert_eq!(graph_width(/*width*/ u16::MAX), u16::MAX);

    let today =
        NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
    let lines = chart_lines(TokenActivityView::Daily, &[], today, /*width*/ 160);
    let rendered = [&lines[0], &lines[1], lines.last().expect("legend line")]
        .into_iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @"
       Jun       Jul     Aug       Sep     Oct     Nov       Dec     Jan     Feb     Mar       Apr     May
    Su □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □ □
      Less □ ■ ■ ■ ■ More
    ");
}

#[test]
fn weekly_graph_snapshot_renders_bar_chart_and_caption() {
    let today =
        NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
    let buckets = vec![
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-11".to_string(),
            tokens: 3,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-18".to_string(),
            tokens: 6,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-25".to_string(),
            tokens: 9,
        },
    ];

    let rendered = chart_lines(
        TokenActivityView::Weekly,
        &buckets,
        today,
        /*width*/ 22,
    )
    .into_iter()
    .map(|line| line.to_string().trim_end().to_string())
    .collect::<Vec<_>>()
    .join("\n");

    assert_snapshot!(rendered, @"
          Apr     May
    max                 █
                        █
                      █ █
                      █ █
                    █ █ █
                    █ █ █
      0             █ █ █

       Each column = 1 week · tallest 9
     ");
}

#[test]
fn cumulative_graph_snapshot_renders_running_total_bar_chart_and_caption() {
    let today =
        NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");
    let buckets = vec![
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-11".to_string(),
            tokens: 3,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-18".to_string(),
            tokens: 6,
        },
        AccountTokenUsageDailyBucket {
            start_date: "2026-05-25".to_string(),
            tokens: 9,
        },
    ];

    let rendered = chart_lines(
        TokenActivityView::Cumulative,
        &buckets,
        today,
        /*width*/ 22,
    )
    .into_iter()
    .map(|line| line.to_string().trim_end().to_string())
    .collect::<Vec<_>>()
    .join("\n");

    assert_snapshot!(rendered, @"
          Apr     May
    max                 █
                        █
                        █
                      █ █
                      █ █
                    █ █ █
      0             █ █ █

       Running total · top 18
     ");
}

#[test]
fn year_window_ignores_other_dates_and_saturates_extreme_totals() {
    let today: NaiveDate = "2026-09-09".parse().unwrap();
    let buckets = [
        ("2025-01-01", 100),
        ("2026-09-10", 100),
        ("invalid", 100),
        ("2026-09-08", i64::MAX),
        ("2026-09-08", i64::MAX),
        ("2026-09-09", i64::MAX),
    ]
    .map(|(date, tokens)| AccountTokenUsageDailyBucket {
        start_date: date.into(),
        tokens,
    });
    let values = daily_values(&buckets, today);
    let mut expected = vec![0; CELL_COUNT];
    let index = (today - chart_start(today)).num_days() as usize;
    expected[index - 1] = i64::MAX;
    expected[index] = i64::MAX;
    assert_eq!(values, expected);
    for view in [
        TokenActivityView::Daily,
        TokenActivityView::Weekly,
        TokenActivityView::Cumulative,
    ] {
        let levels = levels_for_view(&values, view);
        assert!(levels.iter().all(|level| *level <= 4));
        assert!(levels.contains(&4));
        assert!(!chart_lines(view, &buckets, today, /*width*/ 110).is_empty());
    }
}
