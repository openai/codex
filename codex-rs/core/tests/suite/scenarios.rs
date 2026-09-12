//! Multi-turn Astra scenarios snapshot the model-visible request history of shipped features.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::types::McpServerConfig;
use codex_context_fragments::AnsweredQuestion;
use codex_context_fragments::ContextualUserFragment;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::items::AgentMessageDelivery;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::stdio_server_bin;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::executor_path_uri;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::oneshot;

const ONE_PIXEL_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

fn skills_extensions() -> Arc<ExtensionRegistry<Config>> {
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install(&mut extensions, |config: &Config| SkillsExtensionConfig {
        include_instructions: config.include_skill_instructions,
        max_context_tokens: config.skill_max_context_tokens,
        bundled_skills_enabled: config.bundled_skills_enabled(),
        orchestrator_skills_enabled: config.orchestrator_skills_enabled,
        shadow_selection_enabled: config.features.enabled(Feature::SkillSearch),
    });
    Arc::new(extensions.build())
}

fn write_skill(path: &Path, name: &str, description: &str, body: &str) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    let skill = path.join("SKILL.md");
    fs::write(
        &skill,
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n"),
    )?;
    Ok(fs::canonicalize(skill)?)
}

struct ScenarioSkills {
    outline: PathBuf,
    agenda: PathBuf,
    summarize: PathBuf,
    final_check: PathBuf,
}

fn write_scenario_capabilities(home: &TempDir) -> Result<ScenarioSkills> {
    fs::write(
        home.path().join("config.toml"),
        "[features]\nplugins = true\n\n[skills.bundled]\nenabled = false\n\n[plugins.\"calendar@test\"]\nenabled = true\n\n[plugins.\"notes@test\"]\nenabled = true\n",
    )?;
    let plugin_cache = home.path().join("plugins/cache/test");
    for (name, description) in [
        ("calendar", "Prepare a team schedule"),
        ("notes", "Summarize meeting notes"),
    ] {
        let manifest = plugin_cache
            .join(name)
            .join("local/.codex-plugin/plugin.json");
        fs::create_dir_all(manifest.parent().expect("manifest parent"))?;
        fs::write(
            manifest,
            json!({ "name": name, "description": description }).to_string(),
        )?;
    }

    Ok(ScenarioSkills {
        outline: write_skill(
            &home.path().join("skills/outline"),
            "outline",
            "Draft a project outline",
            "List the goals and owners.",
        )?,
        agenda: write_skill(
            &plugin_cache.join("calendar/local/skills/agenda"),
            "agenda",
            "Plan a team agenda",
            "List meetings with dates and attendees.",
        )?,
        summarize: write_skill(
            &plugin_cache.join("notes/local/skills/summarize"),
            "summarize",
            "Summarize team notes",
            "Extract decisions and action items.",
        )?,
        final_check: write_skill(
            &home.path().join("skills/final-check"),
            "final-check",
            "Review a final brief",
            "Check that the brief has an owner for every action.",
        )?,
    })
}

fn text(value: &str) -> UserInput {
    UserInput::Text {
        text: value.to_string(),
        text_elements: Vec::new(),
    }
}

fn selected_skill(name: &str, path: &Path) -> UserInput {
    UserInput::Skill {
        name: name.to_string(),
        path: path.to_path_buf(),
    }
}

fn plugin(name: &str) -> UserInput {
    UserInput::Mention {
        name: name.to_string(),
        path: format!("plugin://{name}@test"),
    }
}

