//! Account layouts, authenticated report loading, independent ranges, and model filters.
use super::*;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use pretty_assertions::assert_eq;

use crate::analytics::sections::Section;
#[test]
fn account_layouts_show_reports_and_wrap_navigation() {
    let mut screens = Vec::new();
    for (kind, expected) in [
        (
            models::AccountKind::Consumer,
            vec![
                Section::Summary,
                Section::Usage,
                Section::Activity,
                Section::Plugins,
                Section::Skills,
                Section::Chats,
            ],
        ),
        (
            models::AccountKind::Enterprise,
            vec![
                Section::Summary,
                Section::Credits,
                Section::Usage,
                Section::Plugins,
                Section::Skills,
                Section::Chats,
            ],
        ),
    ] {
        let mut view = fixture::view(kind);
        for (index, section) in expected.iter().enumerate() {
            press(
                &mut view,
                KeyCode::Char(char::from_digit(index as u32 + 1, /*radix*/ 10).unwrap()),
            );
            assert_eq!(view.section, *section);
            screens.push(screen(&mut view, /*width*/ 100, /*height*/ 30));
        }
        press(&mut view, KeyCode::Tab);
        assert_eq!(view.section, expected[0]);
        press(&mut view, KeyCode::BackTab);
        assert_eq!(view.section, *expected.last().unwrap());
        view.zoomed = false;
        screens.push(screen(&mut view, /*width*/ 140, /*height*/ 64));
    }
    insta::assert_snapshot!(screens.join("\n"));
}

#[tokio::test]
async fn changing_ranges_and_grouping_preserves_other_reports_and_focus() {
    let server = test_support::server().await;
    let (_home, _app_server, mut view) = client::tests::connected_view(&server, "plus").await;
    test_support::settle(&mut view).await;
    view.section = Section::Usage;
    press(&mut view, KeyCode::Left);
    press(&mut view, KeyCode::Enter);
    press(&mut view, KeyCode::Char('r'));
    test_support::settle(&mut view).await;
    press(&mut view, KeyCode::Char('g'));
    test_support::settle(&mut view).await;
    assert_eq!(
        (
            view.ranges,
            view.sections.0.each_ref().map(|state| state.cursor),
            view.sections[Section::Usage].detail,
            view.sections[Section::Usage].group
        ),
        ([1, 0, 0], [28, 6, 6, 0, 6, 6, 6, 6], Some(28), 2)
    );
    let context = (
        view.ranges,
        view.sections.0.each_ref().map(|state| state.cursor),
        view.sections.0.each_ref().map(|state| state.detail),
        view.sections.0.each_ref().map(|state| state.group),
    );
    press(&mut view, KeyCode::Char('z'));
    screen(&mut view, /*width*/ 58, /*height*/ 25);
    press(&mut view, KeyCode::Char('z'));
    assert_eq!(
        (
            view.ranges,
            view.sections.0.each_ref().map(|state| state.cursor),
            view.sections.0.each_ref().map(|state| state.detail),
            view.sections.0.each_ref().map(|state| state.group)
        ),
        context
    );
    press(&mut view, KeyCode::Char('3'));
    press(&mut view, KeyCode::Char('r'));
    test_support::settle(&mut view).await;
    assert_eq!(view.ranges, [1, 1, 0]);
    assert_eq!(
        view.sections
            .0
            .each_ref()
            .map(|state| state.history.ready().map(|history| history.data.len())),
        [Some(30), Some(7), None, None, Some(30), Some(7), None, None]
    );
    let server = test_support::server().await;
    let (_business_home, _business_server, mut view) =
        client::tests::connected_view(&server, "business").await;
    test_support::settle(&mut view).await;
    view.ranges = [1, 1, 0];
    view.load_report(Section::Usage);
    press(&mut view, KeyCode::Char('2'));
    press(&mut view, KeyCode::Char('r'));
    test_support::settle(&mut view).await;
    assert_eq!(view.ranges, [0, 1, 0]);
    assert_eq!(
        view.sections
            .0
            .each_ref()
            .map(|state| state.history.ready().map(|history| history.data.len())),
        [Some(30), Some(7), Some(7), None, None, Some(7), None, None]
    );
}

