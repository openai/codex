use std::collections::BTreeMap;
use std::io::BufReader;
use std::io::Read as _;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;

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
fn local_exec_server_exports_real_otel_metrics() -> Result<()> {
    let collector = TestCollector::start()?;
    let codex_home = TempDir::new()?;
    let base_url = &collector.base_url;
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
    let argv = vec!["cmd.exe", "/C", "exit", "0"];
    #[cfg(not(windows))]
    let argv = vec!["/usr/bin/true"];
    let mut command = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", codex_home.path())
        .args(["exec-server", "--listen", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
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
    )?;
    wait_for_response(&mut stdout, /*expected_id*/ 1)?;
    send_json_line(
        &mut stdin,
        &serde_json::json!({"method": "initialized", "params": {}}),
    )?;
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
    )?;
    wait_for_response(&mut stdout, /*expected_id*/ 2)?;
    send_json_line(
        &mut stdin,
        &serde_json::json!({
            "id": 3,
            "method": "process/read",
            "params": {
                "processId": "otel-process",
                "afterSeq": null,
                "maxBytes": null,
                "waitMs": 5_000
            }
        }),
    )?;
    wait_for_response(&mut stdout, /*expected_id*/ 3)?;
    drop(stdin);
    let mut remaining_stdout = String::new();
    stdout.read_to_string(&mut remaining_stdout)?;
    let status = child.wait()?;
    anyhow::ensure!(
        status.success(),
        "exec-server exited with {status}; remaining stdout: {remaining_stdout}"
    );

    let requests = collector.finish_after_exec_server_metrics(Duration::from_secs(10))?;
    let metrics = parse_metric_exports(&requests)?;
    assert_metric_point(
        &metrics,
        "exec_server_connections_active",
        &[("transport", "stdio")],
        Some(0),
    );
    assert_metric_point(
        &metrics,
        "exec_server_connections_total",
        &[("transport", "stdio"), ("result", "accepted")],
        Some(1),
    );
    assert_metric_point(
        &metrics,
        "exec_server_requests_total",
        &[("method", "initialize"), ("result", "success")],
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
        &[("result", "success")],
        Some(1),
    );
    assert_metric_point(
        &metrics,
        "exec_server_request_duration_seconds",
        &[("method", "process/start"), ("result", "success")],
        None,
    );
    assert_metric_point(
        &metrics,
        "exec_server_process_duration_seconds",
        &[("result", "success")],
        None,
    );
    Ok(())
}

fn send_json_line(stdin: &mut impl std::io::Write, message: &serde_json::Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, message)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn wait_for_response(stdout: &mut impl std::io::BufRead, expected_id: i64) -> Result<()> {
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line)? == 0 {
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

#[derive(Debug)]
struct MetricPoint {
    name: String,
    attributes: BTreeMap<String, String>,
    value: Option<i64>,
}

fn parse_metric_exports(requests: &[CapturedRequest]) -> Result<Vec<MetricPoint>> {
    let mut points = Vec::new();
    for request in requests
        .iter()
        .filter(|request| request.path == "/v1/metrics")
    {
        let payload: serde_json::Value = serde_json::from_str(&request.body)?;
        let Some(resource_metrics) = payload["resourceMetrics"].as_array() else {
            continue;
        };
        for resource in resource_metrics {
            let Some(scope_metrics) = resource["scopeMetrics"].as_array() else {
                continue;
            };
            for scope in scope_metrics {
                let Some(metrics) = scope["metrics"].as_array() else {
                    continue;
                };
                for metric in metrics {
                    let Some(name) = metric["name"].as_str() else {
                        continue;
                    };
                    let data_points = ["gauge", "sum", "histogram"]
                        .into_iter()
                        .find_map(|kind| metric[kind]["dataPoints"].as_array());
                    let Some(data_points) = data_points else {
                        continue;
                    };
                    for point in data_points {
                        let attributes = point["attributes"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|attribute| {
                                Some((
                                    attribute["key"].as_str()?.to_string(),
                                    attribute["value"]["stringValue"].as_str()?.to_string(),
                                ))
                            })
                            .collect();
                        let value = point["asInt"]
                            .as_i64()
                            .or_else(|| point["asInt"].as_str()?.parse().ok());
                        points.push(MetricPoint {
                            name: name.to_string(),
                            attributes,
                            value,
                        });
                    }
                }
            }
        }
    }
    Ok(points)
}

fn assert_metric_point(
    points: &[MetricPoint],
    name: &str,
    attributes: &[(&str, &str)],
    value: Option<i64>,
) {
    assert!(
        points.iter().any(|point| {
            point.name == name
                && point.value == value
                && attributes.iter().all(|(key, value)| {
                    point.attributes.get(*key).map(String::as_str) == Some(*value)
                })
        }),
        "metric {name} with attributes {attributes:?} and value {value:?} missing from {points:#?}"
    );
}

struct CapturedRequest {
    path: String,
    body: String,
}

struct TestCollector {
    base_url: String,
    requests: mpsc::Receiver<CapturedRequest>,
    stop: mpsc::Sender<()>,
    server: thread::JoinHandle<()>,
}

impl TestCollector {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (tx, requests) = mpsc::channel();
        let (stop, stop_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if let Ok(request) = read_http_request(&mut stream)
                            && tx.send(request).is_err()
                        {
                            break;
                        }
                        let _ = stream.write_all(
                            b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        if stop_rx.try_recv().is_ok() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            base_url: format!("http://{addr}"),
            requests,
            stop,
            server,
        })
    }

    fn finish_after_exec_server_metrics(self, timeout: Duration) -> Result<Vec<CapturedRequest>> {
        let deadline = Instant::now() + timeout;
        let mut requests = Vec::new();
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            match self.requests.recv_timeout(remaining) {
                Ok(request) => {
                    requests.push(request);
                    let metrics = requests
                        .iter()
                        .filter(|request| request.path == "/v1/metrics")
                        .map(|request| request.body.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let found = metrics.contains("exec_server_connections_active")
                        && metrics.contains("exec_server_requests_total")
                        && metrics.contains("exec_server_processes_active")
                        && metrics.contains("exec_server_processes_finished_total")
                        && metrics.contains("process/start")
                        && metrics.contains("success");
                    if found {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let _ = self.stop.send(());
        self.server
            .join()
            .map_err(|_| anyhow::anyhow!("collector thread panicked"))?;
        while let Ok(request) = self.requests.try_recv() {
            requests.push(request);
        }
        Ok(requests)
    }
}

fn read_http_request(stream: &mut std::net::TcpStream) -> std::io::Result<CapturedRequest> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    let mut scratch = [0_u8; 8192];
    let header_end = loop {
        let read = stream.read(&mut scratch)?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "request closed before headers",
            ));
        }
        bytes.extend_from_slice(&scratch[..read]);
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break header_end;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = headers.split("\r\n");
    let path = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or_default()
        .to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or_default();
    let mut body = bytes[header_end + 4..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut scratch)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&scratch[..read]);
    }
    body.truncate(content_length);
    Ok(CapturedRequest {
        path,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}
