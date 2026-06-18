#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event_match;
use std::collections::HashMap;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_includes_skill_instructions() -> Result<()> {
    // TODO(anp): Remove after skill-path helpers use target-native paths.
    skip_if_wine_exec!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let skill_body = "skill body";
    let mut builder = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(codex_features::Feature::SkillMcpDependencyInstall)
                .expect("test config should allow skill MCP dependency install");
        })
        .with_workspace_setup(move |cwd, fs| async move {
            write_repo_skill(
                cwd.clone(),
                Arc::clone(&fs),
                "demo",
                "demo skill",
                skill_body,
            )
            .await?;
            let metadata_dir = cwd.join(".agents/skills/demo/agents");
            fs.create_directory(
                &PathUri::from_path(&metadata_dir)?,
                CreateDirectoryOptions { recursive: true },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_path(metadata_dir.join("openai.yaml"))?,
                br#"dependencies:
  tools:
    - type: "mcp"
      value: "docs"
      transport: "streamable_http"
      url: "https://example.com/mcp"
"#
                .to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
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
                approval_policy: Some(AskForApproval::OnRequest),
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

    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.questions.len(), 1);
    assert_eq!(request.questions[0].id, "skill_mcp_dependency_install");
    assert!(request.questions[0].question.contains("docs"));
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "skill_mcp_dependency_install".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Continue anyway".to_string()],
                    },
                )]),
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