#[tokio::test]
async fn business_tokens_filter_models() {
    let server = test_support::server().await;
    let (_home, _app_server, mut view) = client::tests::connected_view(&server, "business").await;
    test_support::settle(&mut view).await;
    view.end_date = fixture::END_DATE;
    fixture::seed_reports(&mut view);
    press(&mut view, KeyCode::Char('3'));
    let all = screen(&mut view, /*width*/ 100, /*height*/ 30);
    assert!(all.contains("All models") && all.contains("Cached input"));
    press(&mut view, KeyCode::Char('m'));
    test_support::settle(&mut view).await;
    assert_eq!(
        view.sections[Section::Usage]
            .history
            .ready()
            .unwrap()
            .data
            .iter()
            .map(|day| day.total)
            .sum::<f64>(),
        420.0
    );
    fixture::seed_reports(&mut view);
    assert_eq!(view.token_model.as_deref(), Some("GPT-5.5"));
    let filtered = screen(&mut view, /*width*/ 100, /*height*/ 30);
    press(&mut view, KeyCode::Char('m'));
    test_support::settle(&mut view).await;
    fixture::seed_reports(&mut view);
    assert_eq!(view.token_model, None);
    insta::assert_snapshot!(format!("{all}\n{filtered}"));
}

#[tokio::test]
async fn account_pending_and_error_do_not_start_reports() {
    let mut view = AnalyticsView::new(RuntimeKeymap::defaults().list);
    let (send, receive) = tokio::sync::oneshot::channel();
    view.account = Load::start(
        async move { receive.await.unwrap() },
        FrameRequester::test_dummy(),
    );
    screen(&mut view, /*width*/ 80, /*height*/ 24);
    assert!(
        view.sections
            .0
            .iter()
            .all(|state| matches!(state.history, Load::Unavailable))
    );
    send.send(Err("Sign in again to view Analytics.".to_string()))
        .unwrap();
    tokio::task::yield_now().await;
    let error = screen(&mut view, /*width*/ 80, /*height*/ 24);
    assert!(error.contains("Sign in again"));
    assert!(
        view.sections
            .0
            .iter()
            .all(|state| matches!(state.history, Load::Unavailable))
    );
    insta::assert_snapshot!(error);
}

#[tokio::test]
async fn account_reports_request_only_eligible_endpoints_and_refresh() {
    for (plan, usage, activity) in [
        (
            "plus",
            "usage/daily-token-usage-breakdown",
            "analytics/daily-workspace-usage-counts",
        ),
        (
            "business",
            "usage/daily-workspace-user-token-usage-breakdown",
            "usage/daily-workspace-user-credit-usage",
        ),
    ] {
        let server = test_support::server().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/backend-api/wham/profiles/me"))
            .respond_with(
                wiremock::ResponseTemplate::new(/*s*/ 200)
                    .set_body_json(serde_json::json!({"stats":{}})),
            )
            .mount(&server)
            .await;
        let (_home, _app_server, mut view) = client::tests::connected_view(&server, plan).await;
        test_support::settle(&mut view).await;
        let mut expected = [
            "accounts/check",
            usage,
            activity,
            "analytics/daily-plugin-usage-metrics",
            "analytics/daily-skill-usage-metrics",
            "profiles/me",
        ]
        .map(|endpoint| format!("/backend-api/wham/{endpoint}"));
        expected.sort();
        let mut paths = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|request| request.url.path().starts_with("/backend-api/wham/"))
            .map(|request| request.url.path().to_string())
            .collect::<Vec<_>>();
        paths.sort();
        assert_eq!(paths, expected);
        let ready = view
            .sections
            .0
            .each_ref()
            .map(|state| state.history.ready().is_some());
        assert_eq!(
            ready,
            if plan == "plus" {
                [true, true, false, false, true, true, false, false]
            } else {
                [true, true, true, false, false, true, false, false]
            }
        );
        press(&mut view, KeyCode::Char('R'));
        test_support::settle(&mut view).await;
        assert_eq!(
            view.sections
                .0
                .each_ref()
                .map(|state| state.history.ready().is_some()),
            ready
        );
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|request| request.url.path().starts_with("/backend-api/wham/"))
                .count(),
            12
        );
        view.cancel_loads();
    }
}

