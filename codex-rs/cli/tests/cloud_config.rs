use std::process::Output;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::ConfigLoadOptions;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::load_config_toml_with_layer_stack;
use codex_core::config::load_global_mcp_servers;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const MANAGED_SERVER_NAME: &str = "managed-slack";
const MANAGED_CLIENT_ID: &str = "managed-oauth-client";
const MANAGED_SCOPE: &str = "managed.read";
const MOCK_ACCESS_TOKEN: &str = "mock-managed-access-token";
const MOCK_REFRESH_TOKEN: &str = "mock-managed-refresh-token";

struct CloudManagedConfigFixture {
    server: MockServer,
    codex_home: TempDir,
    user_config: String,
    mcp_url: String,
}

impl CloudManagedConfigFixture {
    async fn new() -> Result<Option<Self>> {
        let server = MockServer::start().await;
        let chatgpt_base_url = format!("{}/backend-api", server.uri());
        let codex_home = TempDir::new()?;
        let user_config = format!(
            "cli_auth_credentials_store = \"file\"\nchatgpt_base_url = \"{chatgpt_base_url}\"\n"
        );
        std::fs::write(codex_home.path().join("config.toml"), &user_config)?;

        let bootstrap_config = load_config_toml_with_layer_stack(
            codex_home.path(),
            Some(&AbsolutePathBuf::from_absolute_path(codex_home.path())?),
            Vec::new(),
            ConfigLoadOptions::default(),
        )
        .await?;
        if bootstrap_config.config_toml.cli_auth_credentials_store
            != Some(AuthCredentialsStoreMode::File)
            || bootstrap_config.config_toml.chatgpt_base_url.as_deref()
                != Some(chatgpt_base_url.as_str())
        {
            eprintln!(
                "skipping cloud-managed subprocess: host-managed authentication or backend routing prevents isolated mock credentials"
            );
            return Ok(None);
        }

        write_chatgpt_auth(
            codex_home.path(),
            ChatGptAuthFixture::new("chatgpt-token")
                .account_id("workspace-123")
                .chatgpt_account_id("workspace-123")
                .chatgpt_user_id("user-123")
                .plan_type("enterprise"),
            AuthCredentialsStoreMode::File,
        )?;

        let mcp_url = format!("{}/mcp", server.uri());
        let managed_config = format!(
            "mcp_oauth_credentials_store = \"file\"\n\n\
             [mcp_servers.{MANAGED_SERVER_NAME}]\n\
             url = \"{mcp_url}\"\n\
             auth = \"oauth\"\n\
             scopes = [\"{MANAGED_SCOPE}\"]\n\n\
             [mcp_servers.{MANAGED_SERVER_NAME}.oauth]\n\
             client_id = \"{MANAGED_CLIENT_ID}\"\n\n\
             [marketplaces.managed]\n\
             source_type = \"git\"\n\
             source = \"https://github.com/owner/repo.git\"\n"
        );
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .and(header("authorization", "Bearer chatgpt-token"))
            .and(header("chatgpt-account-id", "workspace-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "config_toml": {
                    "enterprise_managed": [{
                        "id": "managed-config",
                        "name": "Managed resources",
                        "contents": managed_config,
                    }],
                },
            })))
            .expect(1)
            .mount(&server)
            .await;

        Ok(Some(Self {
            server,
            codex_home,
            user_config,
            mcp_url,
        }))
    }

    fn command(&self, args: &[&str]) -> Result<Command> {
        let mut command = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
        command
            .kill_on_drop(true)
            .current_dir(self.codex_home.path())
            .env("CODEX_HOME", self.codex_home.path())
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_FEDERATION_RULE_ID")
            .env_remove("OPENAI_IDENTITY_TOKEN_FILE")
            .env_remove("OPENAI_WORKLOAD_IDENTITY_CONTEXT")
            .args(args);
        Ok(command)
    }

    async fn output(&self, args: &[&str]) -> Result<Output> {
        let output = self.command(args)?.output().await?;
        ensure!(
            output.status.success(),
            "codex {} failed with status {}: stdout={}; stderr={}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        Ok(output)
    }

    fn assert_user_config_unchanged(&self) -> Result<()> {
        assert_eq!(
            std::fs::read_to_string(self.codex_home.path().join("config.toml"))?,
            self.user_config
        );
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_and_get_resolve_cloud_managed_mcp_without_writing_user_config() -> Result<()> {
    let Some(fixture) = CloudManagedConfigFixture::new().await? else {
        return Ok(());
    };

    let output = fixture.output(&["mcp", "list", "--json"]).await?;
    let entries: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(entries[0]["name"], MANAGED_SERVER_NAME);
    assert_eq!(entries[0]["transport"]["type"], "streamable_http");
    assert_eq!(entries[0]["transport"]["url"], fixture.mcp_url);

    let output = fixture
        .output(&["mcp", "get", MANAGED_SERVER_NAME, "--json"])
        .await?;
    let entry: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(entry["name"], MANAGED_SERVER_NAME);
    assert_eq!(entry["transport"]["url"], fixture.mcp_url);
    assert!(
        fixture
            .codex_home
            .path()
            .join("cloud-config-bundle-cache.json")
            .exists()
    );
    fixture.assert_user_config_unchanged()?;
    fixture.server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_and_logout_persist_only_cloud_managed_mcp_oauth_credentials() -> Result<()> {
    let Some(fixture) = CloudManagedConfigFixture::new().await? else {
        return Ok(());
    };

    let challenge = format!(
        "Bearer resource_metadata=\"{}/oauth-resource\"",
        fixture.server.uri()
    );
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(401).insert_header("WWW-Authenticate", challenge))
        .mount(&fixture.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/oauth-resource"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": fixture.mcp_url,
            "authorization_servers": [fixture.server.uri()],
        })))
        .mount(&fixture.server)
        .await;
    let oauth_metadata = json!({
        "issuer": fixture.server.uri(),
        "authorization_endpoint": format!("{}/oauth/authorize", fixture.server.uri()),
        "token_endpoint": format!("{}/oauth/token", fixture.server.uri()),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": [MANAGED_SCOPE],
    });
    for metadata_path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/oauth-authorization-server/mcp",
    ] {
        Mock::given(method("GET"))
            .and(path(metadata_path))
            .respond_with(ResponseTemplate::new(200).set_body_json(oauth_metadata.clone()))
            .mount(&fixture.server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains(format!(
            "client_id={MANAGED_CLIENT_ID}"
        )))
        .and(body_string_contains("grant_type=authorization_code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": MOCK_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": MOCK_REFRESH_TOKEN,
            "scope": MANAGED_SCOPE,
        })))
        .expect(1)
        .mount(&fixture.server)
        .await;

    let mut command = fixture.command(&["mcp", "login", MANAGED_SERVER_NAME])?;
    command.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .context("MCP login did not provide captured stdout")?;
    let mut lines = BufReader::new(stdout).lines();
    let authorization_url = timeout(Duration::from_secs(30), async {
        while let Some(line) = lines.next_line().await? {
            if line.starts_with("http://") || line.starts_with("https://") {
                return Ok::<_, anyhow::Error>(Url::parse(line.trim())?);
            }
        }
        anyhow::bail!("MCP login exited before printing its OAuth authorization URL")
    })
    .await
    .context("timed out waiting for the managed MCP authorization URL")??;

    let query_pairs: Vec<_> = authorization_url.query_pairs().into_owned().collect();
    let state = query_pairs
        .iter()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.as_str())
        .context("managed MCP authorization URL did not contain OAuth state")?;
    assert_eq!(
        query_pairs
            .iter()
            .find(|(name, _)| name == "client_id")
            .map(|(_, value)| value.as_str()),
        Some(MANAGED_CLIENT_ID)
    );
    assert_eq!(
        query_pairs
            .iter()
            .find(|(name, _)| name == "scope")
            .map(|(_, value)| value.as_str()),
        Some(MANAGED_SCOPE)
    );
    let redirect_uri = query_pairs
        .iter()
        .find(|(name, _)| name == "redirect_uri")
        .map(|(_, value)| value.as_str())
        .context("managed MCP authorization URL did not contain a callback")?;
    let mut callback_url = Url::parse(redirect_uri)?;
    callback_url
        .query_pairs_mut()
        .append_pair("code", "mock-managed-authorization-code")
        .append_pair("state", state);
    let callback_host = callback_url
        .host_str()
        .context("managed MCP callback did not contain a host")?;
    let callback_port = callback_url
        .port_or_known_default()
        .context("managed MCP callback did not contain a port")?;
    let callback_path = match callback_url.query() {
        Some(query) => format!("{}?{query}", callback_url.path()),
        None => callback_url.path().to_string(),
    };
    let callback_response = timeout(Duration::from_secs(30), async {
        let mut callback = TcpStream::connect((callback_host, callback_port)).await?;
        callback
            .write_all(
                format!(
                    "GET {callback_path} HTTP/1.1\r\nHost: {callback_host}:{callback_port}\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await?;
        let mut response_lines = BufReader::new(callback).lines();
        response_lines
            .next_line()
            .await?
            .context("managed MCP OAuth callback returned an empty HTTP response")
    })
    .await
    .context("timed out waiting for the managed MCP OAuth callback response")??;
    ensure!(
        callback_response.starts_with("HTTP/1.1 200")
            || callback_response.starts_with("HTTP/1.0 200"),
        "managed MCP OAuth callback failed: {callback_response}"
    );

    let login_status = timeout(Duration::from_secs(30), child.wait())
        .await
        .context("timed out waiting for managed MCP login")??;
    ensure!(
        login_status.success(),
        "managed MCP login failed: status={login_status}"
    );
    timeout(Duration::from_secs(30), async {
        while let Some(line) = lines.next_line().await? {
            if line.contains("Successfully logged in to MCP server 'managed-slack'.") {
                return Ok::<_, anyhow::Error>(());
            }
        }
        anyhow::bail!("managed MCP login exited before printing its success message")
    })
    .await
    .context("timed out waiting for the managed MCP login success message")??;

    let credentials_path = fixture.codex_home.path().join(".credentials.json");
    let credentials: Value = serde_json::from_slice(&std::fs::read(&credentials_path)?)?;
    let entries = credentials
        .as_object()
        .context("MCP credentials should be a JSON object")?;
    assert_eq!(entries.len(), 1);
    let credential = entries
        .values()
        .next()
        .context("managed MCP OAuth credentials were not persisted")?;
    assert_eq!(credential["server_name"], MANAGED_SERVER_NAME);
    assert_eq!(credential["server_url"], fixture.mcp_url);
    assert_eq!(credential["client_id"], MANAGED_CLIENT_ID);
    assert_eq!(credential["access_token"], MOCK_ACCESS_TOKEN);
    assert_eq!(credential["refresh_token"], MOCK_REFRESH_TOKEN);
    assert_eq!(credential["scopes"], json!([MANAGED_SCOPE]));
    fixture.assert_user_config_unchanged()?;

    let list_output = fixture.output(&["mcp", "list", "--json"]).await?;
    let entries: Value = serde_json::from_slice(&list_output.stdout)?;
    assert_eq!(entries[0]["name"], MANAGED_SERVER_NAME);
    assert_eq!(entries[0]["auth_status"], "o_auth");

    let logout_output = fixture
        .output(&["mcp", "logout", MANAGED_SERVER_NAME])
        .await?;
    assert!(
        String::from_utf8(logout_output.stdout)?
            .contains("Removed OAuth credentials for 'managed-slack'.")
    );
    assert!(!credentials_path.exists());
    fixture.assert_user_config_unchanged()?;
    fixture.server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_and_remove_preserve_cloud_managed_resources() -> Result<()> {
    let Some(fixture) = CloudManagedConfigFixture::new().await? else {
        return Ok(());
    };

    let installed_root = marketplace_install_root(fixture.codex_home.path()).join("managed");
    std::fs::create_dir_all(&installed_root)?;
    let marker = installed_root.join("marker.txt");
    std::fs::write(&marker, "installed")?;
    // Exercise both the initial fetch and the cached bundle without a user entry.
    for _ in 0..2 {
        let output = fixture
            .command(&["plugin", "marketplace", "remove", "managed"])?
            .output()
            .await?;
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)?.contains(
            "marketplace `managed` is configured in enterprise-managed (Managed resources, managed-config); remove it from that configuration source instead"
        ));
        fixture.assert_user_config_unchanged()?;
        assert_eq!(std::fs::read_to_string(&marker)?, "installed");
    }

    fixture
        .output(&["mcp", "get", MANAGED_SERVER_NAME, "--json"])
        .await?;
    fixture
        .output(&["mcp", "add", "local-docs", "--", "echo", "hello"])
        .await?;
    let local_servers = load_global_mcp_servers(fixture.codex_home.path()).await?;
    assert!(local_servers.contains_key("local-docs"));
    assert!(!local_servers.contains_key(MANAGED_SERVER_NAME));

    let output = fixture
        .output(&["mcp", "remove", MANAGED_SERVER_NAME])
        .await?;
    assert!(
        String::from_utf8(output.stdout)?.contains("No MCP server named 'managed-slack' found.")
    );
    let local_servers = load_global_mcp_servers(fixture.codex_home.path()).await?;
    assert!(local_servers.contains_key("local-docs"));
    assert!(!local_servers.contains_key(MANAGED_SERVER_NAME));

    fixture.output(&["mcp", "remove", "local-docs"]).await?;
    assert!(
        load_global_mcp_servers(fixture.codex_home.path())
            .await?
            .is_empty()
    );
    fixture
        .output(&["mcp", "get", MANAGED_SERVER_NAME, "--json"])
        .await?;
    fixture.server.verify().await;

    Ok(())
}

