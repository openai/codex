//! HTTP contract coverage for account analytics report reads.

use super::*;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[tokio::test]
async fn account_analytics_uses_personal_daily_scope_and_plugin_contract() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/codex/analytics/daily-plugin-usage-metrics"))
        .and(query_param("start_date", "2026-01-03"))
        .and(query_param("end_date", "2026-01-09"))
        .and(query_param("group_by", "day"))
        .and(query_param("workspace_user", "true"))
        .and(query_param("top_plugin_limit", "10"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "data": [{"date": "2026-01-03", "plugin_usage_overviews": [{
                "plugin_id": null, "plugin_name": "example", "marketplace": null,
                "display_name": "Example plugin", "invocation_counts": 12
            }]}]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let response = client
        .get_account_analytics(
            AnalyticsReport::Plugins { limit: 10 },
            "2026-01-03",
            "2026-01-09",
        )
        .await
        .unwrap();
    assert_eq!(
        response,
        AnalyticsResponse::Plugins(models::PluginUsageMetricsResponse {
            data: vec![models::PluginUsageBucket {
                date: "2026-01-03".into(),
                plugin_usage_overviews: vec![models::PluginUsageOverview {
                    plugin_id: None,
                    plugin_name: "example".into(),
                    display_name: "Example plugin".into(),
                    marketplace: None,
                    invocation_counts: 12,
                }],
            }],
            data_freshness_ts: None,
            group_by: None,
        })
    );
}

#[tokio::test]
async fn account_analytics_preserves_signed_credit_events() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wham/usage/credit-usage-events"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "data": [{"date": "2026-01-03", "product_surface": "cli", "credit_amount": -0.004}]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .with_path_style(PathStyle::ChatGptApi);
    let response = client
        .get_account_analytics(AnalyticsReport::Credits, "2026-01-03", "2026-01-09")
        .await
        .unwrap();
    assert_eq!(
        response,
        AnalyticsResponse::Credits(models::CreditUsageEventsResponse {
            data: vec![models::CreditUsageEventBySurface {
                date: "2026-01-03".into(),
                product_surface: "cli".into(),
                credit_amount: -0.004,
                usage_id: None,
            }],
        })
    );
}

#[tokio::test]
async fn account_analytics_rejects_another_reports_shape_without_logging_the_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/codex/analytics/daily-workspace-usage-counts"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "data": [{"date": "2026-01-03", "product_surface": "sensitive-value", "credit_amount": 10}]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let client = Client::new(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let error = client
        .get_account_analytics(AnalyticsReport::Messages, "2026-01-03", "2026-01-09")
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Invalid analytics response.");
}

#[tokio::test]
async fn account_analytics_decodes_each_endpoint_and_accepts_new_string_values() {
    for (report, route, body, expected) in [
        (
            AnalyticsReport::Usage,
            "usage/daily-token-usage-breakdown",
            json!({"data": [{"date": "2026-01-03", "product_surface_usage_values": {"future_surface": 1.0}}], "units": "future_unit"}),
            AnalyticsResponse::Usage(models::DailyProductSurfaceUsageResponse {
                data: vec![models::DailyProductSurfaceUsage {
                    date: "2026-01-03".into(),
                    product_surface_usage_values: [("future_surface".into(), 1.0)].into(),
                    ..Default::default()
                }],
                units: Some("future_unit".into()),
                ..Default::default()
            }),
        ),
        (
            AnalyticsReport::EnterpriseCredits { breakdown: "model" },
            "usage/daily-workspace-user-credit-usage",
            json!({"data": [], "series": [], "breakdown": "model"}),
            AnalyticsResponse::EnterpriseCredits(models::CurrentUserCreditUsageResponse {
                breakdown: "model".into(),
                ..Default::default()
            }),
        ),
        (
            AnalyticsReport::Messages,
            "analytics/daily-workspace-usage-counts",
            json!({"data": []}),
            AnalyticsResponse::Messages(models::DailyWorkspaceUsageCountResponse::default()),
        ),
        (
            AnalyticsReport::Skills { limit: 10 },
            "analytics/daily-skill-usage-metrics",
            json!({"data": []}),
            AnalyticsResponse::Skills(models::DailySkillUsageMetricsResponse::default()),
        ),
        (
            AnalyticsReport::WorkspaceCredits,
            "usage/daily-workspace-user-token-usage-breakdown",
            json!({"data": []}),
            AnalyticsResponse::Usage(models::DailyProductSurfaceUsageResponse::default()),
        ),
        (
            AnalyticsReport::EnterpriseTokens,
            "usage/daily-workspace-user-token-usage-breakdown",
            json!({"data": []}),
            AnalyticsResponse::Usage(models::DailyProductSurfaceUsageResponse::default()),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/api/codex/{route}")))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(body))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let client = Client::new(
            server.uri(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        );
        assert_eq!(
            client
                .get_account_analytics(report, "2026-01-03", "2026-01-09")
                .await
                .unwrap(),
            expected
        );
    }
}
