//! Account-scoped authentication and request identity regression coverage.

use super::*;
use crate::analytics::sections::Section;
use crate::legacy_core::config::ConfigBuilder;
use base64::Engine;
use codex_config::LoaderOverrides;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

pub(super) fn sign_in(home: &std::path::Path, account: &str, user: &str, plan: &str) {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({
            "exp": 4102444800_i64, "email": "analytics@example.test",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account, "chatgpt_user_id": user, "chatgpt_plan_type": plan,
            },
        })
        .to_string(),
    );
    let token = format!("e30.{claims}.test");
    let auth = serde_json::from_value(json!({
        "auth_mode": "chatgpt", "tokens": {"id_token": token, "access_token": token,
            "refresh_token": "test-refresh", "account_id": account},
        "last_refresh": chrono::Utc::now(),
    }))
    .unwrap();
    codex_login::save_auth(
        home,
        &auth,
        codex_login::AuthCredentialsStoreMode::File,
        codex_login::AuthKeyringBackendKind::default(),
    )
    .unwrap();
}

pub(in crate::analytics) async fn live(
    server: &MockServer,
    plan: &str,
) -> (tempfile::TempDir, Live) {
    let home = tempfile::tempdir().unwrap();
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await
        .unwrap();
    config.chatgpt_base_url = format!("{}/backend-api", server.uri());
    config.cli_auth_credentials_store_mode = codex_login::AuthCredentialsStoreMode::File;
    sign_in(home.path(), "account-a", "user-a", plan);
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "accounts": [{"id": "account-a", "plan_type": plan}],
            "account_ordering": ["account-a"]
        })))
        .with_priority(/*p*/ 2)
        .mount(server)
        .await;
    (
        home,
        Live::new(Arc::new(config), "2026-01-15".parse().unwrap()),
    )
}

async fn report_requests(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() != "/backend-api/wham/accounts/check")
        .collect()
}

#[tokio::test]
async fn analytics_rejects_identity_changes_during_requests() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "business").await;
    let session = live.session().await.unwrap();
    let authenticated = &session.backend;
    let result = authenticated
        .request(|_| async {
            sign_in(home.path(), "account-b", "user-a", "business");
            Ok(123)
        })
        .await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "Account changed. Press R to refresh Analytics."
    );
    assert!(report_requests(&server).await.is_empty());
}

#[tokio::test]
async fn analytics_rejects_account_and_user_changes_before_requests() {
    for (account, user) in [("account-b", "user-a"), ("account-a", "user-b")] {
        let server = MockServer::start().await;
        let (home, live) = live(&server, "plus").await;
        let session = live.session().await.unwrap();
        let authenticated = &session.backend;
        sign_in(home.path(), account, user, "plus");
        let requested = std::cell::Cell::new(/*value*/ false);
        let result = authenticated
            .request(|_| async {
                requested.set(/*val*/ true);
                Ok(())
            })
            .await;
        assert!(!requested.get());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Account changed. Press R to refresh Analytics."
        );
    }
}

#[tokio::test]
async fn analytics_requires_local_chatgpt_authentication() {
    for auth in [None, Some(json!({"OPENAI_API_KEY": "sk-test-only"}))] {
        let server = MockServer::start().await;
        let (home, live) = live(&server, "plus").await;
        let path = home.path().join("auth.json");
        match auth {
            Some(auth) => std::fs::write(path, serde_json::to_vec(&auth).unwrap()).unwrap(),
            None => std::fs::remove_file(path).unwrap(),
        }
        assert_eq!(
            live.session().await.err(),
            Some("Sign in locally with ChatGPT to view Analytics.".to_string())
        );
        assert!(report_requests(&server).await.is_empty());
    }
}

