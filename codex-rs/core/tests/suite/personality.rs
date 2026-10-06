use codex_core::TurnInputRequest;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use test_case::test_case;

const CUSTOM_INSTRUCTIONS: &str = "Custom instructions\n# Personality\nThis must remain\n## Writing Style\nThis must also remain\n# General\nGeneral instructions";

fn read_only_text_turn(
    test: &TestCodex,
    text: &str,
    model: String,
    approval_policy: AskForApproval,
) -> TurnInputRequest {
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), test.cwd_path());
    TurnInputRequest::user_input(vec![UserInput::Text {
        text: text.into(),
        text_elements: Vec::new(),
    }])
    .with_thread_settings(ThreadSettingsOverrides {
        environments: Some(local_selections(test.config.cwd.clone())),
        approval_policy: Some(approval_policy),
        sandbox_policy: Some(sandbox_policy),
        permission_profile,
        collaboration_mode: Some(CollaborationMode {
            mode: ModeKind::Default,
            settings: Settings {
                model,
                reasoning_effort: test.config.model_reasoning_effort.clone(),
                developer_instructions: None,
            },
        }),
        ..Default::default()
    })
}

#[test_case(None, None; "default personality")]
#[test_case(Some("none"), None; "none without feature config")]
#[test_case(Some("none"), Some(false); "none with removed feature disabled")]
#[test_case(Some("none"), Some(true); "none with removed feature enabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_personality_controls_baked_personality_section(
    personality: Option<&'static str>,
    legacy_feature_setting: Option<bool>,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.5", |model_info| {
            if let Some(model_messages) = model_info.model_messages.as_mut() {
                model_messages.instructions_template = Some("Base instructions\n# Personality\nBaked personality\n## Writing Style\nNested writing style\n# General\nGeneral instructions".to_string());
            }
        })
        .with_pre_build_hook(move |home| {
            let mut config = String::new();
            if let Some(value) = personality {
                config.push_str(&format!("personality = \"{value}\"\n"));
            }
            if let Some(value) = legacy_feature_setting {
                config.push_str(&format!("[features]\npersonality = {value}\n"));
            }
            std::fs::write(home.join("config.toml"), config).expect("write personality config");
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .start_or_steer_turn(read_only_text_turn(
            &test,
            "hello",
            test.session_configured.model.clone(),
            test.config.permissions.approval_policy.value(),
        ))
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let expected = if personality == Some("none") {
        "Base instructions\n# General\nGeneral instructions"
    } else {
        "Base instructions\n# Personality\nBaked personality\n## Writing Style\nNested writing style\n# General\nGeneral instructions"
    };
    assert_eq!(resp_mock.single_request().instructions_text(), expected);

    Ok(())
}

#[test_case(CUSTOM_INSTRUCTIONS, true; "custom instructions")]
#[test_case("", false; "bridge barebones")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_personality_none_preserves_explicit_base_instructions(
    custom_instructions: &'static str,
    legacy_feature_setting: bool,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.5", |model_info| {
            if let Some(model_messages) = model_info.model_messages.as_mut() {
                model_messages.instructions_template = Some("Base instructions\n# Personality\nBaked personality\n## Writing Style\nNested writing style\n# General\nGeneral instructions".to_string());
            }
        })
        .with_pre_build_hook(move |home| {
            let config = format!(
                "personality = \"none\"\n[features]\npersonality = {legacy_feature_setting}\n"
            );
            std::fs::write(home.join("config.toml"), config).expect("write personality config");
        })
        .with_config(move |config| {
            config.base_instructions = Some(custom_instructions.to_string());
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .start_or_steer_turn(read_only_text_turn(
            &test,
            "hello",
            test.session_configured.model.clone(),
            test.config.permissions.approval_policy.value(),
        ))
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let body = request.body_json();
    assert!(body.get("instructions").is_none());
    if !custom_instructions.is_empty() {
        assert_eq!(request.instructions_text(), custom_instructions);
    }
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .all(|text| !text.is_empty())
    );
    assert!(!request.body_contains_text("Base instructions"));

    Ok(())
}