#[tokio::test]
async fn grouping_cycle_uses_current_server_capabilities() {
    let server = test_support::server().await;
    let (_home, _app_server, mut view) = client::tests::connected_view(&server, "plus").await;
    view.section = Section::Usage;
    view.account = Load::Ready(codex_protocol::account::PlanType::Plus);
    view.sections[Section::Usage].group = 3;
    view.start_reports();
    test_support::settle(&mut view).await;
    assert_eq!(
        (view.group_options(), view.sections[Section::Usage].group),
        (&[0, 2][..], 0)
    );
    press(&mut view, KeyCode::Char('g'));
    assert_eq!(view.sections[Section::Usage].group, 2);
    press(&mut view, KeyCode::Char('g'));
    assert_eq!(view.sections[Section::Usage].group, 0);
}

#[test]
fn ungrouped_sections_preserve_configured_g_jump_top() {
    let mut config = TuiKeymap::default();
    config.list.jump_top = Some(KeybindingsSpec::One(KeybindingSpec("g".into())));
    let keymap = RuntimeKeymap::from_config(&config).expect("valid list keymap");
    let mut view = fixture::view(models::AccountKind::Enterprise);
    view.keymap = keymap.list;
    for section in [
        Section::Chats,
        Section::Plugins,
        Section::Skills,
        Section::Credits,
    ] {
        view.section = section;
        press(&mut view, KeyCode::End);
        assert!(view.sections[section].cursor > 0, "{section:?}");
        press(&mut view, KeyCode::Char('g'));
        assert_eq!(view.sections[section].cursor, 0, "{section:?}");
    }
}

#[test]
fn business_chats_display_only_supplied_dollars() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let chats = match &mut view.chats {
        Load::Ready(chats) => chats,
        _ => panic!("fixture chats"),
    };
    for chat in &mut chats.rows {
        if let Some(usage) = &mut chat.usage {
            usage.estimated_usage_usd_micros = None;
        }
    }
    press(&mut view, KeyCode::Char('6'));
    assert!(!screen(&mut view, /*width*/ 100, /*height*/ 30).contains("Est. $ spent"));
    if let Load::Ready(chats) = &mut view.chats {
        chats.rows[0]
            .usage
            .as_mut()
            .unwrap()
            .estimated_usage_usd_micros = Some(123_450_000);
    }
    let dollars = screen(&mut view, /*width*/ 100, /*height*/ 30);
    assert!(dollars.contains("$123.45") && dollars.contains("Est. $ spent"));
    let narrow = screen(&mut view, /*width*/ 42, /*height*/ 30);
    insta::assert_snapshot!(format!("{dollars}\n{narrow}"));
}

