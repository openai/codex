//! Mock HTTP reports for tests that exercise the authenticated dashboard loading path.
use super::data::Load;
use serde_json::json;

pub(super) async fn server() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path_regex("^/backend-api/wham/(usage|analytics)/"))
        .respond_with(|request: &wiremock::Request| {
            let params = request.url.query_pairs().collect::<std::collections::HashMap<_, _>>();
            let today = chrono::Utc::now().date_naive();
            let start = params.get("start_date").map_or(today, |date| date.parse().unwrap());
            let end = params.get("end_date").map_or(today, |date| date.parse().unwrap());
            let route = request.url.path().rsplit('/').next().unwrap();
            let data = start.iter_days().take_while(|date| *date <= end).map(|date| {
                let date = date.to_string();
                match route {
                    "daily-token-usage-breakdown" | "daily-workspace-user-token-usage-breakdown" => json!({
                        "date":date, "product_surface_usage_values":{"codex_cli":10.0},
                        "premium_usage_values":{
                            "credit_usage_credits":{"codex_cli":10.0}, "total_usage_credits":{},
                            "uncached_text_input_tokens_by_surface":{}, "cached_text_input_tokens_by_surface":{},
                            "text_output_tokens_by_surface":{}, "text_total_tokens_by_surface":{}
                        },
                        "models":[{"model":"GPT-5.5","credits":10.0,"on_demand_credits":10.0,"turns":10,"speed":"standard"}],
                        "groups":[{"dimensions":{"model":"GPT-5.5"},"credits":10.0,"uncached_text_input_tokens":10,"cached_text_input_tokens":20,"text_output_tokens":30}]
                    }),
                    "credit-usage-events" => json!({"date":date,"product_surface":"cli","credit_amount":10.0}),
                    "daily-workspace-user-credit-usage" => json!({"date":date,"values":{"codex":10.0}}),
                    "daily-workspace-usage-counts" => json!({
                        "date":date,"totals":{"users":1,"threads":1,"turns":10,"credits":10.0},"clients":[],
                        "models":[{"model":"GPT-5.5","credits":10.0,"turns":10}]
                    }),
                    "daily-plugin-usage-metrics" => json!({"date":date,"plugin_usage_overviews":[{
                        "plugin_id":null,"plugin_name":"test","marketplace":null,"display_name":"Test plugin","invocation_counts":10
                    }]}),
                    "daily-skill-usage-metrics" => json!({"date":date,"skill_usage_overviews":[{
                        "skill_name":"test","skill_ids":[],"display_name":"Test skill","invocation_counts":10
                    }]}),
                    _ => panic!("Unexpected mock analytics route: {route}"),
                }
            }).collect::<Vec<_>>();
            let response = if route == "daily-workspace-user-credit-usage" {
                json!({"breakdown":params["breakdown"],"series":[{"key":"codex","label":"Codex","total":data.len() as f64 * 10.0}],"data":data})
            } else {
                json!({"data":data})
            };
            wiremock::ResponseTemplate::new(/*s*/ 200).set_body_json(response)
        })
        .mount(&server).await;
    server
}

pub(super) async fn settle(view: &mut super::AnalyticsView) {
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 10), async {
        loop {
            view.poll_reports();
            if !matches!(view.account, Load::Loading(_))
                && !view
                    .sections
                    .0
                    .iter()
                    .any(|state| matches!(state.history, Load::Loading(_)))
                && !matches!(view.chats, Load::Loading(_))
                && !matches!(view.tasks, Load::Loading(_))
                && !matches!(view.profile, Load::Loading(_))
                && !matches!(view.plan.report, Load::Loading(_))
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 5)).await;
        }
    })
    .await
    .unwrap();
}
