//! Fresh catalogs repair saved settings before sending while preserving retained history.

use anyhow::Result;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::MockServer;

fn catalog(efforts: &[ReasoningEffort], tiers: &[&str]) -> Result<ModelsResponse> {
    let mut model = bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.5")
        .expect("bundled model");
    model.supported_reasoning_levels = efforts
        .iter()
        .map(|effort| ReasoningEffortPreset {
            effort: effort.clone(),
            description: effort.to_string(),
        })
        .collect();
    model.default_reasoning_level = efforts.first().cloned();
    model.supports_reasoning_effort_updates = true;
    model.service_tiers = tiers
        .iter()
        .map(|tier| ModelServiceTier {
            id: (*tier).into(),
            name: (*tier).into(),
            description: String::new(),
        })
        .collect();
    model.additional_speed_tiers.clear();
    model.default_service_tier = tiers.first().map(|tier| (*tier).into());
    Ok(ModelsResponse {
        models: vec![model],
    })
}

async fn run_turn(test: &TestCodex, text: &str, settings: ThreadSettingsOverrides) -> Result<()> {
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: text.into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(settings),
        )
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(matches!(event, EventMsg::TurnComplete(event) if event.error.is_none()));
    Ok(())
}

async fn start_test(server: &MockServer, effort: ReasoningEffort, tier: &str) -> Result<TestCodex> {
    responses::mount_models_once(
        server,
        catalog(
            &[
                ReasoningEffort::High,
                ReasoningEffort::Max,
                ReasoningEffort::Low,
            ],
            &["priority", "ultrafast"],
        )?,
    )
    .await;
    let tier = tier.to_string();
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .with_config(move |config| {
            config.model_reasoning_effort = Some(effort);
            config.service_tier = Some(tier);
            for feature in [
                Feature::FastMode,
                Feature::UltrafastMode,
                Feature::ReasoningEffortOverride,
            ] {
                config
                    .features
                    .set_enabled(feature, /*enabled*/ true)
                    .expect("enable model settings");
            }
        })
        .build_with_auto_env(server)
        .await
}

async fn select_effort(test: &TestCodex, effort: ReasoningEffort) -> Result<()> {
    let (reply, applied) = tokio::sync::oneshot::channel();
    test.codex
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                effort: Some(Some(effort)),
                ..Default::default()
            },
            reply: Some(reply),
        })
        .await?;
    applied.await??;
    Ok(())
}

#[test_case::test_case("both")]
#[test_case::test_case("valid")]
#[test_case::test_case("reasoning-only")]
#[test_case::test_case("pin-only")]
#[test_case::test_case("standard")]
#[test_case::test_case("queued-stale-settings")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_refresh_updates_existing_chat_before_next_send(scenario: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let valid = scenario == "valid";
    let initial_effort = if valid {
        ReasoningEffort::Low
    } else {
        ReasoningEffort::High
    };
    let initial_tier = if valid || matches!(scenario, "reasoning-only" | "pin-only") {
        "priority"
    } else {
        "ultrafast"
    };
    let test = start_test(&server, initial_effort.clone(), initial_tier).await?;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse_completed("before-refresh"),
            responses::sse_completed("after-refresh"),
        ],
    )
    .await;
    run_turn(
        &test,
        "establish the original effort",
        ThreadSettingsOverrides::default(),
    )
    .await?;
    if scenario == "pin-only" {
        select_effort(&test, ReasoningEffort::Low).await?;
    }
    let before = test.codex.thread_settings_snapshot().await;
    let tiers = if scenario == "standard" {
        vec![]
    } else {
        vec!["priority"]
    };
    let refresh =
        responses::mount_models_once(&server, catalog(&[ReasoningEffort::Low], &tiers)?).await;
    test.thread_manager
        .list_models(RefreshStrategy::Online, test.config.http_client_factory())
        .await;
    assert_eq!(refresh.requests().len(), 1);
    assert_eq!(
        requests.requests().len(),
        1,
        "catalog refresh must not send a message"
    );
    let mut expected = before;
    expected.reasoning_effort = Some(ReasoningEffort::Low);
    expected.collaboration_mode.settings.reasoning_effort = Some(ReasoningEffort::Low);
    expected.service_tier = Some(
        if initial_tier == "priority" {
            "priority"
        } else {
            "default"
        }
        .into(),
    );
    if scenario != "queued-stale-settings" && scenario != "pin-only" && !valid {
        wait_for_event(&test.codex, |event| {
            matches!(event,
                EventMsg::ThreadSettingsApplied(applied) if applied.thread_settings == expected
            )
        })
        .await;
        assert_eq!(test.codex.thread_settings_snapshot().await, expected);
    }
    let settings = if scenario == "queued-stale-settings" {
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            service_tier: Some(Some("ultrafast".into())),
            ..Default::default()
        }
    } else {
        ThreadSettingsOverrides::default()
    };
    run_turn(&test, "next message", settings).await?;
    assert_eq!(test.codex.thread_settings_snapshot().await, expected);
    let requests = requests.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].body_json()["reasoning"]["effort"],
        json!(initial_effort)
    );
    assert_eq!(requests[1].body_json()["model"], json!("gpt-5.5"));
    assert_eq!(requests[1].body_json()["reasoning"]["effort"], json!("low"));
    assert_eq!(
        requests[1].body_json()["service_tier"],
        json!((initial_tier == "priority").then_some("priority"))
    );
    assert_eq!(
        &requests[1].input()[..requests[0].input().len()],
        requests[0].input()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reopening_consumes_catalog_refreshed_while_chat_was_closed() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let test = start_test(&server, ReasoningEffort::High, "ultrafast").await?;
    responses::mount_sse_once(&server, responses::sse_completed("before-close")).await;
    run_turn(&test, "save this chat", ThreadSettingsOverrides::default()).await?;
    let mut expected = test.codex.thread_settings_snapshot().await;
    test.codex.shutdown_and_wait().await?;
    let manager = test.thread_manager.get_models_manager();
    responses::mount_models_once(&server, catalog(&[ReasoningEffort::Low], &["priority"])?).await;
    manager
        .list_models(RefreshStrategy::Online, test.config.http_client_factory())
        .await;
    let resumed = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .with_models_manager(manager)
        .with_config(|config| {
            config.model_reasoning_effort = Some(ReasoningEffort::High);
            config.service_tier = Some("ultrafast".into());
            for feature in [
                Feature::FastMode,
                Feature::UltrafastMode,
                Feature::ReasoningEffortOverride,
            ] {
                config.features.enable(feature).unwrap();
            }
        })
        .restart_with_auto_env(&server, &test)
        .await?;
    // The restart fixture creates a new runtime directory.
    expected.cwd = resumed.session_configured.cwd.clone();
    expected.reasoning_effort = Some(ReasoningEffort::Low);
    expected.collaboration_mode.settings.reasoning_effort = Some(ReasoningEffort::Low);
    expected.service_tier = Some("default".into());
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::ThreadSettingsApplied(_))
    })
    .await;
    assert_eq!(resumed.codex.thread_settings_snapshot().await, expected);
    let request =
        responses::mount_sse_once(&server, responses::sse_completed("after-reopen")).await;
    run_turn(&resumed, "continue", ThreadSettingsOverrides::default()).await?;
    assert_eq!(
        request.single_request().body_json()["reasoning"]["effort"],
        json!("low")
    );
    assert_eq!(
        request.single_request().body_json()["service_tier"],
        json!(null)
    );
    Ok(())
}
