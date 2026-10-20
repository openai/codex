//! Frozen cloud skills remain available alongside a named executor and use native skill tools.

use super::*;
use codex_skills_extension::FrozenCloudSkillProvider;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frozen_cloud_skills_with_named_executor_request_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let workspace = tempfile::tempdir()?;
    let path = workspace.path().join("skills.json");
    fs::write(
        &path,
        json!({"skills": [{
            "package": "skill://demo/work", "name": "demo:work",
            "description": "Inspect and explain the requested work.",
            "alias_root": "skill://demo", "main_resource": "skill://demo/work/SKILL.md",
            "resources": {"skill://demo/work/SKILL.md": "# Work\nInspect the inputs before explaining the result.\n"}
        }]})
        .to_string(),
    )?;
    let provider = FrozenCloudSkillProvider::from_file(&path)?;
    fs::remove_file(path)?;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new()
            .with_cloud_provider(Arc::new(provider))
            .with_executor_provider(Arc::new(
                ExecutorSkillProvider::new_with_restriction_product(
                    Arc::new(EnvironmentManager::default_for_tests()),
                    /*restriction_product*/ None,
                ),
            )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: true,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call_with_namespace(
                    "list-skills",
                    "skills",
                    "list",
                    r#"{"authority":{"kind":"cloud"}}"#,
                ),
                ev_completed("listed"),
            ]),
            sse(vec![
                ev_function_call_with_namespace(
                    "read-work",
                    "skills",
                    "read",
                    r#"{"package":"c0/work","source":{"kind":"cloud"}}"#,
                ),
                ev_completed("read"),
            ]),
            sse(vec![
                ev_assistant_message("answer", "I read the frozen work instructions."),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        // Cloud parents start without the reserved local executor attachment.
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.cloud_skill_enabled = true;
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscoveryV2)
                .unwrap();
        })
        .build(&server)
        .await?;
    let executor = core_test_support::exec_server::ExecServerProcess::start().await?;
    test.thread_manager
        .environment_manager()
        .upsert_environment(
            "work-env".into(),
            executor.websocket_url.clone(),
            /*connect_timeout*/ None,
        )?;
    let cwd = PathUri::from_host_native_path(fs::canonicalize(workspace.path())?)?;
    test.submit_turn_with_environments(
        "List the cloud skills, then read the work skill.",
        Some(vec![TurnEnvironmentSelection {
            environment_id: "work-env".into(),
            cwd: cwd.clone(),
            workspace_roots: vec![cwd],
            selected_capability_roots: Default::default(),
            config: EnvironmentConfigState::FromThread,
        }]),
    )
    .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    let output: serde_json::Value = serde_json::from_str(
        &mock
            .function_call_output_text("read-work")
            .expect("frozen skill read"),
    )?;
    assert_eq!(
        output,
        json!({
            "resource": "skill://demo/work/SKILL.md",
            "contents": "# Work\nInspect the inputs before explaining the result.\n",
            "installations": [{
                "package": "skill://demo/work", "resource": "skill://demo/work/SKILL.md",
            }],
            "next_cursor": null,
        })
    );
    let mut bodies = requests
        .iter()
        .map(core_test_support::responses::ResponsesRequest::body_json)
        .collect::<Vec<_>>();
    // Cargo and Bazel can serialize embedded JSON in different key orders.
    // Preserve every output value while normalizing before truncation and hashing.
    for body in &mut bodies {
        for item in body["input"].as_array_mut().expect("request input") {
            if item["type"] == "function_call_output"
                && let Some(serde_json::Value::String(output)) = item.get_mut("output")
            {
                *output = context_snapshot::normalize_json_lines(output);
            }
        }
    }
    insta::assert_snapshot!(
        "frozen_cloud_skills_with_named_executor_request_history",
        context_snapshot::format_context_snapshot(
            "A cloud parent lists and reads a frozen skill alongside a named executor after the snapshot file has been removed.",
            &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
