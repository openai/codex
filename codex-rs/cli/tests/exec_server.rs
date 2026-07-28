use std::collections::HashMap;
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

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerClient;
use codex_exec_server::NoiseChannelIdentity;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectArgs;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::ProcessId;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::SinkExt;
use futures::StreamExt;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
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

#[test]
fn local_exec_server_allows_disabled_parent_lifetime_environment_variable() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut cmd = codex_command(codex_home.path())?;
    cmd.env(
        codex_exec_server::CODEX_EXEC_SERVER_EXIT_ON_STDIN_CLOSE_ENV_VAR,
        "false",
    )
    .args(["exec-server", "--listen", "stdio"])
    .assert()
    .success();

    Ok(())
}

#[tokio::test]
async fn remote_exec_server_gracefully_stops_when_parent_stdin_closes() -> Result<()> {
    const ENVIRONMENT_ID: &str = "environment-parent-lifetime";
    const EXECUTOR_REGISTRATION_ID: &str = "registration-parent-lifetime";
    const TEST_TIMEOUT: Duration = Duration::from_secs(30);

    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/metrics"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&collector)
        .await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", listener.local_addr()?);
    let registry = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/register"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": ENVIRONMENT_ID,
            "url": format!("{rendezvous_url}/relay?role=environment"),
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": EXECUTOR_REGISTRATION_ID,
        })))
        .mount(&registry)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/validate"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": true,
        })))
        .mount(&registry)
        .await;

    let codex_home = TempDir::new()?;
    let collector_url = collector.uri();
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
[analytics]
enabled = true

[otel]
environment = "test"
metrics_exporter = {{ otlp-http = {{ endpoint = "{collector_url}/v1/metrics", protocol = "json" }} }}
"#
        ),
    )?;
    let mut command = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_API_KEY", "test-api-key")
        .env(
            codex_exec_server::CODEX_EXEC_SERVER_EXIT_ON_STDIN_CLOSE_ENV_VAR,
            "true",
        )
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .args([
            "exec-server",
            "--remote",
            &registry.uri(),
            "--environment-id",
            ENVIRONMENT_ID,
        ])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("remote exec-server stdin was not piped"))?;

    let environment_websocket = accept_parent_lifetime_websocket(&listener, TEST_TIMEOUT).await?;
    let executor_public_key = registered_parent_lifetime_executor_public_key(&registry).await?;
    let harness_args = NoiseRendezvousConnectArgs {
        bundle: NoiseRendezvousConnectBundle {
            websocket_url: format!("{rendezvous_url}/relay?role=harness"),
            environment_id: ENVIRONMENT_ID.to_string(),
            executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
            executor_public_key,
            harness_key_authorization: "parent-lifetime-harness".to_string(),
        },
        harness_identity: NoiseChannelIdentity::generate()?,
        client_name: "parent-lifetime-test".to_string(),
        connect_timeout: TEST_TIMEOUT,
        initialize_timeout: TEST_TIMEOUT,
        resume_session_id: None,
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    };
    let client_task =
        tokio::spawn(async move { ExecServerClient::connect_noise_rendezvous(harness_args).await });
    let harness_websocket = accept_parent_lifetime_websocket(&listener, TEST_TIMEOUT).await?;
    let relay_task = tokio::spawn(proxy_parent_lifetime_relay(
        environment_websocket,
        harness_websocket,
    ));
    let client = tokio::time::timeout(TEST_TIMEOUT, client_task)
        .await
        .context("remote harness did not connect")???;

    #[cfg(windows)]
    let argv = vec![
        "cmd.exe",
        "/C",
        "if defined CODEX_EXEC_SERVER_EXIT_ON_STDIN_CLOSE (exit /b 1) else ping -n 61 127.0.0.1",
    ];
    #[cfg(not(windows))]
    let argv = vec![
        "/bin/sh",
        "-c",
        "[ -z \"${CODEX_EXEC_SERVER_EXIT_ON_STDIN_CLOSE+present}\" ] && exec /bin/sleep 60",
    ];
    let cwd = url::Url::from_directory_path(std::env::current_dir()?)
        .map_err(|()| anyhow::anyhow!("could not convert cwd to file URL"))?;
    client
        .exec(ExecParams {
            process_id: ProcessId::from("parent-lifetime-process"),
            argv: argv.into_iter().map(str::to_string).collect(),
            cwd: cwd.as_str().parse()?,
            env_policy: Some(codex_exec_server::ExecEnvPolicy {
                inherit: codex_protocol::config_types::ShellEnvironmentPolicyInherit::All,
                ignore_default_excludes: false,
                exclude: Vec::new(),
                r#set: HashMap::new(),
                include_only: Vec::new(),
            }),
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    anyhow::ensure!(
        child.try_wait()?.is_none(),
        "remote exec-server exited while its parent stdin remained open"
    );

    drop(stdin);

    let output = tokio::time::timeout(TEST_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("remote exec-server did not exit after stdin closed"))??;
    anyhow::ensure!(
        output.status.success(),
        "remote exec-server exited with {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    relay_task.abort();
    let _ = relay_task.await;

    let requests = collector
        .received_requests()
        .await
        .ok_or_else(|| anyhow::anyhow!("failed to read OTLP collector requests"))?;
    let metrics = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/metrics")
        .map(|request| serde_json::from_slice::<serde_json::Value>(&request.body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_metric_point(&metrics, "exec_server_processes_active", &[], Some(0));
    assert_metric_point(
        &metrics,
        "exec_server_processes_finished_total",
        &[("result", "terminated")],
        Some(1),
    );
    assert_metric_point(
        &metrics,
        "exec_server_requests_total",
        &[("method", "process/start"), ("result", "success")],
        Some(1),
    );

    Ok(())
}

async fn accept_parent_lifetime_websocket(
    listener: &TcpListener,
    timeout: Duration,
) -> Result<WebSocketStream<tokio::net::TcpStream>> {
    let (socket, _) = tokio::time::timeout(timeout, listener.accept())
        .await
        .context("remote executor or harness did not reach rendezvous")??;
    tokio::time::timeout(timeout, accept_async(socket))
        .await
        .context("fake rendezvous did not complete websocket handshake")?
        .map_err(Into::into)
}

async fn registered_parent_lifetime_executor_public_key(
    registry: &MockServer,
) -> Result<NoiseChannelPublicKey> {
    let requests = registry
        .received_requests()
        .await
        .context("failed to read remote registry requests")?;
    let request = requests
        .iter()
        .find(|request| request.url.path().ends_with("/register"))
        .context("remote executor did not register before connecting")?;
    let body: serde_json::Value = serde_json::from_slice(&request.body)?;
    serde_json::from_value(body["executor_public_key"].clone()).map_err(Into::into)
}

async fn proxy_parent_lifetime_relay(
    mut environment: WebSocketStream<tokio::net::TcpStream>,
    mut harness: WebSocketStream<tokio::net::TcpStream>,
) -> Result<()> {
    loop {
        tokio::select! {
            message = environment.next() => {
                let Some(message) = message else {
                    break;
                };
                harness.send(message?).await?;
            }
            message = harness.next() => {
                let Some(message) = message else {
                    break;
                };
                environment.send(message?).await?;
            }
        }
    }
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
