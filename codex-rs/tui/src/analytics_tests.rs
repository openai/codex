//! Dashboard layout, local detail, shared ranges, grouping, and scroll/focus retention.

use crate::analytics::sections::Section;
#[path = "analytics/compact_tests.rs"]
mod compact;
#[path = "analytics/dashboard_tests.rs"]
mod dashboard;
#[path = "analytics/summary_tests.rs"]
mod summary;

#[path = "analytics/account_layout_tests.rs"]
mod account_layout;

#[path = "analytics/navigation_tests.rs"]
mod navigation;

#[path = "analytics/consumer_refresh_tests.rs"]
mod consumer_refresh;

#[path = "analytics/tool_panel_tests.rs"]
mod tools_panel;

#[path = "analytics/styles_tests.rs"]
mod terminal_styles;

use super::*;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn press(view: &mut AnalyticsView, code: KeyCode) {
    view.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn screen(view: &mut AnalyticsView, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    terminal.backend().to_string()
}

#[test]
fn analytics_exact_fit_content_has_no_scroll_hint() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    view.section = Section::Chats;
    screen(&mut view, /*width*/ 120, /*height*/ 60);
    let chrome_height = 60 - view.viewport_height;
    let content_height = view
        .panel(Section::Chats, /*width*/ 116, /*chart_height*/ 4)
        .lines
        .len();
    let exact_height = (chrome_height + content_height) as u16;

    let exact = screen(&mut view, /*width*/ 120, exact_height);
    assert!(!exact.contains("scroll"));
    assert_eq!(view.viewport_height, content_height);

    insta::assert_snapshot!(exact);
}

#[test]
fn analytics_overview() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('z'));
    let wide = screen(&mut view, /*width*/ 120, /*height*/ 60);
    for section in view.visible_sections() {
        assert!(wide.contains(view.section_title(*section)));
    }
    insta::assert_snapshot!(wide);
}

#[test]
fn analytics_narrow_charts_and_small_terminals_keep_valid_cursors() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('z'));
    for section in ['2', '3', '4'] {
        press(&mut view, KeyCode::Char(section));
        press(&mut view, KeyCode::Char('r'));
        fixture::seed_reports(&mut view);
        press(&mut view, KeyCode::Home);
        for _ in 0..30 {
            screen(&mut view, /*width*/ 28, /*height*/ 12);
            press(&mut view, KeyCode::Right);
        }
        assert_eq!(view.sections[view.section].cursor, 29);
    }
    for _ in 0..2 {
        press(&mut view, KeyCode::Char('z'));
        for width in [1, 4, 32, 107, 108] {
            screen(&mut view, width, /*height*/ 1);
        }
    }
    let empty = ratatui::layout::Rect::default();
    view.render(empty, &mut ratatui::buffer::Buffer::empty(empty));
    press(&mut view, KeyCode::Char('5'));
    let tools = screen(&mut view, /*width*/ 58, /*height*/ 20);
    insta::assert_snapshot!(tools);
}

#[tokio::test]
async fn analytics_credits_grouping_preserves_selected_day_and_total() {
    let server = test_support::server().await;
    let (_home, _app_server, mut view) = client::tests::connected_view(&server, "business").await;
    test_support::settle(&mut view).await;
    press(&mut view, KeyCode::Char('2'));
    press(&mut view, KeyCode::Left);
    let before = view.sections[Section::Credits]
        .history
        .ready()
        .unwrap()
        .data
        .iter()
        .map(|day| day.total)
        .collect::<Vec<_>>();
    for group in [2, 4, 5, 0] {
        press(&mut view, KeyCode::Char('g'));
        test_support::settle(&mut view).await;
        assert_eq!(
            (
                view.sections[Section::Credits].group,
                view.sections[Section::Credits].cursor
            ),
            (group, 5)
        );
        assert_eq!(
            view.sections[Section::Credits]
                .history
                .ready()
                .unwrap()
                .data
                .iter()
                .map(|day| day.total)
                .collect::<Vec<_>>(),
            before
        );
    }
    view.end_date = fixture::END_DATE;
    fixture::seed_reports(&mut view);
    insta::assert_snapshot!(screen(&mut view, /*width*/ 85, /*height*/ 40));
}

