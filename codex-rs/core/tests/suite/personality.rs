use codex_config::types::Personality;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
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

const LOCAL_FRIENDLY_TEMPLATE: &str =
    "You optimize for team morale and being a supportive teammate as much as code quality.";
const BUNDLED_FRIENDLY_TEMPLATE: &str = "You have a vivid inner life as Codex:";
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_personality_none_sends_no_personality() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex().with_model("gpt-5.5").with_config(|config| {
        config
            .features
            .enable(Feature::Personality)
            .expect("test config should allow feature update");
        config.personality = Some(Personality::None);
    });
    let test = builder.build(&server).await?;

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
    let instructions_text = request.instructions_text();
    assert!(
        !instructions_text.contains(BUNDLED_FRIENDLY_TEMPLATE),
        "expected no friendly personality template, got: {instructions_text:?}"
    );
    assert!(!instructions_text.contains("# Personality"));
    assert!(
        !instructions_text.contains("{{ personality }}"),
        "expected personality placeholder to be removed, got: {instructions_text:?}"
    );

    let developer_texts = request.message_input_texts("developer");
    assert!(
        !developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "did not expect a personality update message when personality is None"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_personality_none_strips_baked_personality_section() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.5", |model_info| {
            if let Some(model_messages) = model_info.model_messages.as_mut() {
                model_messages.instructions_template = Some("Base instructions\n# Personality\nBaked personality\n## Writing Style\nNested writing style\n# General\nGeneral instructions".to_string());
                model_messages.instructions_variables = None;
            }
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::Personality)
                .expect("test config should allow feature update");
            config.personality = Some(Personality::None);
        });
    let test = builder.build(&server).await?;

    test.codex
        .start_or_steer_turn(read_only_text_turn(
            &test,
            "hello",
            test.session_configured.model.clone(),
            test.config.permissions.approval_policy.value(),
        ))
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(
        resp_mock.single_request().instructions_text(),
        "Base instructions\n# General\nGeneral instructions"
    );

    Ok(())
}

#[test_case(CUSTOM_INSTRUCTIONS, true; "custom instructions")]
#[test_case("", false; "bridge barebones")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_personality_none_preserves_explicit_base_instructions(
    custom_instructions: &'static str,
    personality_enabled: bool,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model("gpt-5.5")
        .with_config(move |config| {
            config
                .features
                .set_enabled(Feature::Personality, personality_enabled)
                .expect("test config should allow feature update");
            config.personality = Some(Personality::None);
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
    // Responses requests omit the instructions field for an explicit empty override.
    let expected_instructions = (!custom_instructions.is_empty())
        .then(|| serde_json::Value::String(custom_instructions.to_string()));
    assert_eq!(body.get("instructions"), expected_instructions.as_ref());
    assert!(!request.body_contains_text(BUNDLED_FRIENDLY_TEMPLATE));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_instructions_are_friendly_without_config_toml() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex().with_model("gpt-5.5").with_config(|config| {
        config
            .features
            .enable(Feature::Personality)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    assert_eq!(test.config.personality, None);

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
    let instructions_text = request.instructions_text();
    assert!(
        instructions_text.contains(BUNDLED_FRIENDLY_TEMPLATE),
        "expected default friendly template, got: {instructions_text:?}"
    );
    assert!(!request.body_contains_text("<personality_spec>"));

    Ok(())
}

#[test_case("gpt-5.4", LOCAL_FRIENDLY_TEMPLATE; "gpt_5_4")]
#[test_case("gpt-5.5", BUNDLED_FRIENDLY_TEMPLATE; "gpt_5_5")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fixed_friendly_personality_ignores_pragmatic_update(
    model: &str,
    friendly_template: &str,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let mut builder = test_codex().with_model(model).with_config(|config| {
        config
            .features
            .enable(Feature::Personality)
            .expect("test config should allow feature update");
        config.personality = Some(Personality::Friendly);
    });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("first turn").await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            personality: Some(Personality::Pragmatic),
            ..Default::default()
        },
    )
    .await?;
    test.submit_turn("continue with the legacy pragmatic setting")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].instructions_text().contains(friendly_template));
    assert_eq!(
        requests[1].instructions_text(),
        requests[0].instructions_text()
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.body_contains_text("<personality_spec>"))
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_personality_session_resumes_and_completes() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let legacy_instructions = "Legacy model instructions\n# Personality\nBe pragmatic.";
    let mut builder = test_codex()
        .with_model("gpt-5.5")
        .with_config(move |config| {
            config.personality = Some(Personality::Pragmatic);
            config.base_instructions = Some(legacy_instructions.to_string());
            config.base_instructions_provenance = Some(BaseInstructionsProvenance::Model {
                model: "gpt-5.5".to_string(),
            });
        });
    let original = builder.build_with_auto_env(&server).await?;
    original.submit_turn("first turn").await?;

    let mut builder = test_codex().with_model("gpt-5.5").with_config(|config| {
        config.personality = Some(Personality::None);
    });
    let resumed = builder.restart(&server, &original).await?;
    assert_eq!(
        resumed.session_configured.thread_id,
        original.session_configured.thread_id
    );
    resumed.submit_turn("continue the existing session").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].instructions_text(), legacy_instructions);
    assert!(
        requests[1]
            .message_input_texts("user")
            .iter()
            .any(|text| text == "first turn")
    );
    Ok(())
}
