#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Arc;

async fn write_repo_skill(
    cwd: AbsolutePathBuf,
    fs: Arc<dyn ExecutorFileSystem>,
    name: &str,
    description: &str,
    body: &str,
) -> Result<()> {
    let skill_dir = cwd.join(".agents").join("skills").join(name);
    let skill_dir_uri = PathUri::from_path(&skill_dir)?;
    fs.create_directory(
        &skill_dir_uri,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let path = skill_dir.join("SKILL.md");
    let path_uri = PathUri::from_path(&path)?;
    fs.write_file(&path_uri, contents.into_bytes(), /*sandbox*/ None)
        .await?;
    Ok(())
}

async fn submit_user_turn(test: &TestCodex, cwd: AbsolutePathBuf, prompt: &str) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_includes_skill_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let skill_body = "skill body";
    let mut builder = test_codex().with_workspace_setup(move |cwd, fs| async move {
        write_repo_skill(cwd, fs, "demo", "demo skill", skill_body).await
    });
    let test = builder.build_with_remote_env(&server).await?;

    let skill_path = test
        .config
        .cwd
        .join(".agents/skills/demo/SKILL.md")
        .canonicalize()
        .unwrap_or_else(|_| test.config.cwd.join(".agents/skills/demo/SKILL.md"))
        .to_path_buf();

    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![
                UserInput::Text {
                    text: "please use $demo".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "demo".to_string(),
                    path: skill_path.clone(),
                },
            ],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    let request = mock.single_request();
    let user_texts = request.message_input_texts("user");
    let skill_path_str = skill_path.to_string_lossy();
    assert!(
        user_texts.iter().any(|text| {
            text.contains("<skill>\n<name>demo</name>")
                && text.contains("<path>")
                && text.contains(skill_body)
                && text.contains(skill_path_str.as_ref())
        }),
        "expected skill instructions in user input, got {user_texts:?}"
    );

    Ok(())
}

