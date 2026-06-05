use super::*;
use codex_app_server_protocol::AccountTokenUsageDailyBucket;
use codex_app_server_protocol::AccountTokenUsageSummary;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

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
      daily · weekly · cumulative
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
       daily · weekly · cumulative
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
       daily · weekly · cumulative
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
       daily · weekly · cumulative
    ");
}

#[test]
fn summary_snapshot_left_aligns_and_splits_when_needed() {
    let response = GetAccountTokenUsageResponse {
        summary: AccountTokenUsageSummary {
            lifetime_tokens: Some(21_400_000_000),
            peak_daily_tokens: Some(835_000_000),
            longest_running_turn_sec: Some(13_920),
            current_streak_days: Some(54),
            longest_streak_days: Some(54),
        },
        daily_usage_buckets: None,
    };
    let rendered = |width| {
        summary_lines(&response, graph_width(width))
            .into_iter()
            .map(|line| line.to_string().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert_snapshot!(
        format!(
            "wide:\n{}\n\nnarrow:\n{}\n\ntight:\n{}",
            rendered(/*width*/ 120),
            rendered(/*width*/ 80),
            rendered(/*width*/ 62)
        ),
        @"
    wide:
     Lifetime 21.4B · Peak 835M · Streak 54d · Longest task 3h 52m

    narrow:
     Lifetime 21.4B · Peak 835M · Streak 54d · Longest task 3h 52m

    tight:
     Lifetime 21.4B · Peak 835M · Streak 54d
     Longest task 3h 52m
    "
    );
}