async fn mock_startup_bundle(server: &MockServer, status: u16, config: &str, requirements: &str) {
    server.reset().await;
    let layer = |contents: &str| {
        json!({
            "enterprise_managed": [{"id": "startup", "name": "Startup policy", "contents": contents}]
        })
    };
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .respond_with(ResponseTemplate::new(status).set_body_json(json!({
            "config_toml": layer(config), "requirements_toml": layer(requirements),
        })))
        .expect(1..)
        .mount(server)
        .await;
}

async fn successful_json(command: &mut Command) -> Result<Value> {
    let output = command.output().await?;
    ensure!(output.status.success(), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[tokio::test]
async fn startup_config_loads_managed_policy_or_reports_cloud_failure() -> Result<()> {
    for status in [200, 401, 503] {
        let Some(fixture) = CloudManagedConfigFixture::new().await? else {
            return Ok(());
        };
        mock_startup_bundle(
            &fixture.server,
            status,
            "forced_login_method = 'api'\n",
            "[application.network.domains]\n'127.0.0.1' = 'allow'\n'example.com' = 'allow'\n",
        )
        .await;
        let sqlite_home = TempDir::new()?;
        let history_path = sqlite_home.path().join("state_5.sqlite");
        std::fs::write(&history_path, b"history must not be repaired")?;
        let output = fixture
            .command(&["app-server", "--print-startup-config"])?
            .env("CODEX_SQLITE_HOME", sqlite_home.path())
            .env(
                "CODEX_REFRESH_TOKEN_URL_OVERRIDE",
                format!("{}/refresh", fixture.server.uri()),
            )
            .output()
            .await?;
        let value: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output.status.success(), status == 200, "{value}");
        if status == 200 {
            let requirements = &value["configRequirements"]["requirements"];
            assert_eq!(requirements["allowedLoginMethods"], json!(["api"]));
            assert_eq!(
                requirements["application"]["network"]["domains"]["example.com"],
                "allow"
            );
        } else {
            assert_eq!(value["error"]["data"]["reason"], "cloudConfigBundle");
            assert!(value.get("configRequirements").is_none());
        }
        assert_eq!(
            std::fs::read(&history_path)?,
            b"history must not be repaired"
        );
        assert_eq!(std::fs::read_dir(sqlite_home.path())?.count(), 1);
        fixture.server.verify().await;
    }
    Ok(())
}

#[tokio::test]
async fn startup_config_sign_out_reloads_policy_without_starting_runtime() -> Result<()> {
    for (status, provider) in [(200, None), (503, None), (503, Some("amazon-bedrock"))] {
        let Some(mut fixture) = CloudManagedConfigFixture::new().await? else {
            return Ok(());
        };
        if let Some(provider) = provider {
            fixture
                .user_config
                .push_str(&format!("model_provider = \"{provider}\"\n"));
        }
        std::fs::write(
            fixture.codex_home.path().join("config.toml"),
            &fixture.user_config,
        )?;
        mock_startup_bundle(
            &fixture.server, status, "forced_login_method = 'api'\n",
            "cli_auth_credentials_store = 'ephemeral'\nenforce_residency = 'us'\n[application.network.domains]\n'127.0.0.1' = 'allow'\n[windows]\nrequire_mxc = true\n",
        ).await;
        let mut revoke = Mock::given(method("POST")).and(path("/revoke"));
        if status == 200 {
            revoke = revoke.and(header("x-openai-internal-codex-residency", "us"));
        }
        revoke
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&fixture.server)
            .await;
        let signed_out_home = TempDir::new()?;
        std::fs::write(
            signed_out_home.path().join("config.toml"),
            &fixture.user_config,
        )?;
        let baseline = successful_json(
            fixture
                .command(&["app-server", "--print-startup-config"])?
                .env("CODEX_HOME", signed_out_home.path()),
        )
        .await?;
        let sqlite_home = TempDir::new()?;
        let value = successful_json(
            fixture
                .command(&["app-server", "--print-startup-config", "--startup-sign-out"])?
                .env("CODEX_SQLITE_HOME", sqlite_home.path())
                .env(
                    "CODEX_REVOKE_TOKEN_URL_OVERRIDE",
                    format!("{}/revoke", fixture.server.uri()),
                ),
        )
        .await?;
        assert_eq!(value["configRequirements"], baseline["configRequirements"]);
        assert_eq!(std::fs::read_dir(sqlite_home.path())?.count(), 0);
        let user_config: toml::Value = toml::from_str(&std::fs::read_to_string(
            fixture.codex_home.path().join("config.toml"),
        )?)?;
        assert!(user_config.get("model_provider").is_none());
        let status = fixture.command(&["login", "status"])?.output().await?;
        assert!(!status.status.success());
        fixture.server.verify().await;
    }
    Ok(())
}