#[tokio::test]
async fn analytics_loading_errors_and_empty_reports_stay_independent() {
    let mut view = AnalyticsView::new(RuntimeKeymap::defaults().list);
    let (send, receive) = tokio::sync::oneshot::channel();
    view.sections[Section::Usage].history = Load::start(
        async move { receive.await.unwrap() },
        FrameRequester::test_dummy(),
    );
    view.sections[Section::Credits].history =
        Load::Error("Couldn't load credits. Press R to retry.".into());
    let mut empty = fixture::history(/*report*/ 2, /*range*/ 0, /*group*/ 2);
    empty.data.clear();
    view.sections[Section::Activity].history = Load::Ready(empty);
    let panels = [
        Section::Usage,
        Section::Plugins,
        Section::Credits,
        Section::Activity,
    ]
    .into_iter()
    .map(|section| {
        view.panel(section, /*width*/ 54, /*chart_height*/ 2)
            .lines
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })
    .collect::<Vec<_>>()
    .join("\n\n");
    insta::assert_snapshot!(panels);
    // Refresh drops pending work before a stale result can populate the retained view.
    view.refresh();
    tokio::task::yield_now().await;
    assert!(send.is_closed());
    assert!(matches!(
        view.sections[Section::Usage].history,
        Load::Unavailable
    ));
}

