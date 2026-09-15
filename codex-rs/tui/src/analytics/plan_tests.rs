//! Snapshot semantics and independent plan-period navigation without a live billing service.
use super::*;
use crate::analytics::fixture;
use crate::analytics::models::AccountKind;
use crate::analytics::sections::Section;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use serde_json::json;

fn response() -> PlanLimitHistory {
    serde_json::from_value(json!({
        "data_as_of":"2026-09-02T12:00:00Z", "coverage_start":"2026-09-01T00:00:00Z",
        "coverage_complete":false, "boundary_tolerance_seconds":60,
        "periods":[
            {"id":"five", "window_minutes":300,"plan_type":"plus",
             "starts_at":"2026-09-02T10:00:00Z","ends_at":"2026-09-02T15:00:00Z",
             "accounting_complete":false,"used_basis_points":12500,
             "breakdowns":[
                {"dimension":"future_dimension","rows":[{"key":"future","basis_points":500}]},
                {"dimension":"thread_source","rows":[{"key":"user","basis_points":12500}]}]},
            {"id":"weekly", "window_minutes":10080,"plan_type":"plus",
             "starts_at":"2026-09-01T00:00:00Z","ends_at":"2026-09-08T00:00:00Z",
             "accounting_complete":true,"used_basis_points":null,"breakdowns":null}
        ]
    }))
    .unwrap()
}

#[test]
fn period_navigation_preserves_unknowns_and_resets_on_new_snapshot() {
    let mut view = fixture::view(AccountKind::Consumer);
    view.plan.enabled = true;
    view.plan.report = Load::Ready(Report::parse(response()).unwrap().unwrap());
    view.plan.poll();
    view.section = Section::Plan;
    view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    view.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        view.plan.expanded,
        [Some("five".into()), Some("weekly".into())]
    );
    let mut output = Vec::new();
    for width in [120, 58] {
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
            width, /*height*/ 30,
        ))
        .unwrap();
        terminal
            .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
            .unwrap();
        output.push(terminal.backend().to_string());
    }
    insta::assert_snapshot!(output.join("\n"));
    let mut next = response();
    next.data_as_of = Some("2026-09-02T13:00:00Z".into());
    view.plan.report = Load::Ready(Report::parse(next).unwrap().unwrap());
    view.plan.poll();
    assert_eq!(
        (view.plan.cursor, view.plan.expanded),
        ([0, 0], [None, None])
    );
}

