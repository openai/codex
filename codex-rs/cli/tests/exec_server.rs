#[cfg(unix)]
use std::io::BufRead as _;
#[cfg(unix)]
use std::io::BufReader as StdBufReader;
#[cfg(unix)]
use std::io::Read as _;
#[cfg(unix)]
use std::io::Write as _;
#[cfg(unix)]
use std::net::TcpStream;
use std::path::Path;
use std::process::Stdio;
#[cfg(unix)]
use std::thread;
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use anyhow::Result;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn strict_config_rejects_unknown_config_fields_for_exec_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
foo = "bar"
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args([
        "exec-server",
        "--strict-config",
        "--listen",
        "http://127.0.0.1:0",
    ])
    .assert()
    .failure()
    .stderr(contains("unknown configuration field"));

    Ok(())
}

#[test]
fn local_exec_server_ignores_invalid_config_without_strict_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(codex_home.path().join("config.toml"), "not valid toml = [")?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["exec-server", "--listen", "stdio"])
        .assert()
        .success()
        .stderr(contains("not valid toml").not());

    Ok(())
}

#[tokio::test]
async fn local_exec_server_flushes_telemetry_on_stdio_disconnect() -> Result<()> {
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/metrics"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&collector)
        .await;
    let codex_home = TempDir::new()?;
    let base_url = collector.uri();
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
[analytics]
enabled = true

[otel]
environment = "test"
metrics_exporter = {{ otlp-http = {{ endpoint = "{base_url}/v1/metrics", protocol = "json" }} }}
"#
        ),
    )?;

    let cwd = url::Url::from_directory_path(std::env::current_dir()?)
        .map_err(|()| anyhow::anyhow!("could not convert cwd to file URL"))?;
    #[cfg(windows)]
    let argv = vec!["ping.exe", "-n", "61", "127.0.0.1"];
    #[cfg(not(windows))]
    let argv = vec!["/bin/sleep", "60"];
    let codex_bin = codex_utils_cargo_bin::cargo_bin("codex")?;
    let codex_home = codex_home.path().to_path_buf();
    let subprocess = async move {
        let mut command = tokio::process::Command::new(codex_bin);
        command
            .env("CODEX_HOME", codex_home)
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .args(["exec-server", "--listen", "stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("exec-server stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("exec-server stdout was not piped"))?;
        let mut stdout = BufReader::new(stdout);
        send_json_line(
            &mut stdin,
            &serde_json::json!({
                "id": 1,
                "method": "initialize",
                "params": {"clientName": "otel-test", "resumeSessionId": null}
            }),
        )
        .await?;
        wait_for_response(&mut stdout, /*expected_id*/ 1).await?;
        send_json_line(
            &mut stdin,
            &serde_json::json!({"method": "initialized", "params": {}}),
        )
        .await?;
        send_json_line(
            &mut stdin,
            &serde_json::json!({
                "id": 2,
                "method": "process/start",
                "params": {
                    "processId": "otel-process",
                    "argv": argv,
                    "cwd": cwd,
                    "env": {},
                    "tty": false,
                    "pipeStdin": false,
                    "arg0": null
                }
            }),
        )
        .await?;
        wait_for_response(&mut stdout, /*expected_id*/ 2).await?;
        drop(stdin);
        let mut remaining_stdout = String::new();
        stdout.read_to_string(&mut remaining_stdout).await?;
        let status = child.wait().await?;
        anyhow::ensure!(
            status.success(),
            "exec-server exited with {status}; remaining stdout: {remaining_stdout}"
        );
        Ok::<(), anyhow::Error>(())
    };
    let subprocess_result = tokio::time::timeout(Duration::from_secs(30), subprocess)
        .await
        .map_err(|_| anyhow::anyhow!("exec-server subprocess timed out"))?;
    subprocess_result?;

    let requests = collector
        .received_requests()
        .await
        .ok_or_else(|| anyhow::anyhow!("failed to read OTLP collector requests"))?;
    let metrics = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/metrics")
        .map(|request| serde_json::from_slice::<serde_json::Value>(&request.body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_metric_point(
        &metrics,
        "exec_server_connections_active",
        &[("transport", "stdio")],
        Some(0),
    );
    assert_metric_point(
        &metrics,
        "exec_server_connections_total",
        &[("transport", "stdio")],
        Some(1),
    );
    assert_metric_point(
        &metrics,
        "exec_server_requests_total",
        &[("method", "process/start"), ("result", "success")],
        Some(1),
    );
    assert_metric_point(&metrics, "exec_server_processes_active", &[], Some(0));
    assert_metric_point(
        &metrics,
        "exec_server_processes_finished_total",
        &[("result", "terminated")],
        Some(1),
    );
    assert_metric_point(
        &metrics,
        "exec_server_request_duration_seconds",
        &[("method", "process/start"), ("result", "success")],
        /*value*/ None,
    );
    assert_metric_point(
        &metrics,
        "exec_server_process_duration_seconds",
        &[("result", "terminated")],
        /*value*/ None,
    );
    Ok(())
}

async fn send_json_line(
    stdin: &mut (impl tokio::io::AsyncWrite + Unpin),
    message: &serde_json::Value,
) -> Result<()> {
    let mut encoded = serde_json::to_vec(message)?;
    encoded.push(b'\n');
    stdin.write_all(&encoded).await?;
    stdin.flush().await?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn local_exec_server_exits_successfully_on_sigterm() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut child = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut listen_url = String::new();
    StdBufReader::new(child.stdout.take().expect("child stdout")).read_line(&mut listen_url)?;
    assert!(listen_url.starts_with("ws://127.0.0.1:"), "{listen_url}");

    let listen_addr = listen_url
        .trim()
        .strip_prefix("ws://")
        .expect("listen URL should use ws://")
        .parse()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        if let Ok(mut stream) =
            TcpStream::connect_timeout(&listen_addr, remaining.min(Duration::from_millis(100)))
        {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let request =
                format!("GET /readyz HTTP/1.1\r\nHost: {listen_addr}\r\nConnection: close\r\n\r\n");
            let mut response = String::new();
            if stream.write_all(request.as_bytes()).is_ok()
                && stream.read_to_string(&mut response).is_ok()
                && response.starts_with("HTTP/1.1 200")
            {
                ready = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready, "exec-server did not become ready at {listen_url}");

    // SAFETY: `child.id()` is the live process spawned above.
    let result = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
    assert_eq!(result, 0);
    let status = child.wait()?;
    assert!(status.success(), "{status}");
    Ok(())
}

async fn wait_for_response(
    stdout: &mut (impl tokio::io::AsyncBufRead + Unpin),
    expected_id: i64,
) -> Result<()> {
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).await? == 0 {
            anyhow::bail!("exec-server stdout closed before response {expected_id}");
        }
        let message: serde_json::Value = serde_json::from_str(&line)?;
        if message["id"].as_i64() == Some(expected_id) {
            anyhow::ensure!(
                message.get("error").is_none(),
                "exec-server request {expected_id} failed: {message}"
            );
            return Ok(());
        }
    }
}

fn assert_metric_point(
    payloads: &[serde_json::Value],
    name: &str,
    attributes: &[(&str, &str)],
    value: Option<i64>,
) {
    let found = payloads
        .iter()
        .flat_map(|payload| payload["resourceMetrics"].as_array().into_iter().flatten())
        .flat_map(|resource| resource["scopeMetrics"].as_array().into_iter().flatten())
        .flat_map(|scope| scope["metrics"].as_array().into_iter().flatten())
        .filter(|metric| metric["name"].as_str() == Some(name))
        .flat_map(|metric| {
            ["gauge", "sum", "histogram"]
                .into_iter()
                .find_map(|kind| metric[kind]["dataPoints"].as_array())
                .into_iter()
                .flatten()
        })
        .any(|point| {
            let actual_attributes = point["attributes"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            let attributes_match = actual_attributes.len() == attributes.len()
                && attributes.iter().all(|(expected_key, expected_value)| {
                    actual_attributes.iter().any(|actual| {
                        actual["key"].as_str() == Some(*expected_key)
                            && actual["value"]["stringValue"].as_str() == Some(*expected_value)
                    })
                });
            let actual_value = point["asInt"]
                .as_i64()
                .or_else(|| point["asInt"].as_str()?.parse().ok());
            attributes_match && value.is_none_or(|expected| actual_value == Some(expected))
        });
    assert!(
        found,
        "metric {name} with attributes {attributes:?} and value {value:?} missing"
    );
}