#[tokio::test]
async fn analytics_requests_use_reloaded_credentials_for_the_same_identity() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    let session = live.session().await.unwrap();
    let authenticated = &session.backend;
    assert_eq!(
        live.account_label().as_deref(),
        Some("analytics@example.test · account-a")
    );
    let auth_path = home.path().join("auth.json");
    let mut auth: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&auth_path).unwrap()).unwrap();
    auth["tokens"]["access_token"] = json!("refreshed-access-token");
    std::fs::write(auth_path, serde_json::to_vec(&auth).unwrap()).unwrap();
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/usage/daily-token-usage-breakdown"))
        .and(header("chatgpt-account-id", "account-a"))
        .and(header("authorization", "Bearer refreshed-access-token"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": []})))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let result = authenticated
        .request(|client| async move {
            client
                .get_account_analytics(
                    codex_backend_client::AnalyticsReport::Usage,
                    "2026-09-01",
                    "2026-09-07",
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(
        result,
        codex_backend_client::AnalyticsResponse::Usage(
            codex_backend_client::analytics_models::DailyProductSurfaceUsageResponse::default()
        )
    );
    server.verify().await;
}

#[tokio::test]
async fn analytics_retries_unauthorized_requests_after_credentials_reload() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    let session = live.session().await.unwrap();
    let authenticated = &session.backend;
    let auth_path = home.path().join("auth.json");
    let mut auth: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&auth_path).unwrap()).unwrap();
    let original_token = auth["tokens"]["access_token"].as_str().unwrap().to_owned();
    auth["tokens"]["access_token"] = json!("recovered-access-token");
    let updated_auth = serde_json::to_vec(&auth).unwrap();
    Mock::given(method("GET"))
        .and(header("authorization", format!("Bearer {original_token}")))
        .respond_with(move |_: &wiremock::Request| {
            std::fs::write(&auth_path, &updated_auth).unwrap();
            ResponseTemplate::new(/*s*/ 401)
        })
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer recovered-access-token"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": []})))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let result = authenticated
        .request(|client| async move {
            client
                .get_account_analytics(
                    codex_backend_client::AnalyticsReport::Usage,
                    "2026-09-01",
                    "2026-09-07",
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(
        result,
        codex_backend_client::AnalyticsResponse::Usage(
            codex_backend_client::analytics_models::DailyProductSurfaceUsageResponse::default()
        )
    );
    server.verify().await;
}

#[tokio::test]
async fn analytics_retries_session_initialization_after_sign_in() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    std::fs::remove_file(home.path().join("auth.json")).unwrap();
    assert!(live.session().await.is_err());
    sign_in(home.path(), "account-a", "user-a", "plus");
    assert_eq!(
        live.session().await.unwrap().backend.account().id,
        "account-a"
    );
}

#[tokio::test]
async fn analytics_matches_app_account_routes_and_parameters() {
    let server = MockServer::start().await;
    for (plan, report, grouping, endpoint, extra) in [
        (
            "plus",
            Report::Usage,
            Grouping::Model,
            "usage/daily-token-usage-breakdown",
            vec![("group_by", "day")],
        ),
        (
            "business",
            Report::Usage,
            Grouping::TokenType,
            "usage/daily-workspace-user-token-usage-breakdown",
            vec![
                ("group_by", "day"),
                ("breakdown_by", "model"),
                ("modes", "codex"),
                ("modes", "work"),
            ],
        ),
        (
            "business",
            Report::Messages,
            Grouping::Model,
            "analytics/daily-workspace-usage-counts",
            vec![("group_by", "day"), ("workspace_user", "true")],
        ),
        (
            "unknown",
            Report::Usage,
            Grouping::Surface,
            "usage/daily-token-usage-breakdown",
            vec![("group_by", "day")],
        ),
        (
            "unknown",
            Report::Messages,
            Grouping::Model,
            "analytics/daily-workspace-usage-counts",
            vec![("group_by", "day"), ("workspace_user", "true")],
        ),
        (
            "unknown",
            Report::Plugins,
            Grouping::Feature,
            "analytics/daily-plugin-usage-metrics",
            vec![
                ("group_by", "day"),
                ("workspace_user", "true"),
                ("top_plugin_limit", "10"),
            ],
        ),
        (
            "unknown",
            Report::Skills,
            Grouping::Feature,
            "analytics/daily-skill-usage-metrics",
            vec![
                ("group_by", "day"),
                ("workspace_user", "true"),
                ("top_skill_limit", "10"),
            ],
        ),
        (
            "edu",
            Report::Credits,
            Grouping::Model,
            "usage/daily-workspace-user-credit-usage",
            vec![("breakdown", "model")],
        ),
        (
            "plus",
            Report::Credits,
            Grouping::Surface,
            "usage/credit-usage-events",
            vec![],
        ),
        (
            "team",
            Report::Usage,
            Grouping::Model,
            "usage/daily-workspace-user-token-usage-breakdown",
            vec![("group_by", "day")],
        ),
        (
            "team",
            Report::Credits,
            Grouping::Speed,
            "usage/daily-workspace-user-token-usage-breakdown",
            vec![("group_by", "day")],
        ),
        (
            "business",
            Report::Credits,
            Grouping::Reasoning,
            "usage/daily-workspace-user-credit-usage",
            vec![("breakdown", "reasoning_effort")],
        ),
        (
            "plus",
            Report::Messages,
            Grouping::Model,
            "analytics/daily-workspace-usage-counts",
            vec![("group_by", "day"), ("workspace_user", "true")],
        ),
        (
            "plus",
            Report::Plugins,
            Grouping::Feature,
            "analytics/daily-plugin-usage-metrics",
            vec![
                ("group_by", "day"),
                ("workspace_user", "true"),
                ("top_plugin_limit", "10"),
            ],
        ),
        (
            "business",
            Report::Plugins,
            Grouping::Feature,
            "analytics/daily-plugin-usage-metrics",
            vec![
                ("group_by", "day"),
                ("workspace_user", "true"),
                ("top_plugin_limit", "8"),
            ],
        ),
        (
            "business",
            Report::Skills,
            Grouping::Feature,
            "analytics/daily-skill-usage-metrics",
            vec![
                ("group_by", "day"),
                ("workspace_user", "true"),
                ("top_skill_limit", "6"),
            ],
        ),
    ] {
        server.reset().await;
        let (_home, live) = live(&server, plan).await;
        let today = live.end_date;
        let start = (today - chrono::Days::new(/*num*/ 6)).to_string();
        let response = if endpoint == "usage/daily-workspace-user-credit-usage" {
            let breakdown = extra.iter().find(|(key, _)| *key == "breakdown").unwrap().1;
            json!({"data": [], "series": [], "breakdown": breakdown})
        } else {
            json!({"data": []})
        };
        Mock::given(method("GET"))
            .and(path(format!("/backend-api/wham/{endpoint}")))
            .and(header("chatgpt-account-id", "account-a"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(response))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert!(
            live.history(report, /*days*/ 7, grouping)
                .await
                .unwrap()
                .is_some()
        );
        let requests = report_requests(&server).await;
        let mut actual = requests[0]
            .url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        let mut expected = extra
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<Vec<_>>();
        if endpoint != "usage/credit-usage-events" {
            expected.push(("start_date".into(), start.clone()));
            expected.push(("end_date".into(), today.to_string()));
        }
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        assert!(requests[0].headers.get("authorization").is_some());
    }
}

#[tokio::test]
async fn analytics_uses_api_response_for_usage_and_turns_availability() {
    let server = MockServer::start().await;
    for report in [Report::Usage, Report::Messages] {
        for status in [403, 404] {
            server.reset().await;
            let (_home, live) = live(&server, "business").await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(status))
                .expect(/*r*/ 1)
                .mount(&server)
                .await;
            assert_eq!(
                live.history(report, /*days*/ 7, Grouping::Surface)
                    .await
                    .unwrap_err(),
                if status == 403 {
                    "Access denied for this report."
                } else {
                    "This report endpoint is unavailable. Press R to retry."
                }
            );
            server.verify().await;
        }
    }
}

#[tokio::test]
async fn analytics_reuses_payload_for_grouping_and_rejects_account_changes() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "team").await;
    let today = live.end_date.to_string();
    Mock::given(method("GET"))
        .and(path(
            "/backend-api/wham/usage/daily-workspace-user-token-usage-breakdown",
        ))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": [{
                "date": today, "product_surface_usage_values": {}, "premium_usage_values": {
                    "credit_usage_credits": {"cli": 12.5}, "total_usage_credits": {},
                    "uncached_text_input_tokens_by_surface": {}, "cached_text_input_tokens_by_surface": {},
                    "text_output_tokens_by_surface": {}, "text_total_tokens_by_surface": {}
                },
                "models": [{"model": "model-a", "credits": 12.5, "speed": "fast", "on_demand_credits": 12.5}]
            }]})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let surface = live
        .history(Report::Credits, /*days*/ 7, Grouping::Surface)
        .await
        .unwrap()
        .unwrap();
    let model = live
        .history(Report::Credits, /*days*/ 7, Grouping::Model)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            surface.data.last().unwrap().total,
            model.data.last().unwrap().total
        ),
        (12.5, 12.5)
    );
    sign_in(home.path(), "account-a", "user-b", "team");
    assert_eq!(
        live.history(Report::Credits, /*days*/ 7, Grouping::Model)
            .await
            .unwrap_err(),
        "Account changed. Press R to refresh Analytics."
    );
}

