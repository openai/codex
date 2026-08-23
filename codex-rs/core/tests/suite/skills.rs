#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_core::StartIfIdleSubmission;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::create_directory_symlink;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use std::sync::Arc;

async fn write_repo_skill(
    cwd: AbsolutePathBuf,
    fs: Arc<dyn ExecutorFileSystem>,
    name: &str,
    description: &str,
    body: &str,
) -> Result<()> {
    let skill_dir = cwd.join(".agents").join("skills").join(name);
    let skill_dir_uri = PathUri::from_host_native_path(&skill_dir)?;
    fs.create_directory(
        &skill_dir_uri,
        CreateDirectoryOptions {
            recursive: true,
            follow_symlinks: true,
        },
        /*sandbox*/ None,
    )
    .await?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let path = skill_dir.join("SKILL.md");
    let path_uri = PathUri::from_host_native_path(&path)?;
    fs.write_file(
        &path_uri,
        contents.into_bytes(),
        Default::default(),
        /*sandbox*/ None,
    )
    .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_includes_skill_instructions() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "skill paths require matching host and executor path conventions"
    );
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let skill_body = "skill body";
    let mut builder = test_codex().with_workspace_setup(move |cwd, fs| async move {
        write_repo_skill(cwd, fs, "demo", "demo skill", skill_body).await
    });
    let test = builder.build_with_auto_env(&server).await?;

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
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![
                UserInput::Text {
                    text: "please use $demo".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "demo".to_string(),
                    path: skill_path.clone(),
                },
            ])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
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
    assert!(request.has_content_kinds(&["skills.selected_skill_instructions"]));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_selects_symlinked_skill_by_advertised_discovery_path() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "remote filesystems do not expose directory symlink creation"
    );

    let server = start_mock_server().await;
    let skill_body = "instructions from the canonical linked skill";
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install(&mut extensions, |config: &Config| SkillsExtensionConfig {
        include_instructions: config.include_skill_instructions,
        max_context_tokens: config.skill_max_context_tokens,
        bundled_skills_enabled: false,
        orchestrator_skills_enabled: false,
        shadow_selection_enabled: false,
    });
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_workspace_setup(move |cwd, _fs| async move {
            let source_skill_dir = cwd.join("shared-skills/linked-demo");
            let discovery_root = cwd.join(".agents/skills");
            std::fs::create_dir_all(source_skill_dir.as_path())?;
            std::fs::create_dir_all(discovery_root.as_path())?;
            std::fs::write(
                source_skill_dir.join("SKILL.md"),
                format!(
                    "---\nname: linked-demo\ndescription: Linked demo skill\n---\n\n{skill_body}\n"
                ),
            )?;
            create_directory_symlink(
                source_skill_dir.as_path(),
                discovery_root.join("linked-demo").as_path(),
            );
            Ok(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    let discovery_root = test.config.cwd.join(".agents/skills").canonicalize()?;
    let discovery_path = discovery_root.join("linked-demo/SKILL.md");
    let canonical_path = discovery_path.canonicalize()?;
    let discovery_path_display = discovery_path.display();
    let canonical_path_display = canonical_path.display();
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("linked-skill-response"),
            ev_assistant_message("linked-skill-message", "done"),
            ev_completed("linked-skill-response"),
        ]),
    )
    .await;

    let submission = test
        .codex
        .start_turn_if_idle(TurnInputRequest::new(TurnInput::UserInput {
            content: vec![
                UserInput::Text {
                    text: format!("please use [$linked-demo]({discovery_path_display})"),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "linked-demo".to_string(),
                    path: discovery_path.to_path_buf(),
                },
            ],
            client_id: Some("linked-skill-user-message".to_string()),
        }))
        .await?;
    assert!(matches!(submission, StartIfIdleSubmission::Started { .. }));

    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    let request = mock.single_request();
    let developer_texts = request.message_input_texts("developer");
    let advertised_path = format!(
        "(file: {})",
        discovery_path.to_string_lossy().replace('\\', "/")
    );
    assert!(
        developer_texts
            .iter()
            .any(|text| text.contains(&advertised_path)),
        "expected symlink discovery path in the skill catalog, got {developer_texts:?}"
    );

    let user_texts = request.message_input_texts("user");
    let canonical_identity = format!("<path>{canonical_path_display}</path>");
    assert!(
        user_texts.iter().any(|text| {
            text.contains("<skill>\n<name>linked-demo</name>")
                && text.contains(&canonical_identity)
                && text.contains(skill_body)
        }),
        "expected canonical skill instructions selected by discovery path, got {user_texts:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_user_turn_includes_skill_instructions_in_the_first_request() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "skill paths require matching host and executor path conventions"
    );
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let skill_body = "queued skill body";
    let mut builder = test_codex().with_workspace_setup(move |cwd, fs| async move {
        write_repo_skill(cwd, fs, "queued-demo", "queued demo skill", skill_body).await
    });
    let test = builder.build_with_auto_env(&server).await?;
    let skill_path = test
        .config
        .cwd
        .join(".agents/skills/queued-demo/SKILL.md")
        .canonicalize()
        .unwrap_or_else(|_| test.config.cwd.join(".agents/skills/queued-demo/SKILL.md"))
        .to_path_buf();
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("queued-skill-response"),
            ev_assistant_message("queued-skill-message", "done"),
            ev_completed("queued-skill-response"),
        ]),
    )
    .await;

    let submission = test
        .codex
        .start_turn_if_idle(TurnInputRequest::new(TurnInput::UserInput {
            content: vec![
                UserInput::Text {
                    text: "please use $queued-demo".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "queued-demo".to_string(),
                    path: skill_path.clone(),
                },
            ],
            client_id: Some("queued-skill-user-message".to_string()),
        }))
        .await?;
    assert!(matches!(submission, StartIfIdleSubmission::Started { .. }));

    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    let user_texts = mock.single_request().message_input_texts("user");
    let skill_path_str = skill_path.to_string_lossy();
    assert!(
        user_texts.iter().any(|text| {
            text.contains("<skill>\n<name>queued-demo</name>")
                && text.contains("<path>")
                && text.contains(skill_body)
                && text.contains(skill_path_str.as_ref())
        }),
        "expected queued skill instructions in the first request, got {user_texts:?}"
    );

    Ok(())
}