#[test]
fn analytics_preserves_full_denominators_and_signed_small_credits() {
    use crate::analytics::models::AccountAnalyticsDay;
    use crate::analytics::models::AccountAnalyticsValue;
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('z'));
    let mut history = fixture::history(/*report*/ 0, /*range*/ 0, /*group*/ 3);
    history.data = vec![AccountAnalyticsDay {
        date: "2026-09-02".parse().unwrap(),
        total: 100.0,
        values: vec![AccountAnalyticsValue {
            key: "goal".into(),
            label: "Goals".into(),
            value: 30.0,
        }],
    }];
    view.sections[Section::Usage].history = Load::Ready(history.clone());
    view.sections[Section::Usage].group = 3;
    view.sections[Section::Usage].cursor = 6;
    let task = view
        .panel(Section::Usage, /*width*/ 54, /*chart_height*/ 2)
        .lines;
    assert!(
        task.iter()
            .any(|line| line.to_string().contains("Goals") && line.to_string().contains("30.0%"))
    );
    assert!(
        task.iter()
            .any(|line| line.to_string() == "Sep 2 · 100 usage units")
    );
    history.unit = crate::analytics::models::AccountAnalyticsUnit::Credits;
    history.data[0].total = -0.000004;
    history.data[0].values[0].value = -0.000004;
    history.data[0].values[0].label = "CLI".into();
    view.sections[Section::Credits].history = Load::Ready(history);
    view.sections[Section::Credits].cursor = 6;
    let credits = view
        .panel(Section::Credits, /*width*/ 54, /*chart_height*/ 2)
        .lines;
    insta::assert_snapshot!(
        task.into_iter()
            .chain(credits)
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn analytics_credit_display_retains_integer_precision() {
    assert_eq!(
        [753.71, 411.12, 47.4, 0.0, 1_212.224, -0.004, -0.000004].map(data::credit_amount),
        [
            "753.71",
            "411.12",
            "47.40",
            "0.00",
            "1,212.22",
            "-0.004",
            "-0.000004"
        ]
        .map(str::to_string)
    );
    assert_eq!(
        [0, -4, 999_999_999, 19_350_041_555, i64::MIN, i64::MAX].map(data::credits),
        [
            "0.00",
            "-0.000004",
            "1,000.00",
            "19,350.04",
            "-9,223,372,036,854.78",
            "9,223,372,036,854.78"
        ]
        .map(str::to_string)
    );
    assert_eq!(
        [
            12_746.441206,
            -1_000.125,
            999.999,
            0.0,
            -0.000004,
            0.009,
            7.5
        ]
        .map(data::amount),
        [
            "12,746.44",
            "-1,000.12",
            "1,000",
            "0",
            "-0.000004",
            "0.009",
            "7.5"
        ]
        .map(str::to_string)
    );
}

#[test]
fn analytics_dates_distinguish_zero_missing_and_exact_details() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let mut history = fixture::history(/*report*/ 1, /*range*/ 0, /*group*/ 0);
    history.data[1].total = 12_746.441206;
    history.data[1].values[0].value = 12_746.441206;
    history.data[1].values.truncate(/*len*/ 1);
    history.data.remove(/*index*/ 2);
    view.sections[Section::Credits].history = Load::Ready(history.clone());
    press(&mut view, KeyCode::Char('2'));
    press(&mut view, KeyCode::Home);
    let mut screens = Vec::new();
    for keys in [
        vec![],
        vec![KeyCode::Right, KeyCode::Enter],
        vec![KeyCode::Right],
    ] {
        for key in keys {
            press(&mut view, key);
        }
        screens.push(
            view.panel(Section::Credits, /*width*/ 54, /*chart_height*/ 3)
                .lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    assert!(screens[0].contains("No activity (0)"));
    assert!(screens[1].contains("12746.441206 credits"));
    assert!(screens[2].contains("Aug 29 · Not reported"));
    for day in &mut history.data {
        day.total = 0.0;
        day.values.clear();
    }
    view.sections[Section::Credits].history = Load::Ready(history.clone());
    screens.push(
        view.panel(Section::Credits, /*width*/ 54, /*chart_height*/ 3)
            .lines
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    history.data.clear();
    view.sections[Section::Credits].history = Load::Ready(history);
    screens.push(
        view.panel(Section::Credits, /*width*/ 54, /*chart_height*/ 3)
            .lines
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    insta::assert_snapshot!(screens.join("\n\n"));
}

#[test]
fn analytics_model_labels_and_tool_remainders_are_unambiguous() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('z'));
    view.model_names
        .insert("GPT-5.6-Sol".into(), "Friendly model".into());
    let mut tools = fixture::history(/*report*/ 4, /*range*/ 0, /*group*/ 0);
    for day in &mut tools.data {
        day.values[1].label = "Other".into();
    }
    view.sections[Section::Skills].history = Load::Ready(tools);
    press(&mut view, KeyCode::Char('3'));
    let overview = screen(&mut view, /*width*/ 140, /*height*/ 64);
    assert!(overview.contains("Friendly model"));
    assert!(overview.contains("Skills used"));
    press(&mut view, KeyCode::Char('5'));
    press(&mut view, KeyCode::Enter);
    let skills = screen(&mut view, /*width*/ 100, /*height*/ 32);
    assert!(skills.contains("Other"));
    insta::assert_snapshot!(format!("{overview}\n{skills}"));
}

#[test]
fn analytics_maximized_refunds_leave_room_for_both_bands() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let mut history = fixture::history(/*report*/ 1, /*range*/ 0, /*group*/ 0);
    history.data[6].total = -0.000004;
    history.data[6].values = vec![crate::analytics::models::AccountAnalyticsValue {
        key: "CLI".into(),
        label: "CLI".into(),
        value: -0.000004,
    }];
    view.sections[Section::Credits].history = Load::Ready(history);
    press(&mut view, KeyCode::Char('2'));
    let output = screen(&mut view, /*width*/ 100, /*height*/ 32);
    assert!(output.contains("Sep 2 · -0.000004 credits"));
    assert!(
        output
            .lines()
            .any(|line| line.contains("● CLI") && line.contains("-0.000004"))
    );
    insta::assert_snapshot!(output);
}

#[test]
fn analytics_day_navigation_keeps_geometry_and_legend() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let mut history = fixture::history(/*report*/ 1, /*range*/ 0, /*group*/ 0);
    history.data[1].values.truncate(/*len*/ 1);
    history.data[1].total = history.data[1].values[0].value;
    history.data.remove(/*index*/ 2);
    view.sections[Section::Credits].history = Load::Ready(history);
    press(&mut view, KeyCode::Char('2'));
    let mut snapshots = Vec::new();
    for width in [58, 160] {
        let mut terminal = Terminal::new(TestBackend::new(width, /*height*/ 38)).unwrap();
        for expanded in [false, true] {
            press(&mut view, KeyCode::Home);
            if expanded {
                press(&mut view, KeyCode::Enter);
            }
            let mut previous = None;
            for _ in 0..7 {
                terminal
                    .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let bars = buffer
                    .content
                    .iter()
                    .enumerate()
                    .filter(|(_, cell)| {
                        matches!(cell.symbol(), "█" | "▁" | "▂" | "▃" | "▄" | "▅" | "▆" | "▇")
                    })
                    .map(|(index, cell)| (index, cell.clone()))
                    .collect::<Vec<_>>();
                let output = terminal.backend().to_string();
                let marker_row = output.lines().position(|line| line.contains('▲')).unwrap();
                let table_row = output
                    .lines()
                    .position(|line| line.contains("Product"))
                    .unwrap();
                let geometry = (bars, marker_row, table_row);
                if let Some(previous) = &previous {
                    assert_eq!(&geometry, previous);
                }
                previous = Some(geometry);
                assert!(output.contains("● CLI") && output.contains("● Desktop app"));
                if view.sections[Section::Credits].cursor == 2 && !expanded {
                    snapshots.push(output);
                }
                press(&mut view, KeyCode::Right);
            }
            if expanded {
                press(&mut view, KeyCode::Enter);
            }
        }
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}

#[test]
fn analytics_series_rank_by_range_and_keep_calendar_ticks_stable() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    screen(&mut view, /*width*/ 100, /*height*/ 38);
    press(&mut view, KeyCode::Char('r'));
    let mut history = fixture::history(/*report*/ 0, /*range*/ 1, /*group*/ 2);
    for day in &mut history.data {
        day.values[3].value += 500.0;
        day.total += 500.0;
    }
    view.sections[Section::Usage].history = Load::Ready(history.clone());
    screen(&mut view, /*width*/ 100, /*height*/ 38);
    let legend = view
        .history_lines(Section::Usage, /*width*/ 96, /*height*/ 10)
        .lines
        .into_iter()
        .filter(|line| line.spans.first().is_some_and(|span| span.content == "● "))
        .take(/*n*/ 3)
        .map(|line| (line.spans[1].content.to_string(), line.spans[0].style.fg))
        .collect::<Vec<_>>();
    assert_eq!(
        legend,
        ["Other", "GPT-5.6-Sol", "GPT-5.6-Terra"]
            .into_iter()
            .zip(styles::series_colors())
            .map(|(label, color)| (label.to_string(), Some(color)))
            .collect::<Vec<_>>()
    );
    let series = render::categories(&history);
    history.data[8].total = 0.4;
    for value in &mut history.data[8].values {
        value.value = 0.1;
    }
    let days = history
        .data
        .iter()
        .map(|day| (day.date, Some(day)))
        .collect::<Vec<_>>();
    let mut ticks = None;
    for cursor in [0, 7, 8, 25, 29] {
        let chart = plot::chart(
            &days,
            &series,
            cursor,
            /*width*/ 96,
            /*height*/ 10,
            history.unit,
        );
        let labels = chart.lines.last().unwrap().to_string();
        if let Some(ticks) = &ticks {
            assert_eq!(&labels, ticks);
        }
        ticks = Some(labels);
        assert_eq!(
            chart
                .lines
                .iter()
                .filter(|line| line.to_string().contains('▲'))
                .count(),
            1
        );
    }
    let crowded = plot::chart(
        &days,
        &series,
        /*cursor*/ 8,
        /*width*/ 54,
        /*height*/ 10,
        history.unit,
    )
    .lines
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(crowded.contains('┊'));
    assert!(crowded.contains("0.4"));
    insta::assert_snapshot!(crowded);
}