#[test]
fn unavailable_empty_and_zero_history_are_distinct() {
    let mut view = fixture::view(AccountKind::Consumer);
    view.plan.enabled = true;
    view.section = Section::Plan;
    let mut states = Vec::new();
    states.push(
        view.plan_lines(/*width*/ 100)
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut history = response();
    history.coverage_complete = true;
    history.periods.clear();
    view.plan.report = Load::Ready(Report::parse(history).unwrap().unwrap());
    states.push(
        view.plan_lines(/*width*/ 100)
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut history = response();
    history.periods[0].used_basis_points = Some(0.0);
    view.plan.report = Load::Ready(Report::parse(history).unwrap().unwrap());
    states.push(
        view.plan_lines(/*width*/ 100)
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut future = response();
    future.periods[0].starts_at = "2026-09-02T13:00:00Z".into();
    view.plan.report = Load::Ready(Report::parse(future).unwrap().unwrap());
    states.push(
        view.plan_lines(/*width*/ 100)
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let snapshot = states.join("\n\n");
    let snapshot = snapshot
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(snapshot);
    let mut missing = response();
    missing.data_as_of = None;
    assert!(Report::parse(missing).unwrap().is_none());
}

#[tokio::test]
async fn plan_gate_prevents_requests_and_unavailable_plan_does_not_block_other_reports() {
    use crate::analytics::client;
    use crate::analytics::test_support;
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::path;
    let server = test_support::server().await;
    let (_home, _server, mut view) = client::tests::connected_view(&server, "plus").await;
    test_support::settle(&mut view).await;
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|request| request.url.path().ends_with("plan_limit_history"))
    );
    assert!(!view.visible_sections().contains(&Section::Plan));
    Mock::given(path("/backend-api/wham/usage/plan_limit_history"))
        .respond_with(ResponseTemplate::new(/*s*/ 404))
        .with_priority(/*p*/ 1)
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let (mut config, handle, frame) = view.connection.clone().unwrap();
    std::sync::Arc::make_mut(&mut config)
        .features
        .enable(codex_features::Feature::AnalyticsPlanHistory)
        .unwrap();
    view.open(handle, frame, Vec::new(), config);
    test_support::settle(&mut view).await;
    assert!(matches!(view.plan.report, Load::Unavailable));
    assert!(view.sections[Section::Usage].history.ready().is_some());
    assert!(view.visible_sections().contains(&Section::Plan));
    // Closing cancels the pending plan task along with the other account-owned work.
    let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
    view.plan.report = Load::start(
        async move {
            let _sender = sender;
            std::future::pending().await
        },
        crate::tui::FrameRequester::test_dummy(),
    );
    view.cancel_loads();
    assert!(receiver.await.is_err());
    server.verify().await;
    let server = test_support::server().await;
    let (_home, _server, mut business) = client::tests::connected_view(&server, "business").await;
    let (mut config, handle, frame) = business.connection.clone().unwrap();
    std::sync::Arc::make_mut(&mut config)
        .features
        .enable(codex_features::Feature::AnalyticsPlanHistory)
        .unwrap();
    business.open(handle, frame, Vec::new(), config);
    test_support::settle(&mut business).await;
    assert!(!business.visible_sections().contains(&Section::Plan));
    assert!(matches!(business.plan.report, Load::Unavailable));
    server.verify().await;
}

#[test]
fn plan_overview_escape_closes_with_retained_expansion() {
    let mut history = response();
    for index in 1..12 {
        let mut period = history.periods[0].clone();
        period.id = format!("five-{index}");
        history.periods.push(period);
    }
    let mut view = fixture::view(AccountKind::Consumer);
    view.plan.enabled = true;
    view.plan.report = Load::Ready(Report::parse(history).unwrap().unwrap());
    view.plan.poll();
    view.section = Section::Plan;
    view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    view.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
        /*width*/ 120, /*height*/ 60,
    ))
    .unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    let output = terminal.backend().to_string();
    assert!(output.contains("Latest period · 12 total") && output.contains("Weekly limits"));
    insta::assert_snapshot!(output);
    view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(view.is_done);
}

#[test]
fn moving_period_selection_collapses_its_previous_details() {
    let mut history = response();
    let mut older = history.periods[0].clone();
    older.id = "older-five".into();
    older.starts_at = "2026-09-02T05:00:00Z".into();
    older.ends_at = "2026-09-02T10:00:00Z".into();
    history.periods.push(older);
    let mut view = fixture::view(AccountKind::Consumer);
    view.plan.enabled = true;
    view.plan.report = Load::Ready(Report::parse(history).unwrap().unwrap());
    view.plan.poll();
    view.section = Section::Plan;
    view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(
        (view.plan.cursor, view.plan.expanded),
        ([1, 0], [None, None])
    );
}

#[test]
fn invalid_history_returns_safe_errors() {
    for invalid in 0..5 {
        let mut history = response();
        match invalid {
            0 => history.periods[0].starts_at = "private wire value".into(),
            1 => history.periods[0].ends_at = history.periods[0].starts_at.clone(),
            2 => history.periods.push(history.periods[0].clone()),
            3 => history.periods[0].window_minutes = 1,
            _ => history.periods[0].used_basis_points = Some(f64::INFINITY),
        }
        let error = Report::parse(history).err().unwrap();
        assert!(!error.contains("private wire value"));
    }
}
