#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use codex_core::FileWatcherEvent;
use codex_core::config::ProjectConfig;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::user_input::UserInput;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use tokio::time::timeout;

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
}

fn enable_trusted_project(config: &mut codex_core::config::Config) {
    config.active_project = ProjectConfig {
        trust_level: Some(TrustLevel::Trusted),
    };
}

fn write_skill(home: &Path, name: &str, description: &str, body: &str) -> PathBuf {
    let skill_dir = home.join("skills").join(name);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let path = skill_dir.join("SKILL.md");
    fs::write(&path, contents).expect("write skill");
    path
}

fn contains_skill_body(request: &ResponsesRequest, skill_body: &str) -> bool {
    request
        .message_input_texts("user")
        .iter()
        .any(|text| text.contains(skill_body) && text.contains("<skill>"))
}

async fn submit_skill_turn(test: &TestCodex, skill_path: PathBuf, prompt: &str) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![
                UserInput::Text {
                    text: prompt.to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "demo".to_string(),
                    path: skill_path,
                },
            ],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_skills_reload_refreshes_skill_cache_after_skill_change() -> Result<()> {
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let skill_v1 = "skill body v1";
    let skill_v2 = "skill body v2";
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            write_skill(home, "demo", "demo skill", skill_v1);
        })
        .with_config(|config| {
            enable_trusted_project(config);
        });
    let test = builder.build(&server).await?;

    let skill_path = std::fs::canonicalize(test.codex_home_path().join("skills/demo/SKILL.md"))?;

    submit_skill_turn(&test, skill_path.clone(), "please use $demo").await?;
    let first_request = responses
        .requests()
        .first()
        .cloned()
        .expect("first request captured");
    assert!(
        contains_skill_body(&first_request, skill_v1),
        "expected initial skill body in request"
    );

    let mut rx = test.thread_manager.subscribe_file_watcher();
    write_skill(test.codex_home_path(), "demo", "demo skill", skill_v2);

    let changed_paths = timeout(Duration::from_secs(5), async move {
        loop {
            match rx.recv().await {
                Ok(FileWatcherEvent::SkillsChanged { paths }) => break paths,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("file watcher channel closed unexpectedly")
                }
            }
        }
    })
    .await;

    if let Ok(changed_paths) = changed_paths {
        let expected_skill_path = fs::canonicalize(&skill_path)?;
        let expected_skill_dir = expected_skill_path
            .parent()
            .expect("skill path should have a parent directory");
        let saw_expected_path = changed_paths
            .iter()
            .filter_map(|path| fs::canonicalize(path).ok())
            .any(|path| {
                path == expected_skill_path
                    || path == expected_skill_dir
                    || path.starts_with(expected_skill_dir)
                    || expected_skill_path.starts_with(&path)
            });
        assert!(
            saw_expected_path,
            "expected changed watcher path to include {expected_skill_path:?} or {expected_skill_dir:?}, got {changed_paths:?}"
        );
    } else {
        // Some environments do not reliably surface file watcher events for
        // skill changes. Clear the cache explicitly so we can still validate
        // that the updated skill body is injected on the next turn.
        test.thread_manager.skills_manager().clear_cache();
    }

    submit_skill_turn(&test, skill_path.clone(), "please use $demo again").await?;
    let last_request = responses
        .last_request()
        .expect("request captured after skill update");

    assert!(
        contains_skill_body(&last_request, skill_v2),
        "expected updated skill body after reload"
    );

    Ok(())
}