#[test]
fn analytics_usage_ranks_its_own_models_after_turns_load() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    screen(&mut view, /*width*/ 100, /*height*/ 38);
    press(&mut view, KeyCode::Char('g'));
    press(&mut view, KeyCode::Down);
    press(&mut view, KeyCode::Enter);
    let mut history = fixture::history(/*report*/ 0, /*range*/ 0, /*group*/ 2);
    for day in &mut history.data {
        for label in ["Model E", "Model F"] {
            day.values
                .push(crate::analytics::models::AccountAnalyticsValue {
                    key: label.into(),
                    label: label.into(),
                    value: 0.0,
                });
        }
        for value in &mut day.values {
            value.key = format!("remote-{}", value.key);
            value.label = format!("Remote {}", value.label);
        }
    }
    history.data[6].total = 100.0;
    for (value, amount) in history.data[6]
        .values
        .iter_mut()
        .zip([91.1, 0.0, 6.8, 2.1, 0.0, 0.0])
    {
        value.value = amount;
    }
    view.sections[Section::Usage].history = Load::Ready(history);
    let selected_day = screen(&mut view, /*width*/ 100, /*height*/ 38);
    for _ in 0..7 {
        screen(&mut view, /*width*/ 100, /*height*/ 38);
        let expected = if view.sections[Section::Usage].cursor == 6 {
            [
                ("Remote GPT-5.6-Sol", 0),
                ("Remote GPT-5.5", 2),
                ("Remote Other", 3),
            ]
        } else {
            [
                ("Remote GPT-5.6-Sol", 0),
                ("Remote GPT-5.6-Terra", 1),
                ("Remote GPT-5.5", 2),
            ]
        }
        .into_iter()
        .map(|(label, color)| (label.to_string(), Some(styles::series_colors()[color])))
        .collect::<Vec<_>>();
        let legend = view
            .history_lines(Section::Usage, /*width*/ 96, /*height*/ 10)
            .lines
            .into_iter()
            .filter(|line| line.spans.first().is_some_and(|span| span.content == "● "))
            .take(/*n*/ 3)
            .map(|line| (line.spans[1].content.to_string(), line.spans[0].style.fg))
            .collect::<Vec<_>>();
        assert_eq!(legend, expected);
        press(&mut view, KeyCode::Left);
    }
    insta::assert_snapshot!(selected_day);
}

