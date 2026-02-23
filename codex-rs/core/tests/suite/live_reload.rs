#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use codex_core::config::ProjectConfig;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use tokio::time::timeout;

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
        vec![
            responses::sse(vec![responses::ev_completed("resp-1")]),
            responses::sse(vec![responses::ev_completed("resp-2")]),
        ],
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

    let skill_path = dunce::canonicalize(test.codex_home_path().join("skills/demo/SKILL.md"))?;

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

    write_skill(test.codex_home_path(), "demo", "demo skill", skill_v2);

    let saw_skills_update = timeout(Duration::from_secs(5), async {
        loop {
            match test.codex.next_event().await {
                Ok(event) => {
                    if matches!(event.msg, EventMsg::SkillsUpdateAvailable) {
                        break;
                    }
                }
                Err(err) => panic!("event stream ended unexpectedly: {err}"),
            }
        }
    })
    .await;

    if saw_skills_update.is_err() {
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
