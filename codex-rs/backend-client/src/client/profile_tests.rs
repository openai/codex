//! Full and partial profile decoding through the authenticated HTTP client.
use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn profile_keeps_missing_zero_and_future_invocations_distinct() {
    let server = MockServer::start().await;
    let client = Client::new(
        server.uri(),
        codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    );
    let empty = AccountProfile {
        profile: None,
        metadata: None,
        stats: ProfileStats {
            tokens: TokenUsageProfileStats {
                lifetime_tokens: None,
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
                daily_usage_buckets: None,
            },
            fast_mode_usage_percentage: None,
            most_used_reasoning_effort: None,
            most_used_reasoning_effort_percentage: None,
            unique_skills_used: None,
            total_skills_used: None,
            total_threads: None,
            top_invocations: None,
        },
    };
    let full = AccountProfile {
        profile: Some(ProfileIdentity {
            display_name: Some("Test user".into()),
            username: Some("test.user".into()),
        }),
        metadata: Some(ProfileMetadata {
            stats_as_of: Some("2026-09-09".into()),
            stats_error: Some("partial upstream data".into()),
        }),
        stats: ProfileStats {
            tokens: TokenUsageProfileStats {
                lifetime_tokens: Some(142_300_000_000),
                peak_daily_tokens: Some(7_000_000_000),
                longest_running_turn_sec: Some(158_760),
                current_streak_days: Some(158),
                longest_streak_days: Some(160),
                daily_usage_buckets: Some(vec![crate::TokenUsageProfileDailyBucket {
                    start_date: "2026-09-09".into(),
                    tokens: 42,
                }]),
            },
            fast_mode_usage_percentage: serde_json::Number::from_f64(/*f*/ 37.5),
            most_used_reasoning_effort: Some("high".into()),
            most_used_reasoning_effort_percentage: Some(26.into()),
            unique_skills_used: Some(147),
            total_skills_used: Some(13_190),
            total_threads: Some(0),
            top_invocations: Some(vec![
                ProfileInvocation {
                    kind: ProfileInvocationKind::Unknown,
                    plugin_name: None,
                    skill_name: None,
                    usage_count: None,
                },
                ProfileInvocation {
                    kind: ProfileInvocationKind::Skill,
                    plugin_name: None,
                    skill_name: Some("review".into()),
                    usage_count: Some(12),
                },
                ProfileInvocation {
                    kind: ProfileInvocationKind::Plugin,
                    plugin_name: Some("github".into()),
                    skill_name: None,
                    usage_count: Some(34),
                },
            ]),
        },
    };
    for (body, expected) in [
        (json!({"stats":{}}), empty),
        (
            json!({
                "profile":{"display_name":"Test user","username":"test.user"},
                "metadata":{"stats_as_of":"2026-09-09","stats_error":"partial upstream data"},
                "stats":{
                    "lifetime_tokens":142_300_000_000_i64,"peak_daily_tokens":7_000_000_000_i64,
                    "longest_running_turn_sec":158_760,"current_streak_days":158,"longest_streak_days":160,
                    "daily_usage_buckets":[{"start_date":"2026-09-09","tokens":42}],
                    "fast_mode_usage_percentage":37.5,"most_used_reasoning_effort":"high",
                    "most_used_reasoning_effort_percentage":26,"unique_skills_used":147,
                    "total_skills_used":13_190,"total_threads":0,
                    "top_invocations":[{"type":"future_kind"},{"type":"skill","skill_name":"review","usage_count":12},
                        {"type":"plugin","plugin_name":"github","usage_count":34}]
                }
            }),
            full,
        ),
    ] {
        Mock::given(method("GET"))
            .and(path("/api/codex/profiles/me"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(body))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert_eq!(client.get_account_profile().await.unwrap(), expected);
        server.verify().await;
        server.reset().await;
    }
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_string("private invalid payload"))
        .mount(&server)
        .await;
    let error = client.get_account_profile().await.unwrap_err().to_string();
    assert_eq!(error, "Invalid account profile response.");
}
