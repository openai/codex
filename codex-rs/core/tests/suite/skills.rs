#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use std::fs;
use std::path::Path;

fn write_skill(home: &Path, name: &str, description: &str, body: &str) -> std::path::PathBuf {
    let skill_dir = home.join("skills").join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let path = skill_dir.join("SKILL.md");
    fs::write(&path, contents).unwrap();
    path
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
    let mut builder = test_codex().with_pre_build_hook(|home| {
        write_skill(home, "demo", "demo skill", skill_body);
    });
    let test = builder.build(&server).await?;

    let skill_path = test.codex_home_path().join("skills/demo/SKILL.md");
    let skill_path = std::fs::canonicalize(skill_path)?;

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
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: codex_protocol::config_types::ReasoningSummary::Auto,
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
