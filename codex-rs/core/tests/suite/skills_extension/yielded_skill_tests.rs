//! Skill analytics retain the originating turn when Code Mode resumes a yielded cell.

use super::*;
use codex_core::TurnInputSubmission;
use codex_protocol::openai_models::ToolMode;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yielded_skill_read_keeps_originating_turn_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const PACKAGE: &str = "skill://demo/yielded";
    const RESOURCE: &str = "skill://demo/yielded/SKILL.md";
    let server = responses::start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(Arc::new(FakeCloudSkillProvider {
            catalog: SkillCatalog {
                entries: vec![SkillCatalogEntry::new(
                    SkillPackageId(PACKAGE.to_string()),
                    SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                    "demo:yielded",
                    "Instructions for the yielded skill.",
                    SkillResourceId::new(RESOURCE),
                )],
                warnings: Vec::new(),
            },
            resources: std::collections::HashMap::from([(
                RESOURCE.to_string(),
                "Yielded skill instructions.".to_string(),
            )]),
            reads: Mutex::default(),
        })),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let chatgpt_base_url = server.uri();
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model| {
            model.tool_mode = Some(ToolMode::CodeMode);
            model.experimental_supported_tools = vec!["test_sync_tool".to_string()];
        })
        .with_config(move |config| {
            config.chatgpt_base_url = chatgpt_base_url;
            config.cloud_skill_enabled = true;
            config.features.enable(Feature::CodeMode).unwrap();
            config.features.enable(Feature::CodeModeHost).unwrap();
        })
        .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?);
    let test = builder.build_with_auto_env(&server).await?;
    // A cannot read its skill until B has become the active turn.
    let read_after_barrier = format!(
        r#"await tools.test_sync_tool({{barrier: {{
            id: "yielded-skill-turns", participants: 2, timeout_ms: 60000
        }}}});
        text(await tools.skills__read({{package: "{PACKAGE}"}}));"#
    );
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-a"),
            responses::ev_custom_tool_call(
                "call-a",
                "exec",
                &format!("// @exec: {{\"yield_time_ms\": 1}}\n{read_after_barrier}"),
            ),
            ev_completed("resp-a"),
        ]),
    )
    .await;
    let yielded = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-a-done"),
            ev_completed("resp-a-done"),
        ]),
    )
    .await;
    let request = TurnInputRequest::user_input(vec![UserInput::Text {
        text: "Start a skill read and leave it running.".to_string(),
        text_elements: Vec::new(),
    }]);
    let submitted = test.codex.start_or_steer_turn(request).await?;
    let TurnInputSubmission::Started { turn_id: turn_a } = submitted else {
        anyhow::bail!("expected turn A to start, got {submitted:?}");
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let (output, _) = yielded
        .single_request()
        .custom_tool_call_output_content_and_success("call-a")
        .expect("yielded cell output");
    assert!(
        output
            .expect("yielded cell status")
            .starts_with("Script running with cell ID ")
    );

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-b"),
            responses::ev_custom_tool_call(
                "call-b",
                "exec",
                &format!("// @exec: {{\"yield_time_ms\": 60000}}\n{read_after_barrier}"),
            ),
            ev_completed("resp-b"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-b-done"),
            ev_completed("resp-b-done"),
        ]),
    )
    .await;
    let submitted = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Read the skill in this turn and release the previous read.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let TurnInputSubmission::Started { turn_id: turn_b } = submitted else {
        anyhow::bail!("expected turn B to start, got {submitted:?}");
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let events = wait_for_analytics_events(&server, "skill_invocation", /*expected_count*/ 2).await;
    assert_eq!(events.len(), 2);
    for turn_id in [&turn_a, &turn_b] {
        let event = events
            .iter()
            .find(|event| event["event_params"]["turn_id"] == *turn_id)
            .expect("each skill read should retain its originating turn");
        assert_eq!(event["skill_name"], "demo:yielded");
    }
    Ok(())
}
