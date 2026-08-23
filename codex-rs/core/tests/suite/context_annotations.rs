use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::CurrentTimeReminderConfig;
use codex_core::config::RolloutBudgetConfig;
use codex_core::config::TokenBudgetConfig;
use codex_features::Feature;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use std::collections::BTreeMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_request_item_types_roles_and_content_annotations() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.input_modalities.push(InputModality::Audio);
        })
        .with_config(|config| {
            config.developer_instructions = Some("Keep world-state annotations aligned.".into());
            config.model_context_window = Some(128_000);
            config.current_time_reminder = Some(CurrentTimeReminderConfig::default());
            config.token_budget = Some(TokenBudgetConfig {
                guidance_message: Some("Preserve important context.".into()),
                ..TokenBudgetConfig::default()
            });
            config.rollout_budget = Some(RolloutBudgetConfig {
                limit_tokens: 100,
                reminder_at_remaining_tokens: Vec::new(),
                sampling_token_weight: 1.0,
                prefill_token_weight: 1.0,
            });
            config.multi_agent_v2.root_agent_usage_hint_text =
                Some("Coordinate available subagents.".into());
            config.multi_agent_v2.multi_agent_mode_hint_text =
                Some("Delegate independent work.".into());
            for feature in [
                Feature::CurrentTimeReminder,
                Feature::DeferredExecutor,
                Feature::MultiAgentV2,
                Feature::TokenBudget,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("test config should allow feature update");
            }
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![
                UserInput::Text {
                    text: "inspect world state".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Image {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: None,
                },
                UserInput::Audio {
                    audio_url: "data:audio/wav;base64,AAAA".to_string(),
                },
            ])
            .with_additional_context(BTreeMap::from([
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry {
                        value: "tab one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: "run one".to_string(),
                        kind: AdditionalContextKind::Application,
                    },
                ),
            ])),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let items = response
        .single_request()
        .input()
        .into_iter()
        .map(|item| {
            let item_type = item["type"].as_str().expect("response item type");
            let role = item["role"].as_str().unwrap_or("-");
            let content_annotations =
                &item["internal_chat_message_metadata_passthrough"]["content_item_kinds"];
            format!("{item_type} {role} {content_annotations}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(items, @r#"
    message developer ["generic.developer_instructions","token_budget.context_window_guidance","generic.permissions_instructions","environments.instructions"]
    message developer ["token_budget.context_window"]
    message developer ["multi_agent.usage_hint"]
    message developer ["multi_agent.mode_instructions"]
    message user ["environments.environment_context"]
    message developer ["additional_content.automation_info"]
    message user ["additional_content.browser_info"]
    message user ["user.text","user.image","user.audio"]
    message developer ["rollout_budget.remaining_tokens"]
    message developer ["current_time.reminder"]
    "#);

    Ok(())
}
