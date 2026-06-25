use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_features::Feature;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

const GLOBAL_INSTRUCTIONS: &str = "global user instructions";
const HOOK_INSTRUCTIONS: &str = r#"{"source":"user-instructions-hook"}"#;
const PROJECT_INSTRUCTIONS: &str = "project instructions";

fn python_hook_command(script_path: &Path) -> String {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let script_path = if cfg!(windows) {
        format!(r#""{}""#, script_path.display())
    } else {
        format!(
            "'{}'",
            script_path.display().to_string().replace('\'', "'\\''")
        )
    };
    format!("{python} {script_path}")
}

fn write_user_instructions_hook(home: &Path, output: &str, exit_code: Option<i32>) -> Result<()> {
    let script_path = home.join("user_instructions_hook.py");
    let log_path = home.join("user_instructions_hook_input.json");
    let output = serde_json::to_string(output).context("serialize hook output")?;
    let exit = exit_code
        .map(|exit_code| format!("raise SystemExit({exit_code})"))
        .unwrap_or_else(|| format!("sys.stdout.write({output})"));
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as log:
    log.write(json.dumps(payload) + "\n")
{exit}
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "UserInstructions": {
                "type": "command",
                "command": python_hook_command(&script_path),
                "timeout": 10,
                "statusMessage": "loading user instructions",
            }
        }
    });

    fs::write(&script_path, script).context("write user instructions hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_global_instructions(home: &Path) -> Result<AbsolutePathBuf> {
    let path = home.join("AGENTS.md");
    fs::write(&path, GLOBAL_INSTRUCTIONS).context("write global AGENTS.md")?;
    Ok(path.abs())
}

