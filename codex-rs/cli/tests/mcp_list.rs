use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::load_global_mcp_servers;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use serde_json::json;
use tempfile::TempDir;
#[cfg(target_os = "macos")]
use wiremock::Mock;
#[cfg(target_os = "macos")]
use wiremock::MockServer;
#[cfg(target_os = "macos")]
use wiremock::ResponseTemplate;
#[cfg(target_os = "macos")]
use wiremock::matchers::method;
#[cfg(target_os = "macos")]
use wiremock::matchers::path;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

async fn configure_http_oauth_server(codex_home: &Path, url: &str) -> Result<()> {
    let mut servers = load_global_mcp_servers(codex_home).await?;
    servers.insert(
        "oauth".to_string(),
        toml::from_str(&format!("url = \"{url}\""))?,
    );
    ConfigEditsBuilder::new(codex_home)
        .replace_mcp_servers(&servers)
        .apply_blocking()?;
    Ok(())
}

#[test]
fn list_shows_empty_state() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.args(["mcp", "list"]).output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("No MCP servers configured yet."));

    Ok(())
}

#[tokio::test]
async fn list_discovers_local_oauth_server_through_environment_proxy() -> Result<()> {
    let codex_home = TempDir::new()?;
    configure_http_oauth_server(codex_home.path(), "http://mcp-proxy.invalid/mcp").await?;

    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let proxy_url = format!("http://{}", listener.local_addr()?);
    let proxy = std::thread::spawn(move || -> Result<Vec<String>> {
        let resource_metadata = json!({
            "resource": "http://mcp-proxy.invalid/mcp",
            "authorization_servers": ["http://mcp-proxy.invalid"],
        })
        .to_string();
        let authorization_metadata = json!({
            "authorization_endpoint": "https://oauth.example/authorize",
            "token_endpoint": "https://oauth.example/token",
        })
        .to_string();
        let responses = [
            concat!(
                "HTTP/1.1 401 Unauthorized\r\n",
                "www-authenticate: Bearer resource_metadata=\"http://mcp-proxy.invalid/oauth-resource\"\r\n",
                "content-length: 0\r\n",
                "connection: close\r\n\r\n"
            )
            .to_string(),
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{resource_metadata}",
                resource_metadata.len()
            ),
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{authorization_metadata}",
                authorization_metadata.len()
            ),
        ];

        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        anyhow::ensure!(
                            Instant::now() < deadline,
                            "proxy did not receive OAuth discovery request {}",
                            requests.len() + 1
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let bytes_read = stream.read(&mut buffer)?;
                anyhow::ensure!(bytes_read > 0, "proxy request ended before its headers");
                request.extend_from_slice(&buffer[..bytes_read]);
                anyhow::ensure!(
                    request.len() <= 64 * 1024,
                    "proxy request headers are too large"
                );
            }
            let request = String::from_utf8(request)?;
            requests.push(request.lines().next().unwrap_or_default().to_string());
            stream.write_all(response.as_bytes())?;
        }

        Ok(requests)
    });

    let mut command = codex_command(codex_home.path())?;
    command
        .env("HTTP_PROXY", &proxy_url)
        .env("http_proxy", &proxy_url)
        .env_remove("HTTPS_PROXY")
        .env_remove("https_proxy")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .args([
            "-c",
            "mcp_oauth_credentials_store=\"file\"",
            "mcp",
            "list",
            "--json",
        ]);
    let output = command.output()?;
    assert!(
        output.status.success(),
        "mcp list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let proxy_requests = proxy
        .join()
        .expect("OAuth discovery proxy thread should finish")?;

    assert_eq!(
        proxy_requests,
        vec![
            "GET http://mcp-proxy.invalid/mcp HTTP/1.1",
            "GET http://mcp-proxy.invalid/oauth-resource HTTP/1.1",
            "GET http://mcp-proxy.invalid/.well-known/oauth-authorization-server HTTP/1.1",
        ]
    );
    let entries: JsonValue = serde_json::from_slice(&output.stdout)?;
    assert_eq!(entries[0]["name"], "oauth");
    assert_eq!(
        entries[0]["auth_status"],
        "not_logged_in",
        "OAuth discovery failed after proxy requests {proxy_requests:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_with_macos_proxy_resolution_does_not_panic() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": "https://oauth.example/authorize",
            "token_endpoint": "https://oauth.example/token",
        })))
        .expect(2)
        .mount(&server)
        .await;
    configure_http_oauth_server(codex_home.path(), &format!("{}/mcp", server.uri())).await?;

    for respect_system_proxy in [false, true] {
        let system_proxy_override = format!("features.respect_system_proxy={respect_system_proxy}");
        let mut command = codex_command(codex_home.path())?;
        command
            .env_remove("HTTP_PROXY")
            .env_remove("http_proxy")
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .env_remove("ALL_PROXY")
            .env_remove("all_proxy")
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .args([
                "-c",
                &system_proxy_override,
                "-c",
                "mcp_oauth_credentials_store=\"file\"",
                "mcp",
                "list",
                "--json",
            ]);
        let output = command.output()?;
        assert!(
            output.status.success(),
            "macOS proxy resolution should not panic with respect_system_proxy={respect_system_proxy}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let entries: JsonValue = serde_json::from_slice(&output.stdout)?;
        assert_eq!(entries[0]["auth_status"], "not_logged_in");
    }
    Ok(())
}

