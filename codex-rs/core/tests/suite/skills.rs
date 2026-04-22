#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::ThreadManager;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::ExecutorFileSystem;
use codex_login::CodexAuth;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

async fn write_repo_skill(
    cwd: AbsolutePathBuf,
    fs: Arc<dyn ExecutorFileSystem>,
    name: &str,
    description: &str,
    body: &str,
) -> Result<()> {
    let skill_dir = cwd.join(".agents").join("skills").join(name);
    fs.create_directory(
        &skill_dir,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let path = skill_dir.join("SKILL.md");
    fs.write_file(&path, contents.into_bytes(), /*sandbox*/ None)
        .await?;
    Ok(())
}

fn write_home_skill(codex_home: &Path, dir: &str, name: &str, description: &str) -> Result<()> {
    let skill_dir = codex_home.join("skills").join(dir);
    fs::create_dir_all(&skill_dir)?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), contents)?;
    Ok(())
}

fn system_skill_md_path(home: impl AsRef<Path>, name: &str) -> std::path::PathBuf {
    home.as_ref()
        .join("skills")
        .join(".system")
        .join(name)
        .join("SKILL.md")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_includes_skill_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let skill_body = "skill body";
    let mut builder = test_codex().with_workspace_setup(move |cwd, fs| async move {
        write_repo_skill(cwd, fs, "demo", "demo skill", skill_body).await
    });
    let test = builder.build_remote_aware(&server).await?;

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
    test.codex
        .submit(Op::UserTurn {
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
            cwd: test.config.cwd.to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            permission_profile: None,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_skills_includes_repo_and_home_skills_remote_aware() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_home_skill(home, "home-demo", "home-demo", "from home")
                .expect("write home skill");
        })
        .with_workspace_setup(|cwd, fs| async move {
            write_repo_skill(cwd, fs, "repo-demo", "from repo", "# Body").await
        });
    let test = builder.build_remote_aware(&server).await?;

    test.codex
        .submit(Op::ListSkills {
            cwds: Vec::new(),
            force_reload: true,
        })
        .await?;
    let response =
        core_test_support::wait_for_event_match(test.codex.as_ref(), |event| match event {
            codex_protocol::protocol::EventMsg::ListSkillsResponse(response) => {
                Some(response.clone())
            }
            _ => None,
        })
        .await;

    let cwd = test.config.cwd.as_path();
    let skills = response
        .skills
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| entry.skills.clone())
        .unwrap_or_default();

    let repo_skill = skills
        .iter()
        .find(|skill| skill.name == "repo-demo")
        .expect("expected repo skill");
    assert_eq!(repo_skill.scope, codex_protocol::protocol::SkillScope::Repo);
    let repo_path = repo_skill.path.to_string_lossy().replace('\\', "/");
    assert!(
        repo_path.ends_with("/.agents/skills/repo-demo/SKILL.md"),
        "unexpected repo skill path: {repo_path}"
    );

    let home_skill = skills
        .iter()
        .find(|skill| skill.name == "home-demo")
        .expect("expected home skill");
    assert_eq!(home_skill.scope, codex_protocol::protocol::SkillScope::User);
    let home_path = home_skill.path.to_string_lossy().replace('\\', "/");
    assert!(
        home_path.ends_with("/skills/home-demo/SKILL.md"),
        "unexpected home skill path: {home_path}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_skills_skips_cwd_roots_when_environment_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_home_skill(
        codex_home.path(),
        "home-disabled",
        "home-disabled",
        "from home",
    )?;
    let repo_skill_dir = cwd
        .path()
        .join(".agents")
        .join("skills")
        .join("repo-disabled");
    fs::create_dir_all(&repo_skill_dir)?;
    fs::write(
        repo_skill_dir.join("SKILL.md"),
        "---\nname: repo-disabled\ndescription: from repo\n---\n\n# Body\n",
    )?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.cwd = AbsolutePathBuf::from_absolute_path_checked(cwd.path())?;

    let thread_manager = ThreadManager::new(
        &config,
        codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("dummy")),
        SessionSource::Exec,
        CollaborationModesConfig::default(),
        Arc::new(EnvironmentManager::new(
            codex_exec_server::EnvironmentManagerArgs {
                exec_server_url: Some("none".to_string()),
                local_runtime_paths: ExecServerRuntimePaths::new(
                    std::env::current_exe()?,
                    /*codex_linux_sandbox_exe*/ None,
                )?,
            },
        )),
        /*analytics_events_client*/ None,
    );
    let new_thread = thread_manager.start_thread(config.clone()).await?;
    let cwd = config.cwd.to_path_buf();

    new_thread
        .thread
        .submit(Op::ListSkills {
            cwds: vec![cwd.clone()],
            force_reload: true,
        })
        .await?;
    let response =
        core_test_support::wait_for_event_match(new_thread.thread.as_ref(), |event| match event {
            codex_protocol::protocol::EventMsg::ListSkillsResponse(response) => {
                Some(response.clone())
            }
            _ => None,
        })
        .await;

    assert_eq!(response.skills.len(), 1);
    assert_eq!(response.skills[0].cwd, cwd);
    assert_eq!(response.skills[0].errors.len(), 0);
    assert!(
        response.skills[0]
            .skills
            .iter()
            .any(|skill| skill.name == "home-disabled")
    );
    assert!(
        response.skills[0]
            .skills
            .iter()
            .all(|skill| skill.name != "repo-disabled")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_load_errors_surface_in_session_configured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_pre_build_hook(|home| {
        let skill_dir = home.join("skills").join("broken");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "not yaml").unwrap();
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::ListSkills {
            cwds: Vec::new(),
            force_reload: false,
        })
        .await?;
    let response =
        core_test_support::wait_for_event_match(test.codex.as_ref(), |event| match event {
            codex_protocol::protocol::EventMsg::ListSkillsResponse(response) => {
                Some(response.clone())
            }
            _ => None,
        })
        .await;

    let cwd = test.cwd_path();
    let (skills, errors) = response
        .skills
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| (entry.skills.clone(), entry.errors.clone()))
        .unwrap_or_default();

    assert!(
        skills.iter().all(|skill| {
            !skill
                .path
                .to_string_lossy()
                .ends_with("skills/broken/SKILL.md")
        }),
        "expected broken skill not loaded, got {skills:?}"
    );
    assert_eq!(errors.len(), 1, "expected one load error");
    let error_path = errors[0].path.to_string_lossy();
    assert!(
        error_path.ends_with("skills/broken/SKILL.md"),
        "unexpected error path: {error_path}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_skills_includes_system_cache_entries() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const SYSTEM_SKILL_NAME: &str = "skill-creator";

    let server = start_mock_server().await;
    let mut builder = test_codex().with_pre_build_hook(|home| {
        let system_skill_path = system_skill_md_path(home, SYSTEM_SKILL_NAME);
        assert!(
            !system_skill_path.exists(),
            "expected embedded system skills not yet installed, but {system_skill_path:?} exists"
        );
    });
    let test = builder.build(&server).await?;

    let system_skill_path = system_skill_md_path(test.codex_home_path(), SYSTEM_SKILL_NAME);
    assert!(
        system_skill_path.exists(),
        "expected embedded system skills installed to {system_skill_path:?}"
    );
    let system_skill_contents = fs::read_to_string(&system_skill_path)?;
    let expected_name_line = format!("name: {SYSTEM_SKILL_NAME}");
    assert!(
        system_skill_contents.contains(&expected_name_line),
        "expected embedded system skill file, got:\n{system_skill_contents}"
    );

    test.codex
        .submit(Op::ListSkills {
            cwds: Vec::new(),
            force_reload: true,
        })
        .await?;
    let response =
        core_test_support::wait_for_event_match(test.codex.as_ref(), |event| match event {
            codex_protocol::protocol::EventMsg::ListSkillsResponse(response) => {
                Some(response.clone())
            }
            _ => None,
        })
        .await;

    let cwd = test.cwd_path();
    let (skills, _errors) = response
        .skills
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| (entry.skills.clone(), entry.errors.clone()))
        .unwrap_or_default();

    let skill = skills
        .iter()
        .find(|skill| skill.name == SYSTEM_SKILL_NAME)
        .expect("expected system skill to be present");
    assert_eq!(skill.scope, codex_protocol::protocol::SkillScope::System);
    let path_str = skill.path.to_string_lossy().replace('\\', "/");
    let expected_path_suffix = format!("/skills/.system/{SYSTEM_SKILL_NAME}/SKILL.md");
    assert!(
        path_str.ends_with(&expected_path_suffix),
        "unexpected skill path: {path_str}"
    );

    Ok(())
}