fn configure_scenario_catalog(config: &mut Config) {
    // Keep the fixture independent of the checkout's project configuration.
    let stack = &config.config_layer_stack;
    config.config_layer_stack = ConfigLayerStack::new(
        stack
            .all_layers_low_to_high()
            .filter(|layer| !matches!(&layer.name, ConfigLayerSource::Project { .. }))
            .cloned()
            .collect(),
        stack.requirements().clone(),
        stack.requirements_toml().clone(),
    )
    .expect("fixture config layers");
    config.model_catalog = Some(bundled_models_response().expect("bundled model catalog"));
    config.orchestrator_skills_enabled = false;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn astra_asks_an_async_question_and_receives_the_answer_while_working() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let question = "Who should receive the launch update?";
    let (release_continuation, continuation_gate) = oneshot::channel();
    let mut working_message = ev_assistant_message("working", "I drafted a short launch update.");
    working_message["item"]["phase"] = json!("commentary");
    let (streaming, _completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created("question-response"),
                ev_function_call_with_namespace(
                    "audience-question",
                    "functions",
                    "request_user_input_async",
                    &json!({"questions": [{"title": question, "options": ["Internal team", "Customers"]}]}).to_string(),
                ),
                ev_completed("question-response"),
            ]),
        }],
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![ev_response_created("working-response"), working_message]),
            },
            StreamingSseChunk {
                gate: Some(continuation_gate),
                body: sse(vec![ev_completed("working-response")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created("answered-response"),
                ev_assistant_message("answered", "Here is the launch update for customers."),
                ev_completed("answered-response"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created("follow-up-response"),
                ev_assistant_message("follow-up", "The email subject is: Launch update."),
                ev_completed("follow-up-response"),
            ]),
        }],
    ])
    .await;
    let config_server = start_mock_server().await;
    let base_url = format!("{}/v1", streaming.uri());
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.model_provider.base_url = Some(base_url);
            // The gated mock records raw request bodies for the shared snapshot renderer.
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("disable compression for the gated mock");
        })
        .build_with_auto_env(&config_server)
        .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
            "Draft a short launch update. Ask me who it is for and keep working while I answer.",
        )]))
        .await?;
    let turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ItemCompleted(event)
            if matches!(&event.item, TurnItem::AgentMessage(message)
                if message.delivery == Some(AgentMessageDelivery::Async)))
    })
    .await;
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 10),
        streaming.wait_for_request_count(/*count*/ 2),
    )
    .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ItemCompleted(event)
            if matches!(&event.item, TurnItem::AgentMessage(message) if message.id == "working"))
    })
    .await;

    let answer = format!("{}Customers", AnsweredQuestion::new(question).render());
    test.codex
        .steer_turn(TurnInputRequest::user_input(vec![text(&answer)]), turn_id)
        .await?;
    release_continuation.send(()).expect("release continuation");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("Now give it an email subject.").await?;

    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(
        "astra_async_question_and_answer",
        context_snapshot::format_context_snapshot(
            "Astra asks who a launch update is for, keeps working, and receives the user's answer in the active turn.",
            &entries,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn astra_kickoff_with_skills_plugins_and_remote_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let skills = write_scenario_capabilities(&home)?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("draft", "Agenda drafted for the kickoff."),
                ev_completed("draft-response"),
            ]),
            sse(vec![
                ev_assistant_message("notes", "The team chose Friday and assigned owners."),
                ev_completed("notes-response"),
            ]),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "SCENARIO_REMOTE_CHECKPOINT",
                    }
                }),
                ev_completed("compact-response"),
            ]),
            sse(vec![
                ev_assistant_message("final", "Here is the checked kickoff brief."),
                ev_completed("final-response"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_model("gpt-6-astra")
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(skills_extensions())
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join("AGENTS.md"))?,
                b"Kickoff updates must name an owner and a date.".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        })
        .with_config(|config| {
            configure_scenario_catalog(config);
        });
    let test = builder.build(&server).await?;

    for input in [
        vec![
            text("Plan a team kickoff for Friday using $outline and $calendar:agenda."),
            selected_skill("outline", &skills.outline),
            selected_skill("calendar:agenda", &skills.agenda),
            plugin("calendar"),
        ],
        vec![
            text("Summarize the kickoff notes with $notes:summarize."),
            selected_skill("notes:summarize", &skills.summarize),
            plugin("notes"),
        ],
    ]
    .into_iter()
    {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(input))
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Check the final kickoff brief and attached sketch with $final-check and $calendar:agenda."),
            UserInput::Image {
                image_url: format!("data:image/png;base64,{ONE_PIXEL_PNG_BASE64}"),
                detail: None,
            },
            selected_skill("final-check", &skills.final_check),
            plugin("calendar"),
        ]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = mock.requests();
    insta::assert_snapshot!(
        "astra_kickoff_remote_compaction_windows",
        context_snapshot::format_request_history_snapshot(
            "Astra plans a kickoff with local and plugin skills, remotely compacts, and checks an image brief.",
            &requests,
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}

#[cfg_attr(windows, ignore = "the fixture uses a Unix shell command")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn astra_settings_release_check_with_direct_and_code_mode_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the fixture uses a Unix shell command");

    let server = start_mock_server().await;
    let rmcp_server_bin = stdio_server_bin()?;
    let home = Arc::new(TempDir::new()?);
    let mut builder = test_codex()
        .with_model("gpt-6-astra")
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            configure_scenario_catalog(config);
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "rmcp".to_string(),
                serde_json::from_value::<McpServerConfig>(json!({
                    "command": rmcp_server_bin,
                    "env": { "MCP_TEST_VALUE": "release-check" },
                }))
                .expect("test MCP server config"),
            );
            config.mcp_servers.set(servers).expect("test MCP servers");
        });
    let test = builder.build(&server).await?;
    let release = test.cwd_path().join("release");
    fs::create_dir_all(&release)?;
    let diagnostics = std::iter::once("UI-42: expected Settings button label: Apply".to_string())
        .chain((1..=120).map(|line| format!("diagnostic {line:03}: rendering settings panel")))
        .chain(std::iter::once(
            "UI-42: observed Settings button label: Save".to_string(),
        ))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(release.join("diagnostics.log"), diagnostics)?;
    fs::write(release.join("status.md"), "Status: pending\n")?;
    fs::write(
        release.join("settings.png"),
        BASE64_STANDARD.decode(ONE_PIXEL_PNG_BASE64)?,
    )?;
    wait_for_mcp_server(&test.codex, "rmcp").await?;

    let patch = "*** Begin Patch\n*** Update File: release/status.md\n@@\n-Status: pending\n+Status: blocked\n+Reason: expected Apply; observed Save\n+MCP: reachable\n*** End Patch\n";
    let patch_code = format!("text(await tools.apply_patch(`{patch}`));");
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("agents-response"),
                ev_function_call_with_namespace("agents-call", "collaboration", "list_agents", "{}"),
                ev_completed("agents-response"),
            ]),
            sse(vec![
                ev_response_created("diagnostics-response"),
                ev_custom_tool_call(
                    "diagnostics-call",
                    "exec",
                    r#"const [log, ping] = await Promise.all([
  tools.exec_command({ cmd: "cat release/diagnostics.log", login: false, max_output_tokens: 4000 }),
  tools.mcp__rmcp__echo({ message: "settings-release-check" }),
]);
text(`diagnostics:\n${log.output}`);
text(`MCP: ${ping.structuredContent?.echo ?? "missing"}`);"#,
                ),
                ev_completed("diagnostics-response"),
            ]),
            sse(vec![
                ev_response_created("image-response"),
                ev_custom_tool_call(
                    "image-call",
                    "exec",
                    "image(await tools.view_image({ path: \"release/settings.png\", detail: \"original\" }));",
                ),
                ev_completed("image-response"),
            ]),
            sse(vec![
                ev_response_created("patch-response"),
                ev_custom_tool_call("patch-call", "exec", &patch_code),
                ev_completed("patch-response"),
            ]),
            sse(vec![
                ev_response_created("readback-response"),
                ev_custom_tool_call(
                    "readback-call",
                    "exec",
                    "text((await tools.exec_command({ cmd: \"cat release/status.md\", login: false })).output);",
                ),
                ev_completed("readback-response"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "final",
                    "The Settings release is blocked: expected Apply, observed Save. The MCP integration responded and no other agent is working on this task.",
                ),
                ev_completed("final-response"),
            ]),
        ],
    )
    .await;

    test.submit_turn("Check the Settings release. Read release/diagnostics.log, inspect release/settings.png, confirm the local MCP integration responds, and update release/status.md with the result. Tell me whether another agent is working on this task.").await?;
    insta::assert_snapshot!(
        "astra_settings_release_check_tool_shapes",
        context_snapshot::format_request_history_snapshot(
            "Astra checks a Settings release using direct collaboration and Code Mode tools.",
            &mock.requests(),
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn astra_refreshes_plugin_tools_and_skills_in_an_existing_thread() -> Result<()> {
    skip_if_no_network!(Ok(()));
    core_test_support::skip_if_remote!(Ok(()), "plugin and MCP fixtures use host-local paths");

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let config = "[features]\nplugins = true\n\n[skills.bundled]\nenabled = false\n";
    fs::write(home.path().join("config.toml"), config)?;
    let lookup = r#"text(ALL_TOOLS.filter(({ name }) => name === "mcp__notes__echo").map(({ name }) => name));"#;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("before-refresh"),
                ev_custom_tool_call("before-lookup", "exec", lookup),
                ev_completed("before-refresh"),
            ]),
            sse(vec![
                ev_assistant_message("missing-tool", "The Notes tool is not installed yet."),
                ev_completed("missing-tool-response"),
            ]),
            sse(vec![
                ev_response_created("after-refresh"),
                ev_custom_tool_call("after-lookup", "exec", lookup),
                ev_completed("after-refresh"),
            ]),
            sse(vec![
                ev_response_created("first-echo"),
                ev_custom_tool_call(
                    "first-echo-call",
                    "exec",
                    r#"const result = await tools.mcp__notes__echo({ message: "Mira owns the kickoff" }); text(result.structuredContent?.echo);"#,
                ),
                ev_completed("first-echo"),
            ]),
            sse(vec![
                ev_assistant_message("owner", "The Notes plugin confirms Mira owns the kickoff."),
                ev_completed("owner-response"),
            ]),
            sse(vec![
                ev_response_created("follow-up"),
                ev_custom_tool_call(
                    "second-echo-call",
                    "exec",
                    r#"const result = await tools.mcp__notes__echo({ message: "The kickoff is Friday" }); text(result.structuredContent?.echo);"#,
                ),
                ev_completed("follow-up"),
            ]),
            sse(vec![
                ev_assistant_message("deadline", "The Notes plugin confirms the kickoff is Friday."),
                ev_completed("deadline-response"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_model("gpt-6-astra")
        .with_home(Arc::clone(&home))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(skills_extensions())
        .with_config(configure_scenario_catalog);
    let test = builder.build(&server).await?;

    test.submit_turn("Check whether the Notes plugin echo tool is available.")
        .await?;

    let plugin_root = home.path().join("plugins/cache/test/notes/local");
    fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        json!({ "name": "notes", "description": "Look up and summarize team notes" }).to_string(),
    )?;
    fs::write(
        plugin_root.join(".mcp.json"),
        json!({ "mcpServers": { "notes": { "command": stdio_server_bin()?, "cwd": "." } } })
            .to_string(),
    )?;
    let skill = write_skill(
        &plugin_root.join("skills/summarize"),
        "summarize",
        "Summarize team notes",
        "State the owner and date from the notes.",
    )?;
    fs::write(
        home.path().join("config.toml"),
        format!("{config}\n[plugins.\"notes@test\"]\nenabled = true\n"),
    )?;
    test.codex.submit(Op::ReloadUserConfig).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("I installed Notes. Use $notes:summarize and the Notes tool to check that Mira owns the kickoff."),
            plugin("notes"),
            selected_skill("notes:summarize", &skill),
        ]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.submit_text_turn("Use the Notes tool again to check that the kickoff is Friday.")
        .await?;

    insta::assert_snapshot!(
        "astra_plugin_refresh",
        context_snapshot::format_request_history_snapshot(
            "Astra checks for Notes, refreshes its installed plugin without restarting, and uses the new skill and Code Mode MCP tool across turns.",
            &mock.requests(),
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