#[tokio::test]
async fn list_and_get_render_expected_output() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add = codex_command(codex_home.path())?;
    add.args([
        "mcp",
        "add",
        "docs",
        "--env",
        "TOKEN=secret",
        "--",
        "docs-server",
        "--port",
        "4000",
    ])
    .assert()
    .success();

    let mut servers = load_global_mcp_servers(codex_home.path()).await?;
    let docs_entry = servers
        .get_mut("docs")
        .expect("docs server should exist after add");
    match &mut docs_entry.transport {
        McpServerTransportConfig::Stdio { env_vars, .. } => {
            *env_vars = vec!["APP_TOKEN".into(), "WORKSPACE_ID".into()];
        }
        other => panic!("unexpected transport: {other:?}"),
    }
    ConfigEditsBuilder::new(codex_home.path())
        .replace_mcp_servers(&servers)
        .apply_blocking()?;

    let mut list_cmd = codex_command(codex_home.path())?;
    let list_output = list_cmd.args(["mcp", "list"]).output()?;
    assert!(list_output.status.success());
    let stdout = String::from_utf8(list_output.stdout)?;
    assert!(stdout.contains("Name"));
    assert!(stdout.contains("docs"));
    assert!(stdout.contains("docs-server"));
    assert!(stdout.contains("TOKEN=*****"));
    assert!(stdout.contains("APP_TOKEN=*****"));
    assert!(stdout.contains("WORKSPACE_ID=*****"));
    assert!(stdout.contains("Status"));
    assert!(stdout.contains("Auth"));
    assert!(stdout.contains("enabled"));
    assert!(stdout.contains("Unsupported"));

    let mut list_json_cmd = codex_command(codex_home.path())?;
    let json_output = list_json_cmd.args(["mcp", "list", "--json"]).output()?;
    assert!(json_output.status.success());
    let stdout = String::from_utf8(json_output.stdout)?;
    let parsed: JsonValue = serde_json::from_str(&stdout)?;
    assert_eq!(
        parsed,
        json!([
          {
            "name": "docs",
            "enabled": true,
            "disabled_reason": null,
            "transport": {
              "type": "stdio",
              "command": "docs-server",
              "args": [
                "--port",
                "4000"
              ],
              "env": {
                "TOKEN": "secret"
              },
              "env_vars": [
                "APP_TOKEN",
                "WORKSPACE_ID"
              ],
              "cwd": null
            },
            "startup_timeout_sec": null,
            "tool_timeout_sec": null,
            "auth_status": "unsupported"
          }
        ]
        )
    );

    let mut get_cmd = codex_command(codex_home.path())?;
    let get_output = get_cmd.args(["mcp", "get", "docs"]).output()?;
    assert!(get_output.status.success());
    let stdout = String::from_utf8(get_output.stdout)?;
    assert!(stdout.contains("docs"));
    assert!(stdout.contains("transport: stdio"));
    assert!(stdout.contains("command: docs-server"));
    assert!(stdout.contains("args: --port 4000"));
    assert!(stdout.contains("env: TOKEN=*****"));
    assert!(stdout.contains("APP_TOKEN=*****"));
    assert!(stdout.contains("WORKSPACE_ID=*****"));
    assert!(stdout.contains("enabled: true"));
    assert!(stdout.contains("remove: codex mcp remove docs"));

    let mut get_json_cmd = codex_command(codex_home.path())?;
    get_json_cmd
        .args(["mcp", "get", "docs", "--json"])
        .assert()
        .success()
        .stdout(contains("\"name\": \"docs\"").and(contains("\"enabled\": true")));

    Ok(())
}

#[tokio::test]
async fn get_disabled_server_shows_single_line() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add = codex_command(codex_home.path())?;
    add.args(["mcp", "add", "docs", "--", "docs-server"])
        .assert()
        .success();

    let mut servers = load_global_mcp_servers(codex_home.path()).await?;
    let docs = servers
        .get_mut("docs")
        .expect("docs server should exist after add");
    docs.enabled = false;
    ConfigEditsBuilder::new(codex_home.path())
        .replace_mcp_servers(&servers)
        .apply_blocking()?;

    let mut get_cmd = codex_command(codex_home.path())?;
    let get_output = get_cmd.args(["mcp", "get", "docs"]).output()?;
    assert!(get_output.status.success());
    let stdout = String::from_utf8(get_output.stdout)?;
    assert_eq!(stdout.trim_end(), "docs (disabled)");

    Ok(())
}
