//! Exercises a real `responses-api-proxy` process with request dumping enabled, then verifies that
//! parent and spawned subagent requests carry the expected window, parent-thread, and subagent
//! identity headers in the dumped Responses API requests.

use anyhow::Result;
use anyhow::anyhow;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;

const PARENT_PROMPT: &str = "spawn a subagent and report when it is started";
const CHILD_PROMPT: &str = "child: say done";
const SPAWN_CALL_ID: &str = "spawn-call-1";
const PROXY_START_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const PROXY_POLL_INTERVAL: Duration = Duration::from_millis(/*millis*/ 20);
const TURN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);

struct ResponsesApiProxy {
    child: Child,
    port: u16,
}

impl ResponsesApiProxy {
    fn start(upstream_url: &str, dump_dir: &Path) -> Result<Self> {
        let server_info = dump_dir.join("server-info.json");
        let (proxy_program, use_codex_multitool) =
            match codex_utils_cargo_bin::cargo_bin("codex-responses-api-proxy") {
                Ok(path) => (path, false),
                Err(_) => (codex_utils_cargo_bin::cargo_bin("codex")?, true),
            };
        let mut command = StdCommand::new(proxy_program);
        if use_codex_multitool {
            command.arg("responses-api-proxy");
        }
        let mut child = command
            .args(["--server-info"])
            .arg(&server_info)
            .args(["--upstream-url", upstream_url, "--dump-dir"])
            .arg(dump_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("responses-api-proxy stdin was not piped"))?
            .write_all(b"dummy\n")?;

        let deadline = Instant::now() + PROXY_START_TIMEOUT;
        loop {
            match std::fs::read_to_string(&server_info) {
                Ok(info) => {
                    if !info.trim().is_empty() {
                        match serde_json::from_str::<Value>(&info) {
                            Ok(info) => {
                                let port = info
                                    .get("port")
                                    .and_then(Value::as_u64)
                                    .ok_or_else(|| anyhow!("proxy server info missing port"))?;
                                return Ok(Self {
                                    child,
                                    port: u16::try_from(port)?,
                                });
                            }
                            Err(err) if err.is_eof() => {}
                            Err(err) => return Err(err.into()),
                        }
                    }
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
            if let Some(status) = child.try_wait()? {
                return Err(anyhow!(
                    "responses-api-proxy exited before writing server info: {status}"
                ));
            }
            if Instant::now() >= deadline {
                return Err(anyhow!("timed out waiting for responses-api-proxy"));
            }
            std::thread::sleep(PROXY_POLL_INTERVAL);
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }
}

impl Drop for ResponsesApiProxy {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_api_proxy_dumps_parent_and_subagent_identity_headers() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let dump_dir = TempDir::new()?;
    let proxy =
        ResponsesApiProxy::start(&format!("{}/v1/responses", server.uri()), dump_dir.path())?;

    let spawn_args = serde_json::to_string(&json!({ "message": CHILD_PROMPT }))?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| request_body_contains(req, PARENT_PROMPT),
        sse(vec![
            ev_response_created("resp-parent-1"),
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            request_body_contains(req, CHILD_PROMPT) && !request_body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| request_body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "parent done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;

    let proxy_base_url = proxy.base_url();
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider.base_url = Some(proxy_base_url);
        config
            .features
            .disable(Feature::EnableRequestCompression)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    submit_turn_with_timeout(&test, PARENT_PROMPT, dump_dir.path()).await?;

    let dumps = wait_for_proxy_request_dumps(dump_dir.path())?;
    let parent = dumps
        .iter()
        .find(|dump| dump_body_contains(dump, PARENT_PROMPT))
        .ok_or_else(|| anyhow!("missing parent request dump"))?;
    let child = dumps
        .iter()
        .find(|dump| {
            dump_body_contains(dump, CHILD_PROMPT) && !dump_body_contains(dump, SPAWN_CALL_ID)
        })
        .ok_or_else(|| anyhow!("missing child request dump"))?;

    let parent_window_id = header(parent, "x-codex-window-id")
        .ok_or_else(|| anyhow!("parent request missing x-codex-window-id"))?;
    let child_window_id = header(child, "x-codex-window-id")
        .ok_or_else(|| anyhow!("child request missing x-codex-window-id"))?;
    let (parent_thread_id, parent_generation) = split_window_id(parent_window_id)?;
    let (child_thread_id, child_generation) = split_window_id(child_window_id)?;

    assert_eq!(parent_generation, 0);
    assert_eq!(child_generation, 0);
    assert!(child_thread_id != parent_thread_id);
    assert_eq!(header(parent, "x-openai-subagent"), None);
    assert_eq!(header(child, "x-openai-subagent"), Some("collab_spawn"));
    assert_eq!(
        header(child, "x-codex-parent-thread-id"),
        Some(parent_thread_id)
    );

    Ok(())
}

async fn submit_turn_with_timeout(test: &TestCodex, prompt: &str, dump_dir: &Path) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.config.cwd.to_path_buf(),
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                read_only_access: Default::default(),
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let turn_started = wait_for_event_result(test, "turn started", dump_dir, |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await?;
    let EventMsg::TurnStarted(turn_started) = turn_started else {
        unreachable!("event predicate only matches turn started events");
    };
    wait_for_event_result(test, "turn complete", dump_dir, |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_started.turn_id,
        _ => false,
    })
    .await?;

    Ok(())
}

async fn wait_for_event_result<F>(
    test: &TestCodex,
    stage: &str,
    dump_dir: &Path,
    mut predicate: F,
) -> Result<EventMsg>
where
    F: FnMut(&EventMsg) -> bool,
{
    let mut seen_events = Vec::new();
    tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            let event = test.codex.next_event().await?;
            seen_events.push(event_summary(&event.msg));
            if predicate(&event.msg) {
                return Ok::<EventMsg, anyhow::Error>(event.msg);
            }
        }
    })
    .await
    .map_err(|_| {
        anyhow!(
            "timed out waiting for {stage}; saw events: {}; {}",
            seen_events.join(" | "),
            proxy_dump_summary(dump_dir)
        )
    })?
}

fn event_summary(event: &EventMsg) -> String {
    let mut summary = format!("{event:?}");
    summary.truncate(240);
    summary
}

fn request_body_contains(req: &wiremock::Request, text: &str) -> bool {
    std::str::from_utf8(&req.body).is_ok_and(|body| body.contains(text))
}

fn wait_for_proxy_request_dumps(dump_dir: &Path) -> Result<Vec<Value>> {
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 2);
    loop {
        let dumps = read_proxy_request_dumps(dump_dir).unwrap_or_default();
        if dumps.len() >= 3
            && dumps
                .iter()
                .any(|dump| dump_body_contains(dump, CHILD_PROMPT))
        {
            return Ok(dumps);
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for proxy request dumps, got {}",
                dumps.len()
            ));
        }
        std::thread::sleep(PROXY_POLL_INTERVAL);
    }
}

