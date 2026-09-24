use std::path::Path;

use anyhow::Result;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::load_global_mcp_servers;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
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

#[tokio::test]
async fn add_and_remove_server_updates_global_config() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args(["mcp", "add", "docs", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(contains("Added global MCP server 'docs'."));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert_eq!(servers.len(), 1);
    let docs = servers.get("docs").expect("server should exist");
    match &docs.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            assert_eq!(command, "echo");
            assert_eq!(args, &vec!["hello".to_string()]);
            assert!(env.is_none());
            assert!(env_vars.is_empty());
            assert!(cwd.is_none());
        }
        other => panic!("unexpected transport: {other:?}"),
    }
    assert!(docs.enabled);

    let mut remove_cmd = codex_command(codex_home.path())?;
    remove_cmd
        .args(["mcp", "remove", "docs"])
        .assert()
        .success()
        .stdout(contains("Removed global MCP server 'docs'."));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    let mut remove_again_cmd = codex_command(codex_home.path())?;
    remove_again_cmd
        .args(["mcp", "remove", "docs"])
        .assert()
        .success()
        .stdout(contains("No MCP server named 'docs' found."));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}

#[tokio::test]
async fn npm_server_names_round_trip_through_cli() -> Result<()> {
    let codex_home = TempDir::new()?;
    let name = "npm:@modelcontextprotocol/server-sequential.thinking";

    codex_command(codex_home.path())?
        .args(["mcp", "add", name, "--", "echo", "hello"])
        .assert()
        .success();

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains(&format!("[mcp_servers.\"{name}\"]")));
    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert_eq!(
        servers.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![name]
    );

    let output = codex_command(codex_home.path())?
        .args(["mcp", "get", name, "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let server: serde_json::Value = serde_json::from_slice(&output)?;
    assert_eq!(server["name"], name);

    let output = codex_command(codex_home.path())?
        .args(["mcp", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&output)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["name"], name);

    codex_command(codex_home.path())?
        .args(["mcp", "remove", name])
        .assert()
        .success();
    assert!(load_global_mcp_servers(codex_home.path()).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn add_and_login_discover_oauth_through_configured_http_proxy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let proxy = MockServer::start().await;
    let resource_url = "http://cli-mcp.invalid";
    let challenge = "Bearer resource_metadata=\"http://cli-mcp.invalid/oauth-resource\"";
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(401).insert_header("WWW-Authenticate", challenge))
        .mount(&proxy)
        .await;
    Mock::given(method("GET"))
        .and(path("/oauth-resource"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "resource": format!("{resource_url}/mcp"),
            "authorization_servers": [resource_url],
        })))
        .mount(&proxy)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "authorization_endpoint": format!("{resource_url}/oauth/authorize"),
            "token_endpoint": format!("{resource_url}/oauth/token"),
            "registration_endpoint": format!("{resource_url}/oauth/register"),
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"],
        })))
        .mount(&proxy)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/register"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&proxy)
        .await;

    let mut add = codex_command(codex_home.path())?;
    add.env("HTTP_PROXY", proxy.uri())
        .env("http_proxy", proxy.uri())
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
            "add",
            "oauth",
            "--url",
            "http://cli-mcp.invalid/mcp",
        ]);
    let add_output = tokio::task::spawn_blocking(move || add.output()).await??;
    assert!(
        !add_output.status.success(),
        "mock OAuth registration should terminate the automatic login"
    );
    assert!(
        load_global_mcp_servers(codex_home.path())
            .await?
            .contains_key("oauth")
    );
    let helper_command = if cfg!(windows) {
        r#"echo {"X-Gateway":"gateway-token"}"#
    } else {
        r#"printf '{"X-Gateway":"gateway-token"}'"#
    };
    let config_path = codex_home.path().join("config.toml");
    let mut config = std::fs::read_to_string(&config_path)?;
    config.push_str(&format!(
        "http_headers_helper = {}\n",
        toml::Value::String(helper_command.to_string())
    ));
    std::fs::write(config_path, config)?;

    // Local OAuth login does not require the execution-environment registry.
    std::fs::write(codex_home.path().join("environments.toml"), "invalid = [")?;

    let mut login = codex_command(codex_home.path())?;
    login
        .env("HTTP_PROXY", proxy.uri())
        .env("http_proxy", proxy.uri())
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
            "login",
            "oauth",
        ]);
    let login_output = tokio::task::spawn_blocking(move || login.output()).await??;
    assert!(
        !login_output.status.success(),
        "mock OAuth registration should terminate the explicit login"
    );

    let requests = proxy
        .received_requests()
        .await
        .expect("mock proxy should record OAuth requests");
    let registrations: Vec<_> = requests
        .iter()
        .filter(|request| request.method == "POST" && request.url.path() == "/oauth/register")
        .collect();
    assert_eq!(registrations.len(), 2);
    assert_eq!(
        registrations
            .iter()
            .filter(|request| request.headers.get("x-gateway").is_some())
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn profile_mcp_reports_legacy_profile_migration() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[profiles.work]
model = "gpt-5"
"#,
    )?;

    let mut list_cmd = codex_command(codex_home.path())?;
    list_cmd
        .args(["--profile", "work", "mcp", "list"])
        .assert()
        .failure()
        .stderr(contains("--profile `work` cannot be used"))
        .stderr(contains("[profiles.work]"))
        .stderr(contains("work.config.toml"));

    Ok(())
}