#[tokio::test]
async fn token_model_filter_reuses_the_account_scoped_response() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "business").await;
    let today = live.end_date.to_string();
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/usage/daily-workspace-user-token-usage-breakdown"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data":[{
            "date":today, "product_surface_usage_values": {}, "groups":[
                {"dimensions":{"model":"alpha"}, "credits": 0, "uncached_text_input_tokens":10,"cached_text_input_tokens":20,"text_output_tokens":30},
                {"dimensions":{"model":"beta"}, "credits": 0, "uncached_text_input_tokens":100,"cached_text_input_tokens":200,"text_output_tokens":300}
            ]
        }]})))
        .expect(/*r*/ 1).mount(&server).await;
    let all = live
        .history(Report::Usage, /*days*/ 7, Grouping::TokenType)
        .await
        .unwrap()
        .unwrap();
    let alpha = live
        .filtered_history(
            Report::Usage,
            /*days*/ 7,
            Grouping::TokenType,
            Some("alpha"),
        )
        .await
        .unwrap()
        .unwrap();
    let beta = live
        .filtered_history(
            Report::Usage,
            /*days*/ 7,
            Grouping::TokenType,
            Some("beta"),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (all.data[0].total, alpha.data[0].total, beta.data[0].total),
        (660.0, 60.0, 600.0)
    );
    assert_eq!(
        live.token_models(),
        vec!["beta".to_string(), "alpha".to_string()]
    );
    assert_eq!(report_requests(&server).await.len(), 1);
}