#[test]
fn analytics_empty_turns_keep_geometry_and_disable_details() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    let mut history = fixture::history(/*report*/ 2, /*range*/ 0, /*group*/ 2);
    history.data[5].total = 0.0;
    for value in &mut history.data[5].values {
        value.value = 0.0;
    }
    history.data.remove(/*index*/ 6);
    view.sections[Section::Activity].history = Load::Ready(history);
    press(&mut view, KeyCode::Char('3'));
    let mut snapshots = Vec::new();
    for width in [58, 100] {
        view.sections[Section::Activity].cursor = 4;
        let populated = screen(&mut view, width, /*height*/ 38);
        assert!(populated.contains("reported messages"));
        assert!(populated.contains("details"));
        let marker_row = populated.lines().position(|line| line.contains('▲'));
        for cursor in [5, 6] {
            view.sections[Section::Activity].cursor = cursor;
            let empty = screen(&mut view, width, /*height*/ 38);
            assert!(empty.contains(if cursor == 5 {
                "No messages reported for"
            } else {
                "Data not reported for"
            }));
            assert!(!empty.contains("details"));
            assert!(!empty.contains('●'));
            assert_eq!(
                empty.lines().position(|line| line.contains('▲')),
                marker_row
            );
            press(&mut view, KeyCode::Enter);
            assert_eq!(view.sections[Section::Activity].detail, None);
            snapshots.push(empty);
        }
        view.sections[Section::Activity].cursor = 4;
        press(&mut view, KeyCode::Enter);
        assert_eq!(view.sections[Section::Activity].detail, Some(4));
        let expanded = screen(&mut view, width, /*height*/ 38);
        let marker_row = expanded.lines().position(|line| line.contains('▲'));
        press(&mut view, KeyCode::Right);
        let empty = screen(&mut view, width, /*height*/ 38);
        assert_eq!(
            empty.lines().position(|line| line.contains('▲')),
            marker_row
        );
        let details = view.sections[Section::Activity].detail;
        press(&mut view, KeyCode::Enter);
        assert_eq!(view.sections[Section::Activity].detail, details);
        view.sections[Section::Activity].detail = None;
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}

#[path = "analytics/chat_panel_tests.rs"]
mod chats_table;

#[test]
fn analytics_details_reflow_and_keep_focus() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    view.chats = Load::Ready(fixture::chats());
    press(&mut view, KeyCode::Char('z'));
    press(&mut view, KeyCode::Char('6'));
    press(&mut view, KeyCode::Enter);
    assert!(view.zoomed);
    press(&mut view, KeyCode::Enter);
    let wide = screen(&mut view, /*width*/ 120, /*height*/ 42);
    view.follow_selection = true;
    let narrow = screen(&mut view, /*width*/ 58, /*height*/ 20);
    assert!(narrow.contains("Q3 planning analysis") && narrow.contains("GPT-5.6-Sol"));
    insta::assert_snapshot!(format!("{wide}\n{narrow}"));
    let before = (
        view.section,
        view.sections.0.each_ref().map(|state| state.cursor),
        view.sections.0.each_ref().map(|state| state.detail),
    );
    press(&mut view, KeyCode::PageDown);
    screen(&mut view, /*width*/ 58, /*height*/ 20);
    assert_eq!(
        (
            view.section,
            view.sections.0.each_ref().map(|state| state.cursor),
            view.sections.0.each_ref().map(|state| state.detail)
        ),
        before
    );
    press(&mut view, KeyCode::Tab);
    press(&mut view, KeyCode::Char('6'));
    assert_eq!(
        (
            view.section,
            view.sections.0.each_ref().map(|state| state.cursor),
            view.sections.0.each_ref().map(|state| state.detail)
        ),
        before
    );
    press(&mut view, KeyCode::Esc);
    assert_eq!(
        (
            view.is_done,
            view.sections.0.each_ref().map(|state| state.detail)
        ),
        (false, [None; 8])
    );
    press(&mut view, KeyCode::Esc);
    assert!(view.is_done);
}
