use std::io::Read as _;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::Path;
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
fn remote_exec_server_preserves_websocket_error_in_stderr() -> Result<()> {
    let failed_websocket_listener = TcpListener::bind("127.0.0.1:0")?;
    let failed_websocket_url = format!("ws://{}", failed_websocket_listener.local_addr()?);
    drop(failed_websocket_listener);

    let registry = TestRegistry::start(&failed_websocket_url)?;
    let codex_home = TempDir::new()?;
    let stderr_path = codex_home.path().join("remote.stderr");
    let stderr_file = std::fs::File::create(&stderr_path)?;
    let mut cmd = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home.path())
        .env("CODEX_API_KEY", "test-key")
        .env("RUST_LOG", "codex_exec_server=warn")
        .args([
            "exec-server",
            "--remote",
            &registry.base_url,
            "--environment-id",
            "env-test",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(stderr_file);

    let mut child = cmd.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let stderr = loop {
        let stderr = std::fs::read_to_string(&stderr_path)?;
        if stderr.contains("failed to connect remote exec-server websocket")
            || Instant::now() >= deadline
        {
            break stderr;
        }
        thread::sleep(Duration::from_millis(50));
    };
    let _ = child.kill();
    child.wait()?;
    registry.finish()?;

    assert!(
        stderr.contains("failed to connect remote exec-server websocket"),
        "{stderr}"
    );
    assert!(stderr.contains("environment_id env-test"), "{stderr}");
    assert!(stderr.contains("IO error"), "{stderr}");

    Ok(())
}

struct CapturedRequest {
    path: String,
    body: String,
}

struct TestRegistry {
    base_url: String,
    server: thread::JoinHandle<Result<()>>,
}

impl TestRegistry {
    fn start(websocket_url: &str) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let websocket_url = websocket_url.to_string();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => return Err(err.into()),
                }
            };
            let request = read_http_request(&mut stream)?;
            assert_eq!(request.path, "/cloud/environment/env-test/register");
            assert!(request.body.is_empty());
            let body = format!(r#"{{"environment_id":"env-test","url":"{websocket_url}"}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes())?;
            Ok(())
        });
        Ok(Self {
            base_url: format!("http://{addr}"),
            server,
        })
    }

    fn finish(self) -> Result<()> {
        self.server
            .join()
            .map_err(|_| anyhow::anyhow!("registry thread panicked"))?
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