#[tokio::test]
async fn legacy_usage_falls_back_to_surface_for_attribution_only_groupings() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "plus").await;
    let today = live.end_date.to_string();
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": [{
                "date": today, "product_surface_usage_values": {"cli": 12.5}
            }]})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let surface = live
        .history(Report::Usage, /*days*/ 7, Grouping::Surface)
        .await
        .unwrap();
    for grouping in [Grouping::Feature, Grouping::TaskStart] {
        let history = live
            .history(Report::Usage, /*days*/ 7, grouping)
            .await
            .unwrap();
        assert_eq!((live.attributed_usage(), history), (false, surface.clone()));
    }
    server.verify().await;
}

#[tokio::test]
async fn invalid_ranges_fail_without_requesting_reports() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "plus").await;
    for days in [0, u32::MAX] {
        assert_eq!(
            live.history(Report::Usage, days, Grouping::Surface)
                .await
                .unwrap_err(),
            "Invalid analytics date range."
        );
    }
    assert!(report_requests(&server).await.is_empty());
}

#[tokio::test]
async fn unknown_plan_exposes_no_credit_breakdowns() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "unknown").await;
    live.session().await.unwrap();
    assert!(live.credit_groups().is_empty());
    assert!(
        live.history(Report::Credits, /*days*/ 7, Grouping::Surface)
            .await
            .is_err()
    );
    assert!(report_requests(&server).await.is_empty());
}

#[tokio::test]
async fn in_flight_account_switch_retains_refresh_guidance() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    let home_path = home.path().to_path_buf();
    Mock::given(method("GET"))
        .respond_with(move |_: &wiremock::Request| {
            sign_in(&home_path, "account-b", "user-a", "plus");
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": []}))
        })
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    assert_eq!(
        live.history(Report::Usage, /*days*/ 7, Grouping::Surface)
            .await
            .unwrap_err(),
        "Account changed. Press R to refresh Analytics."
    );
    server.verify().await;
}

#[tokio::test]
async fn missing_breakdowns_remain_unavailable() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "plus").await;
    let today = live.end_date.to_string();
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(
                json!({"data": [{"date": today, "product_surface_usage_values": {}}]}),
            ),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    assert_eq!(
        live.history(Report::Usage, /*days*/ 7, Grouping::Model)
            .await,
        Ok(None)
    );
    server.verify().await;
}

#[tokio::test]
async fn out_of_range_legacy_rows_do_not_disable_attribution() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "plus").await;
    let today = live.end_date.to_string();
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": [
                {"date": "2000-01-01", "product_surface_usage_values": {}},
                {"date": today, "product_surface_usage_values": {}, "attribution": [{"thread_source": "user", "turn_trigger": "composer", "model": "example", "surface": "cli", "value": 12.0}]}
            ]})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let history = live
        .history(Report::Usage, /*days*/ 7, Grouping::Feature)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        history.data,
        vec![super::super::models::AccountAnalyticsDay {
            date: live.end_date,
            total: 12.0,
            values: vec![super::super::models::AccountAnalyticsValue {
                key: "user".into(),
                label: "Tasks".into(),
                value: 12.0,
            }],
        }]
    );
    assert!(live.attributed_usage());
}

