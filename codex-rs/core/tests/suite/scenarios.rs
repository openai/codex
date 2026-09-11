//! A multi-turn Astra scenario whose actual requests expose the context around remote compaction.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::types::McpServerConfig;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
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
use core_test_support::test_codex::executor_path_uri;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use serde_json::json;
use tempfile::TempDir;

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