#[tokio::test]
async fn unsupported_workspace_plans_hide_top_chats_and_skip_loading() {
    for plan in [
        "team",
        "self_serve_business_prolite",
        "self_serve_business_usage_based",
        "enterprise",
        "ent26",
        "edu",
        "edu_plus",
        "edu_pro",
    ] {
        let server = test_support::server().await;
        let (_home, _app_server, mut view) = client::tests::connected_view(&server, plan).await;
        test_support::settle(&mut view).await;
        assert_eq!(
            view.visible_sections(),
            &[
                Section::Summary,
                Section::Credits,
                Section::Usage,
                Section::Plugins,
                Section::Skills
            ],
            "{plan}"
        );
        assert!(matches!(view.chats, Load::Unavailable), "{plan}");
        let selected = view.section;
        press(&mut view, KeyCode::Char('6'));
        assert_eq!(view.section, selected);
    }
    for plan in [
        "business",
        "enterprise_cbp_usage_based",
        "enterprise_cbp_automation",
    ] {
        let server = test_support::server().await;
        let (_home, _app_server, mut view) = client::tests::connected_view(&server, plan).await;
        test_support::settle(&mut view).await;
        assert!(view.visible_sections().contains(&Section::Chats), "{plan}");
        press(&mut view, KeyCode::Char('6'));
        assert_eq!(view.section, Section::Chats);
    }
    let mut view = fixture::view(models::AccountKind::Enterprise);
    view.account = Load::Ready(codex_protocol::account::PlanType::Enterprise);
    insta::assert_snapshot!(screen(&mut view, /*width*/ 110, /*height*/ 24));
}

#[tokio::test]
async fn unknown_plan_keeps_summary_available_without_billing_reports() {
    let server = test_support::server().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/profiles/me"))
        .respond_with(
            wiremock::ResponseTemplate::new(/*s*/ 200)
                .set_body_json(serde_json::json!({"stats":{"lifetime_tokens":12345}})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let (_home, _app_server, mut view) =
        client::tests::connected_view(&server, "future-plan").await;
    test_support::settle(&mut view).await;
    assert_eq!(view.visible_sections(), &[Section::Summary]);
    assert_eq!(
        view.profile.ready().unwrap().stats.tokens.lifetime_tokens,
        Some(12345)
    );
    assert!(
        view.sections
            .0
            .iter()
            .all(|section| matches!(section.history, Load::Unavailable))
    );
    let paths = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path().starts_with("/backend-api/wham/"))
        .map(|request| request.url.path().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "/backend-api/wham/accounts/check",
            "/backend-api/wham/profiles/me"
        ]
    );
    view.end_date = "2026-09-09".parse().unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
        /*width*/ 100, /*height*/ 40,
    ))
    .unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!(terminal.backend().to_string());
}

#[test]
fn navigation_keeps_its_row_when_sections_change() {
    for width in [64, 120] {
        let mut view = fixture::view(models::AccountKind::Consumer);
        let mut positions = Vec::new();
        for section in [
            Section::Summary,
            Section::Usage,
            Section::Chats,
            Section::Summary,
        ] {
            view.section = section;
            let rendered = screen(&mut view, width, /*height*/ 40);
            positions.push(
                rendered
                    .lines()
                    .position(|line| line.contains("Summary") && line.contains("usage history"))
                    .unwrap(),
            );
        }
        assert_eq!(positions, vec![positions[0]; 4]);
    }
}

#[test]
fn taller_report_reveals_more_legend_rows_and_keeps_timestamp_in_footer() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.section = Section::Usage;
    let mut history = fixture::history(/*report*/ 0, /*range*/ 0, /*group*/ 1);
    history.updated_at = Some(1_788_364_800);
    for day in &mut history.data {
        day.values = (0..10)
            .map(|index| models::AccountAnalyticsValue {
                key: format!("feature-{index}"),
                label: format!("Feature {}", index + 1),
                value: day.total / 10.0,
            })
            .collect();
    }
    view.sections[Section::Usage].history = data::Load::Ready(history);
    let short = screen(&mut view, /*width*/ 120, /*height*/ 30);
    let tall = screen(&mut view, /*width*/ 120, /*height*/ 60);
    assert!(
        tall.lines().filter(|line| line.contains('●')).count()
            > short.lines().filter(|line| line.contains('●')).count()
    );
    assert!(
        tall.lines()
            .rev()
            .take(3)
            .any(|line| line.contains("Updated"))
    );
    insta::assert_snapshot!("legend_tall_report", tall);
    for width in [24, 40, 64] {
        let narrow = screen(&mut view, width, /*height*/ 60);
        assert!(narrow.lines().last().unwrap().contains("UTC"));
    }
}
