//! Summary reflow, aggregation selection, and independently unavailable profile sections.
use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn profile() -> codex_backend_client::AccountProfile {
    serde_json::from_value(json!({
        "profile":{"display_name":"Example User","username":"example.user"},
        "metadata":{"stats_as_of":"2026-09-09"},
        "stats":{"lifetime_tokens":142_300_000_000_i64,"peak_daily_tokens":7_000_000_000_i64,
            "longest_running_turn_sec":158820,"current_streak_days":158,"longest_streak_days":160,
            "daily_usage_buckets":[{"start_date":"2026-09-08","tokens":100},{"start_date":"2026-09-09","tokens":200}],
            "fast_mode_usage_percentage":37.0,"most_used_reasoning_effort":"high","most_used_reasoning_effort_percentage":26.0,
            "unique_skills_used":147,"total_skills_used":13190,"total_threads":21482,
            "top_invocations":[{"type":"skill","skill_name":"review","usage_count":1142},{"type":"plugin","plugin_name":"GitHub","usage_count":1059},{"type":"future"}]}
    })).unwrap()
}

#[test]
fn summary_modes_reflow_and_keep_selection_across_tabs() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.profile = Load::Ready(profile());
    view.end_date = "2026-09-09".parse().unwrap();
    view.select_summary(Some(crate::analytics::TokenActivityView::Daily));
    insta::assert_snapshot!(
        "summary_daily_wide",
        screen(&mut view, /*width*/ 140, /*height*/ 44)
    );
    press(&mut view, KeyCode::Char('g'));
    assert_eq!(view.sections[Section::Summary].group, 1);
    assert!(view.profile.ready().is_some());
    press(&mut view, KeyCode::Tab);
    press(&mut view, KeyCode::BackTab);
    assert_eq!(view.sections[Section::Summary].group, 1);
    press(&mut view, KeyCode::Char('g'));
    insta::assert_snapshot!(
        "summary_cumulative_narrow",
        screen(&mut view, /*width*/ 64, /*height*/ 60)
    );
    for width in [1, 4, 28, 64] {
        let lines = view.summary_lines(width);
        assert!(lines.iter().all(|line| line.width() <= width));
    }
    screen(&mut view, /*width*/ 64, /*height*/ 18);
    press(&mut view, KeyCode::End);
    screen(&mut view, /*width*/ 64, /*height*/ 18);
    assert!(view.scroll_offset() > 0);
    press(&mut view, KeyCode::Home);
    assert_eq!(view.scroll_offset(), 0);
    press(&mut view, KeyCode::Esc);
    assert!(view.is_done);
}

#[test]
fn partial_summary_keeps_missing_values_and_zero() {
    let mut view = fixture::view(models::AccountKind::Business);
    view.select_summary(/*view*/ None);
    view.profile = Load::Ready(serde_json::from_value(json!({
        "metadata":{"stats_error":"sensitive upstream error"},
        "stats":{"lifetime_tokens":0,"fast_mode_usage_percentage":0,"total_threads":0,"top_invocations":[]}
    })).unwrap());
    let rendered = screen(&mut view, /*width*/ 100, /*height*/ 40);
    assert!(!rendered.contains("sensitive upstream error"));
    insta::assert_snapshot!("summary_partial", rendered);
    view.refresh();
    assert!(view.profile.ready().is_none());
}

#[tokio::test]
async fn summary_loads_once_per_open_and_does_not_block_other_reports() {
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    let server = test_support::server().await;
    let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(/*v*/ 0));
    let count = std::sync::Arc::clone(&requests);
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/profiles/me"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(move |_: &wiremock::Request| {
            if count.fetch_add(/*val*/ 1, std::sync::atomic::Ordering::SeqCst) == 0 {
                ResponseTemplate::new(/*s*/ 503)
            } else {
                ResponseTemplate::new(/*s*/ 200)
                    .set_body_json(json!({"stats":{"lifetime_tokens":123}}))
            }
        })
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    let (_home, _app_server, mut view) = client::tests::connected_view(&server, "plus").await;
    test_support::settle(&mut view).await;
    assert_eq!(view.section, Section::Summary);
    assert!(matches!(view.profile, Load::Error(_)));
    assert!(view.sections[Section::Usage].history.ready().is_some());
    press(&mut view, KeyCode::Char('g'));
    press(&mut view, KeyCode::Tab);
    test_support::settle(&mut view).await;
    assert!(view.sections[Section::Usage].history.ready().is_some());
    press(&mut view, KeyCode::BackTab);
    test_support::settle(&mut view).await;
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
    let (config, handle, frame) = view.connection.as_ref().unwrap().clone();
    view.cancel_loads();
    view.open(handle, frame, Vec::new(), config);
    test_support::settle(&mut view).await;
    assert_eq!(
        view.profile.ready().unwrap().stats.tokens.lifetime_tokens,
        Some(123)
    );
    assert_eq!(view.sections[Section::Summary].group, 1);
    view.cancel_loads();
    assert!(matches!(view.profile, Load::Unavailable));
}

#[tokio::test]
async fn closing_summary_aborts_its_pending_profile() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel::<()>();
    view.profile = Load::start(
        async move {
            let _cancelled = cancelled_tx;
            std::future::pending().await
        },
        crate::tui::FrameRequester::test_dummy(),
    );
    view.cancel_loads();
    assert!(cancelled_rx.await.is_err());
}

#[test]
fn summary_scroll_hints_follow_overflow_and_grouping_wraps() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.profile = Load::Ready(profile());
    view.select_summary(Some(crate::analytics::TokenActivityView::Daily));
    assert!(!screen(&mut view, /*width*/ 140, /*height*/ 60).contains("scroll"));
    assert!(screen(&mut view, /*width*/ 80, /*height*/ 18).contains("scroll"));
    let mut groups = Vec::new();
    for _ in 0..4 {
        groups.push(view.sections[Section::Summary].group);
        press(&mut view, KeyCode::Char('g'));
    }
    assert_eq!(groups, vec![0, 1, 2, 0]);
    assert!(!screen(&mut view, /*width*/ 140, /*height*/ 60).contains("scroll"));
}