fn read_proxy_request_dumps(dump_dir: &Path) -> Result<Vec<Value>> {
    let mut dumps = Vec::new();
    for entry in std::fs::read_dir(dump_dir)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("-request.json"))
        {
            let contents = std::fs::read_to_string(&path)?;
            if contents.trim().is_empty() {
                continue;
            }

            match serde_json::from_str(&contents) {
                Ok(dump) => dumps.push(dump),
                Err(err) if err.is_eof() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }
    Ok(dumps)
}

fn proxy_dump_summary(dump_dir: &Path) -> String {
    match read_proxy_request_dumps(dump_dir) {
        Ok(dumps) => {
            let bodies = dumps
                .iter()
                .filter_map(|dump| dump.get("body"))
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            format!("proxy wrote {} request dumps: {bodies}", dumps.len())
        }
        Err(err) => format!("failed to read proxy request dumps: {err}"),
    }
}

#[test]
fn read_proxy_request_dumps_ignores_in_progress_files() -> Result<()> {
    let dump_dir = TempDir::new()?;
    std::fs::write(dump_dir.path().join("empty-request.json"), "")?;
    std::fs::write(dump_dir.path().join("partial-request.json"), "{\"body\"")?;
    std::fs::write(
        dump_dir.path().join("complete-request.json"),
        serde_json::to_string(&json!({ "body": "ready" }))?,
    )?;

    assert_eq!(
        read_proxy_request_dumps(dump_dir.path())?,
        vec![json!({ "body": "ready" })]
    );

    Ok(())
}

fn dump_body_contains(dump: &Value, text: &str) -> bool {
    dump.get("body")
        .is_some_and(|body| body.to_string().contains(text))
}

fn header<'a>(dump: &'a Value, name: &str) -> Option<&'a str> {
    dump.get("headers")?.as_array()?.iter().find_map(|header| {
        (header.get("name")?.as_str()?.eq_ignore_ascii_case(name))
            .then(|| header.get("value")?.as_str())
            .flatten()
    })
}

fn split_window_id(window_id: &str) -> Result<(&str, u64)> {
    let (thread_id, generation) = window_id
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("invalid window id header: {window_id}"))?;
    Ok((thread_id, generation.parse::<u64>()?))
}
