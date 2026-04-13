//! Exercises a real `responses-api-proxy` process with request dumping enabled, then verifies that
//! parent and spawned subagent requests carry the expected window, parent-thread, and subagent
//! identity headers in the dumped Responses API requests.

use anyhow::Result;
use anyhow::anyhow;
use codex_features::Feature;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
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
const PROXY_START_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
const PROXY_POLL_INTERVAL: Duration = Duration::from_millis(/*millis*/ 20);

struct ResponsesApiProxy {
    child: Child,
    port: u16,
}

impl ResponsesApiProxy {
    fn start(upstream_url: &str, dump_dir: &Path) -> Result<Self> {
        let server_info = dump_dir.join("server-info.json");
        let mut child = StdCommand::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .args(["responses-api-proxy", "--server-info"])
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
            if let Ok(info) = std::fs::read_to_string(&server_info) {
                let port = serde_json::from_str::<Value>(&info)?
                    .get("port")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| anyhow!("proxy server info missing port"))?;
                return Ok(Self {
                    child,
                    port: u16::try_from(port)?,
                });
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
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .disable(Feature::EnableRequestCompression)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    test.submit_turn(PARENT_PROMPT).await?;

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

fn request_body_contains(req: &wiremock::Request, text: &str) -> bool {
    let is_zstd = req
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    let bytes = if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&req.body)).ok()
    } else {
        Some(req.body.clone())
    };
    bytes
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
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