#[tokio::test]
async fn add_with_env_preserves_key_order_and_values() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "envy",
            "--env",
            "FOO=bar",
            "--env",
            "ALPHA=beta",
            "--",
            "python",
            "server.py",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let envy = servers.get("envy").expect("server should exist");
    let env = match &envy.transport {
        McpServerTransportConfig::Stdio { env: Some(env), .. } => env,
        other => panic!("unexpected transport: {other:?}"),
    };

    assert_eq!(env.len(), 2);
    assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
    assert_eq!(env.get("ALPHA"), Some(&"beta".to_string()));
    assert!(envy.enabled);

    Ok(())
}

#[tokio::test]
async fn add_streamable_http_without_manual_token() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "github",
            "--url",
            "https://example.com/mcp",
            "--oauth-client-registration",
            "dcr",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let github = servers.get("github").expect("github server should exist");
    match &github.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
            ..
        } => {
            assert_eq!(url, "https://example.com/mcp");
            assert!(bearer_token_env_var.is_none());
            assert!(http_headers.is_none());
            assert!(env_http_headers.is_none());
        }
        other => panic!("unexpected transport: {other:?}"),
    }
    assert!(github.enabled);
    assert_eq!(github.oauth, None);

    assert!(!codex_home.path().join(".credentials.json").exists());
    assert!(!codex_home.path().join(".env").exists());
    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config.contains("client_registration"));
    assert!(!config.contains("[mcp_servers.github.oauth]"));

    Ok(())
}

#[tokio::test]
async fn add_rejects_blank_client_credentials_before_saving() -> Result<()> {
    let mut errors = Vec::new();
    for (client_id, client_secret) in [("registered-client", "   "), ("   ", "cli-secret-marker")] {
        let codex_home = TempDir::new()?;
        let output = codex_command(codex_home.path())?
            .args([
                "mcp",
                "add",
                "private",
                "--url",
                "http://127.0.0.1:9/mcp",
                "--oauth-client-id",
                client_id,
                "--oauth-client-secret",
                client_secret,
            ])
            .assert()
            .failure()
            .get_output()
            .clone();
        let stderr = String::from_utf8(output.stderr)?;
        assert!(!stderr.contains("cli-secret-marker"));
        assert!(!String::from_utf8(output.stdout)?.contains("cli-secret-marker"));
        assert!(load_global_mcp_servers(codex_home.path()).await?.is_empty());
        errors.push(stderr.trim().to_string());
    }
    insta::assert_snapshot!(errors.join("\n"), @r"
    Error: --oauth-client-secret must not be empty
    Error: --oauth-client-secret requires a nonempty --oauth-client-id
    ");
    Ok(())
}

#[tokio::test]
async fn add_streamable_http_with_custom_env_var() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "issues",
            "--url",
            "https://example.com/issues",
            "--bearer-token-env-var",
            "GITHUB_TOKEN",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let issues = servers.get("issues").expect("issues server should exist");
    match &issues.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
            ..
        } => {
            assert_eq!(url, "https://example.com/issues");
            assert_eq!(bearer_token_env_var.as_deref(), Some("GITHUB_TOKEN"));
            assert!(http_headers.is_none());
            assert!(env_http_headers.is_none());
        }
        other => panic!("unexpected transport: {other:?}"),
    }
    assert!(issues.enabled);
    Ok(())
}

