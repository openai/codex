//! Custom provider overrides constrain web access and enable V2 compaction.

use super::*;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_provider_capabilities_constrain_web_access_and_enable_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    fs::write(
        home.path().join("config.toml"),
        r#"
model_provider = "custom"
web_search = "cached"
[features]
standalone_web_search = false
[features.multi_agent_v2]
enabled = true
tool_namespace = "collaboration"
expose_spawn_agent_model_overrides = false
[model_providers.custom]
name = "Custom"
requires_openai_auth = true
[model_providers.custom.capabilities]
external_web_access = false
remote_compaction = "v2"
"#,
    )?;
    let test = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            configure_scenario_catalog(config);
            let base_url = config.model_provider.base_url.clone();
            // Restore the configured provider after the test builder installs its default.
            config.model_provider = config.model_providers["custom"].clone();
            config.model_provider.base_url = base_url;
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.use_responses_lite = false;
            model.tool_mode = Some(ToolMode::Direct);
            model.apply_patch_tool_type = None;
        })
        .build_with_auto_env(&server)
        .await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("response")]),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {"type": "compaction", "encrypted_content": "CUSTOM_SUMMARY"},
                }),
                ev_completed("compact-response"),
            ]),
        ],
    )
    .await;
    test.submit_turn_with_permission_profile(
        "Check the configuration.",
        PermissionProfile::Disabled,
    )
    .await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
    let body = requests[0].body_json();
    let tools = body["tools"].as_array().expect("request tools");
    assert!(
        requests[0]
            .tool_by_name("collaboration", "list_agents")
            .is_some()
    );
    let web_search = tools
        .iter()
        .find(|tool| tool["type"] == "web_search")
        .expect("cached search");
    assert_eq!(web_search["external_web_access"], false);
    insta::assert_snapshot!(
        "custom_provider_capabilities",
        context_snapshot::format_request_history_snapshot(
            "Custom provider overrides keep unrestricted search cached and enable V2 compaction.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}
