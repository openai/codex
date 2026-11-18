#![cfg(test)]
//! Checks review counter attribution from admitted main and subagent turns.

use super::*;
use codex_core::TurnStartOptions;
use codex_otel::GUARDIAN_REVIEW_COUNT_METRIC;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_metrics_distinguish_automation_main_and_subagent_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::User;
        })
        .build_with_auto_env(&server)
        .await?;
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
    )?;
    let mut expected = BTreeMap::new();
    for source in [
        SessionSource::Custom("codex-cloud-agent".into()),
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: test.session_configured.thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
    ] {
        let mut options = test.start_thread_options().await;
        options.session_source = Some(source.clone());
        options.thread_extension_init.insert(metrics.clone());
        let thread = test.thread_manager.start_thread(options).await?.thread;
        // A later user turn on the same session must not retain automation attribution.
        for (trigger, origin) in [
            ("automation_cron_scheduled", "automation"),
            ("composer", "other"),
        ] {
            let responses = mount_sse_sequence(
                &server,
                vec![
                    sse(vec![
                        ev_response_created("parent-tool"),
                        ev_function_call(
                            "reviewed-command",
                            "exec_command",
                            r#"{"cmd":"true","sandbox_permissions":"require_escalated","justification":"Exercise review attribution."}"#,
                        ),
                        ev_completed("parent-tool"),
                    ]),
                    sse(vec![
                        ev_response_created("guardian-verdict"),
                        ev_assistant_message(
                            "guardian",
                            r#"{"risk_level":"high","user_authorization":"low","outcome":"deny","rationale":"Test denial."}"#,
                        ),
                        ev_completed("guardian-verdict"),
                    ]),
                    sse(vec![ev_completed("parent-complete")]),
                ],
            )
            .await;
            thread
                .start_or_steer_turn(
                    TurnInputRequest::user_input(vec![UserInput::Text {
                        text: "Run the reviewed command.".into(),
                        text_elements: Vec::new(),
                    }])
                    .on_start(TurnStartOptions {
                        turn_trigger: Some(trigger.into()),
                        ..Default::default()
                    })
                    .with_thread_settings(ThreadSettingsOverrides {
                        approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                        ..Default::default()
                    }),
                )
                .await?;
            wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
            assert_eq!(responses.requests().len(), 3);
            expected.insert(
                (source.to_string(), origin.to_string(), "deny".to_string()),
                1,
            );
        }
        thread.shutdown_and_wait().await?;
    }
    let snapshot = metrics.snapshot()?;
    let metric = snapshot
        .scope_metrics()
        .flat_map(ScopeMetrics::metrics)
        .find(|metric| metric.name() == GUARDIAN_REVIEW_COUNT_METRIC)
        .context("Guardian review counter")?;
    let AggregatedMetrics::U64(MetricData::Sum(counter)) = metric.data() else {
        panic!("expected review counter");
    };
    let observed = counter
        .data_points()
        .map(|point| {
            let tags = point
                .attributes()
                .map(|tag| (tag.key.as_str(), tag.value.as_str().to_string()))
                .collect::<BTreeMap<_, _>>();
            (
                (
                    tags["session_source"].clone(),
                    tags["turn_origin"].clone(),
                    tags["outcome"].clone(),
                ),
                point.value(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observed, expected);
    Ok(())
}