fn instruction_fragment(request: &responses::ResponsesRequest) -> String {
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .expect("user instructions fragment")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_instructions_hook_replaces_global_instructions_and_keeps_project_docs() -> Result<()>
{
    // This test uses wiremock's loopback TCP endpoint, which is unavailable
    // when the Codex sandbox disables networking.
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source = write_global_instructions(home.path())?;
    write_user_instructions_hook(home.path(), HOOK_INSTRUCTIONS, /*exit_code*/ None)?;

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &PathUri::from_host_native_path(cwd.join("AGENTS.md"))?,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<_, anyhow::Error>(())
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build_with_auto_env(&server).await?;

    let started = tokio::time::timeout(Duration::from_secs(10), test.codex.next_event()).await??;
    let completed =
        tokio::time::timeout(Duration::from_secs(10), test.codex.next_event()).await??;
    let (started, completed) = match (started.msg, completed.msg) {
        (EventMsg::HookStarted(started), EventMsg::HookCompleted(completed)) => {
            (started, completed)
        }
        (started, completed) => panic!(
            "expected instruction-resolution hook events immediately after SessionConfigured, got {started:?} then {completed:?}"
        ),
    };
    assert_eq!(started.turn_id, None);
    assert_eq!(completed.turn_id, None);
    assert_eq!(started.run.id, completed.run.id);
    assert_eq!(started.run.event_name, HookEventName::UserInstructions);
    assert_eq!(completed.run.event_name, HookEventName::UserInstructions);
    assert_eq!(started.run.source_path, completed.run.source_path);
    assert_eq!(started.run.status, HookRunStatus::Running);
    assert_eq!(completed.run.status, HookRunStatus::Completed);

    let expected_warning = format!(
        "UserInstructions hook output overrides user-level instructions from `{}`.",
        global_source.display()
    );
    let warning = wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::Warning(warning) if warning.message == expected_warning),
    )
    .await;
    let EventMsg::Warning(warning) = warning else {
        unreachable!("wait_for_event matched a warning")
    };
    assert_eq!(warning.message, expected_warning);

    test.submit_turn("hello").await?;

    let contents = format!("{HOOK_INSTRUCTIONS}\n\n--- project-doc ---\n\n{PROJECT_INSTRUCTIONS}");
    let cwd = PathUri::from_abs_path(&test.config.cwd).inferred_native_path_string();
    assert_eq!(
        instruction_fragment(&response.single_request()),
        format!(
            "# AGENTS.md instructions for {cwd}\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>"
        )
    );

    let hook_source = home.path().join("hooks.json").abs();
    let project_source = test.config.cwd.join("AGENTS.md");
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![
            PathUri::from_abs_path(&hook_source),
            PathUri::from_abs_path(&project_source),
        ]
    );
    let hook_input: Value = serde_json::from_str(&fs::read_to_string(
        home.path().join("user_instructions_hook_input.json"),
    )?)?;
    assert_eq!(hook_input["hook_event_name"], "UserInstructions");
    assert_eq!(
        hook_input["session_id"],
        test.session_configured.session_id.to_string()
    );
    assert_eq!(
        hook_input["cwd"],
        PathUri::from_abs_path(&test.config.cwd).inferred_native_path_string()
    );
    assert_eq!(
        hook_input["transcript_path"],
        serde_json::json!(
            test.session_configured
                .rollout_path
                .as_ref()
                .map(|path| path.display().to_string())
        )
    );
    assert_eq!(hook_input["model"], test.session_configured.model);
    assert_eq!(hook_input["permission_mode"], "default");
    assert_eq!(hook_input.get("turn_id"), None);

    Ok(())
}

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    serde_json::from_slice::<Value>(&request.body).is_ok_and(|body| body.to_string().contains(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_context_subagent_reruns_user_instructions_hook() -> Result<()> {
    // This test uses wiremock's loopback TCP endpoint, which is unavailable
    // when the Codex sandbox disables networking.
    skip_if_no_network!(Ok(()));
    run_subagent_user_instructions_case(/*fork_context*/ false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_context_subagent_reruns_user_instructions_hook() -> Result<()> {
    // This test uses wiremock's loopback TCP endpoint, which is unavailable
    // when the Codex sandbox disables networking.
    skip_if_no_network!(Ok(()));
    run_subagent_user_instructions_case(/*fork_context*/ true).await
}

async fn run_subagent_user_instructions_case(fork_context: bool) -> Result<()> {
    const PARENT_PROMPT: &str = "spawn a worker to inspect the instructions";
    const CHILD_PROMPT: &str = "inspect the child instructions";
    const SPAWN_CALL_ID: &str = "spawn-user-instructions-child";

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&serde_json::json!({
        "message": CHILD_PROMPT,
        "fork_context": fork_context,
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, PARENT_PROMPT),
        sse(vec![
            ev_response_created("spawn-response"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "multi_agent_v1",
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("spawn-response"),
        ]),
    )
    .await;
    let child_response = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, CHILD_PROMPT) && !body_contains(request, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("child-response"),
            ev_assistant_message("child-message", "done"),
            ev_completed("child-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("spawn-follow-up-response"),
            ev_assistant_message("spawn-follow-up-message", "child started"),
            ev_completed("spawn-follow-up-response"),
        ]),
    )
    .await;

    let home = Arc::new(TempDir::new()?);
    write_user_instructions_hook(home.path(), HOOK_INSTRUCTIONS, /*exit_code*/ None)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(trust_discovered_hooks)
        .with_config(|config| {
            let _ = config.features.enable(Feature::Collab);
            let _ = config.features.disable(Feature::EnableRequestCompression);
        });
    let test = builder.build_with_auto_env(&server).await?;

    let mut created_threads = test.thread_manager.subscribe_thread_created();
    test.submit_turn(PARENT_PROMPT).await?;
    let child_thread_id =
        tokio::time::timeout(Duration::from_secs(10), created_threads.recv()).await??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;

    let child_request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(request) = child_response.last_request() {
                break request;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for the child model request"))?;
    assert_eq!(
        instruction_fragment(&child_request),
        format!("# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{HOOK_INSTRUCTIONS}\n</INSTRUCTIONS>")
    );
    assert_eq!(
        child_thread.instruction_sources().await,
        test.codex.instruction_sources().await
    );
    let hook_runs = fs::read_to_string(home.path().join("user_instructions_hook_input.json"))?;
    assert_eq!(hook_runs.lines().count(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_user_instructions_hook_falls_back_to_global_instructions() -> Result<()> {
    // This test uses wiremock's loopback TCP endpoint, which is unavailable
    // when the Codex sandbox disables networking.
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source = write_global_instructions(home.path())?;
    write_user_instructions_hook(home.path(), "", Some(1))?;

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(trust_discovered_hooks);
    let test = builder.build_with_auto_env(&server).await?;

    let started = tokio::time::timeout(Duration::from_secs(10), test.codex.next_event()).await??;
    let completed =
        tokio::time::timeout(Duration::from_secs(10), test.codex.next_event()).await??;
    let warning = tokio::time::timeout(Duration::from_secs(10), test.codex.next_event()).await??;
    let (started, completed, warning) = match (started.msg, completed.msg, warning.msg) {
        (
            EventMsg::HookStarted(started),
            EventMsg::HookCompleted(completed),
            EventMsg::Warning(warning),
        ) => (started, completed, warning),
        (started, completed, warning) => panic!(
            "expected failed UserInstructions hook lifecycle events, got {started:?}, {completed:?}, then {warning:?}"
        ),
    };
    assert_eq!(started.run.id, completed.run.id);
    assert_eq!(started.run.status, HookRunStatus::Running);
    assert_eq!(completed.run.status, HookRunStatus::Failed);
    let expected_warning = format!(
        "UserInstructions hook from {} failed: hook exited with code 1",
        PathUri::from_abs_path(&completed.run.source_path)
    );
    assert_eq!(warning.message, expected_warning);

    test.submit_turn("hello").await?;

    assert_eq!(
        instruction_fragment(&response.single_request()),
        format!(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{GLOBAL_INSTRUCTIONS}\n</INSTRUCTIONS>"
        )
    );
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&global_source)]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_instructions_hook_output_is_not_capped_at_project_doc_limit() -> Result<()> {
    // This test uses wiremock's loopback TCP endpoint, which is unavailable
    // when the Codex sandbox disables networking.
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let large_instructions = "uncapped user instruction ".repeat(2_000);
    write_user_instructions_hook(home.path(), &large_instructions, /*exit_code*/ None)?;

    let mut builder = test_codex()
        .with_home(home)
        .with_config(trust_discovered_hooks)
        .with_config(|config| config.project_doc_max_bytes = 1);
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("hello").await?;

    // `core/src/agents_md.rs::load_project_instructions` loads user instructions
    // before `read_agents_md` applies `project_doc_max_bytes` only to project docs.
    // Hook output replaces that user-level value, so it intentionally remains uncapped.
    assert_eq!(
        instruction_fragment(&response.single_request()),
        format!(
            "# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{}\n</INSTRUCTIONS>",
            large_instructions.trim()
        )
    );

    Ok(())
}