#[tokio::test]
async fn invalid_reports_can_retry_without_recreating_the_session() {
    for plan in ["plus", "business"] {
        let server = MockServer::start().await;
        let (_home, live) = live(&server, plan).await;
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(/*v*/ 0));
        let count = Arc::clone(&requests);
        Mock::given(method("GET"))
            .respond_with(move |_: &wiremock::Request| {
                let body = if count.fetch_add(/*val*/ 1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    json!({"data": [{"date": "invalid"}]})
                } else {
                    json!({"data": []})
                };
                ResponseTemplate::new(/*s*/ 200).set_body_json(body)
            })
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        assert!(
            live.history(Report::Usage, /*days*/ 7, Grouping::Surface)
                .await
                .is_err()
        );
        assert!(
            live.history(Report::Usage, /*days*/ 7, Grouping::Surface)
                .await
                .is_ok()
        );
        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 2);
        server.verify().await;
    }
}

#[tokio::test]
async fn invalid_cached_breakdowns_are_evicted_before_retry() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "plus").await;
    let today = live.end_date.to_string();
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(/*v*/ 0));
    let count = Arc::clone(&attempts);
    Mock::given(method("GET"))
        .respond_with(move |_: &wiremock::Request| {
            let model = if count.fetch_add(/*val*/ 1, std::sync::atomic::Ordering::SeqCst) == 0 {
                json!({"model": "alpha", "credits": -1})
            } else {
                json!({"model": "alpha", "credits": 12})
            };
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": [{
                "date": today, "product_surface_usage_values": {"cli": 12}, "models": [model]
            }]}))
        })
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    live.history(Report::Usage, /*days*/ 7, Grouping::Surface)
        .await
        .unwrap();
    assert!(
        live.history(Report::Usage, /*days*/ 7, Grouping::Model)
            .await
            .is_err()
    );
    assert!(
        live.history(Report::Usage, /*days*/ 7, Grouping::Model)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    server.verify().await;
}

pub(in crate::analytics) async fn connected_view(
    server: &MockServer,
    plan: &str,
) -> (
    tempfile::TempDir,
    crate::AppServerSession,
    crate::analytics::AnalyticsView,
) {
    let (home, live) = live(server, plan).await;
    let config = Arc::clone(&live.config);
    let app_server = crate::start_embedded_app_server_for_picker(&config)
        .await
        .unwrap();
    let mut view =
        crate::analytics::AnalyticsView::new(crate::keymap::RuntimeKeymap::defaults().list);
    view.open(
        app_server.request_handle(),
        crate::tui::FrameRequester::test_dummy(),
        Vec::new(),
        config,
    );
    (home, app_server, view)
}

#[tokio::test]
async fn live_account_identity_is_visible_and_closing_aborts_pending_loads() {
    let server = MockServer::start().await;
    let (_home, live) = live(&server, "plus").await;
    live.session().await.unwrap();
    let mut view =
        crate::analytics::AnalyticsView::new(crate::keymap::RuntimeKeymap::defaults().list);
    view.live = Some(Arc::new(live));
    view.sections[Section::Usage].history =
        super::super::data::Load::Error("Couldn't load usage. Press R to retry.".into());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
        /*width*/ 100, /*height*/ 24,
    ))
    .unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!(terminal.backend().to_string());
    let (draw_tx, _) = tokio::sync::broadcast::channel(/*capacity*/ 1);
    let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel::<()>();
    view.sections[Section::Usage].history = super::super::data::Load::start(
        async move {
            let _cancelled = cancelled_tx;
            std::future::pending().await
        },
        crate::tui::FrameRequester::new(draw_tx),
    );
    view.cancel_loads();
    assert!(cancelled_rx.await.is_err());
}

#[tokio::test]
async fn plan_history_rejects_account_change_during_response() {
    let server = MockServer::start().await;
    for status in [200, 404, 503] {
        server.reset().await;
        let (home, live) = live(&server, "plus").await;
        let home_path = home.path().to_path_buf();
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/usage/plan_limit_history"))
            .and(header("chatgpt-account-id", "account-a"))
            .respond_with(move |_: &wiremock::Request| {
                sign_in(&home_path, "account-a", "user-b", "plus");
                ResponseTemplate::new(status).set_body_json(json!({
                    "data_as_of":"2026-09-02T00:00:00Z", "coverage_start":null,
                    "coverage_complete":true, "periods":[]
                }))
            })
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert_eq!(
            live.plan_history().await.err().unwrap(),
            "Account changed. Press R to refresh Analytics."
        );
    }
}