fn top_level_tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_search_tool_is_visible_and_returns_matching_repo_skill() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let search_call_id = "skill-search-call";
    let mut builder = test_codex()
        .with_skill_search_extension()
        .with_config(|config| {
            config
                .features
                .enable(Feature::SkillSearchTool)
                .expect("skill search feature should be configurable");
        })
        .with_workspace_setup(move |cwd, fs| async move {
            write_repo_skill(
                cwd,
                fs,
                "demo",
                "Find demo workflows",
                "Use this skill for demo workflow work.",
            )
            .await
        });
    let test = builder.build_with_remote_env(&server).await?;

    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    search_call_id,
                    "skill_search",
                    r#"{"query":"demo workflows"}"#,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    submit_user_turn(&test, test.config.cwd.clone(), "Find the right repo skill.").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);

    let first_request = &requests[0];
    let tool_names = top_level_tool_names(&first_request.body_json());
    assert!(
        tool_names.iter().any(|name| name == "skill_search"),
        "skill_search should be visible to the model: {tool_names:?}"
    );
    assert!(
        first_request.body_contains_text("Use `skill_search` when a task may benefit from one"),
        "skill-search guidance should replace the static skills block"
    );

    let output = mock
        .function_call_output_text(search_call_id)
        .expect("skill_search output should be posted back to the model");
    assert!(
        output.contains("- demo: Find demo workflows"),
        "skill_search output should include the matching repo skill: {output}"
    );
    assert!(
        output.contains(".agents/skills/demo/SKILL.md"),
        "skill_search output should include the SKILL.md path: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_skill_search_keeps_static_instructions_and_hides_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_skill_search_extension()
        .with_workspace_setup(move |cwd, fs| async move {
            write_repo_skill(
                cwd,
                fs,
                "demo",
                "Find disabled-feature workflows",
                "Use this skill for disabled-feature workflow work.",
            )
            .await
        });
    let test = builder.build_with_remote_env(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-disabled"),
            ev_assistant_message("msg-disabled", "done"),
            ev_completed("resp-disabled"),
        ]),
    )
    .await;

    submit_user_turn(
        &test,
        test.config.cwd.clone(),
        "Find the disabled-feature workflow.",
    )
    .await?;

    let request = mock.single_request();
    assert!(
        !top_level_tool_names(&request.body_json())
            .iter()
            .any(|name| name == "skill_search"),
        "skill_search should be hidden when the feature is disabled"
    );
    assert!(request.body_contains_text("<skills_instructions>"));
    assert!(request.body_contains_text("Find disabled-feature workflows"));
    assert!(!request.body_contains_text("Use `skill_search` when a task may benefit from one"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_search_excludes_skills_that_disallow_implicit_invocation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let search_call_id = "disallowed-skill-search-call";
    let mut builder = test_codex()
        .with_skill_search_extension()
        .with_config(|config| {
            config
                .features
                .enable(Feature::SkillSearchTool)
                .expect("skill search feature should be configurable");
        })
        .with_workspace_setup(move |cwd, fs| async move {
            write_repo_skill(
                cwd.clone(),
                Arc::clone(&fs),
                "private-demo",
                "Find private demo workflows",
                "Use this skill only when explicitly invoked.",
            )
            .await?;
            let metadata_dir = cwd
                .join(".agents")
                .join("skills")
                .join("private-demo")
                .join("agents");
            let metadata_dir_uri = PathUri::from_path(&metadata_dir)?;
            fs.create_directory(
                &metadata_dir_uri,
                CreateDirectoryOptions { recursive: true },
                /*sandbox*/ None,
            )
            .await?;
            let metadata_path_uri = PathUri::from_path(metadata_dir.join("openai.yaml"))?;
            fs.write_file(
                &metadata_path_uri,
                b"policy:\n  allow_implicit_invocation: false\n".to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_remote_env(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-disallowed-1"),
                ev_function_call(
                    search_call_id,
                    "skill_search",
                    r#"{"query":"private demo workflows"}"#,
                ),
                ev_completed("resp-disallowed-1"),
            ]),
            sse(vec![
                ev_response_created("resp-disallowed-2"),
                ev_assistant_message("msg-disallowed", "done"),
                ev_completed("resp-disallowed-2"),
            ]),
        ],
    )
    .await;

    submit_user_turn(
        &test,
        test.config.cwd.clone(),
        "Search for the private demo workflow.",
    )
    .await?;

    assert_eq!(
        mock.function_call_output_text(search_call_id).as_deref(),
        Some("")
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_search_uses_host_loaded_skills_from_each_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_call_id = "first-turn-skill-search-call";
    let second_call_id = "second-turn-skill-search-call";
    let mut builder = test_codex()
        .with_skill_search_extension()
        .with_config(|config| {
            config
                .features
                .enable(Feature::SkillSearchTool)
                .expect("skill search feature should be configurable");
        })
        .with_workspace_setup(move |cwd, fs| async move {
            write_repo_skill(
                cwd.join("turn-one"),
                Arc::clone(&fs),
                "alpha-skill",
                "Alpha-only workflow",
                "Use this skill for alpha work.",
            )
            .await?;
            write_repo_skill(
                cwd.join("turn-two"),
                fs,
                "beta-skill",
                "Beta-only workflow",
                "Use this skill for beta work.",
            )
            .await
        });
    let test = builder.build_with_remote_env(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-turn-one-1"),
                ev_function_call(
                    first_call_id,
                    "skill_search",
                    r#"{"query":"alpha-only workflow"}"#,
                ),
                ev_completed("resp-turn-one-1"),
            ]),
            sse(vec![
                ev_response_created("resp-turn-one-2"),
                ev_assistant_message("msg-turn-one", "done"),
                ev_completed("resp-turn-one-2"),
            ]),
            sse(vec![
                ev_response_created("resp-turn-two-1"),
                ev_function_call(
                    second_call_id,
                    "skill_search",
                    r#"{"query":"beta-only workflow"}"#,
                ),
                ev_completed("resp-turn-two-1"),
            ]),
            sse(vec![
                ev_response_created("resp-turn-two-2"),
                ev_assistant_message("msg-turn-two", "done"),
                ev_completed("resp-turn-two-2"),
            ]),
        ],
    )
    .await;

    submit_user_turn(
        &test,
        test.config.cwd.join("turn-one"),
        "Find the alpha workflow.",
    )
    .await?;
    submit_user_turn(
        &test,
        test.config.cwd.join("turn-two"),
        "Find the beta workflow.",
    )
    .await?;

    let first_output = mock
        .function_call_output_text(first_call_id)
        .expect("first turn should return skill search output");
    assert!(first_output.contains("alpha-skill"));
    assert!(!first_output.contains("beta-skill"));

    let second_output = mock
        .function_call_output_text(second_call_id)
        .expect("second turn should return skill search output");
    assert!(second_output.contains("beta-skill"));
    assert!(!second_output.contains("alpha-skill"));

    Ok(())
}