#[tokio::test]
async fn add_streamable_http_with_oauth_options() -> Result<()> {
    let codex_home = TempDir::new()?;
    let expected_callback = "http://127.0.0.1/callback/w9gKTtkB7gWy";

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "-c",
            "mcp_oauth_callback_port=43123",
            "mcp",
            "add",
            "oauth-server",
            "--url",
            "https://example.com/mcp",
            "--oauth-client-id",
            "eci-prd-pub-codex-123",
            "--oauth-resource",
            "https://resource.example.com",
        ])
        .assert()
        .success()
        .stdout(contains(format!("OAuth callback URL: {expected_callback}")));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let oauth_server = servers
        .get("oauth-server")
        .expect("oauth server should exist");
    assert_eq!(
        oauth_server.oauth_client_id(),
        Some("eci-prd-pub-codex-123")
    );
    assert_eq!(
        oauth_server
            .oauth
            .as_ref()
            .and_then(|oauth| oauth.callback_url.as_deref()),
        Some(expected_callback)
    );
    assert_eq!(
        oauth_server.oauth_resource.as_deref(),
        Some("https://resource.example.com")
    );

    Ok(())
}

#[tokio::test]
async fn add_persists_issuer_bound_callback_before_starting_oauth() -> Result<()> {
    let codex_home = TempDir::new()?;
    let oauth_server = MockServer::start().await;
    let issuer = format!("{}/mcp", oauth_server.uri());
    let metadata = serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": "not-a-valid-authorization-url",
        "token_endpoint": format!("{}/token", oauth_server.uri()),
        "authorization_response_iss_parameter_supported": true,
    });
    let concurrent_codex_home = codex_home.path().to_path_buf();
    let concurrent_add = std::sync::Once::new();
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(move |_: &wiremock::Request| {
            concurrent_add.call_once(|| {
                codex_command(&concurrent_codex_home)
                    .expect("create concurrent MCP add command")
                    .args(["mcp", "add", "concurrent", "--", "echo", "concurrent"])
                    .assert()
                    .success();
            });
            ResponseTemplate::new(200).set_body_json(metadata.clone())
        })
        .mount(&oauth_server)
        .await;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd.args([
        "mcp",
        "add",
        "issuer-bound",
        "--url",
        &format!("{}/mcp", oauth_server.uri()),
        "--oauth-client-id",
        "registered-client",
    ]);
    let output = tokio::task::spawn_blocking(move || add_cmd.output()).await??;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)?.contains("OAuth callback URL: http://127.0.0.1/callback")
    );
    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.contains_key("concurrent"));
    assert_eq!(
        servers["issuer-bound"]
            .oauth
            .as_ref()
            .and_then(|oauth| oauth.callback_url.as_deref()),
        Some("http://127.0.0.1/callback")
    );

    Ok(())
}

#[tokio::test]
async fn add_streamable_http_rejects_removed_flag() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "github",
            "--url",
            "https://example.com/mcp",
            "--with-bearer-token",
        ])
        .assert()
        .failure()
        .stderr(contains("--with-bearer-token"));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}

#[tokio::test]
async fn add_cant_add_command_and_url() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "github",
            "--url",
            "https://example.com/mcp",
            "--command",
            "--",
            "echo",
            "hello",
        ])
        .assert()
        .failure()
        .stderr(contains("unexpected argument '--command' found"));

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}
