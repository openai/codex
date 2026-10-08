//! Verifies MCP argument handling and managed networking in standalone commands.

use clap::Parser;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpError;
use codex_http_client::NetworkPolicyDenied;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use super::McpCli;
use super::McpSubcommand;

#[test]
fn oauth_client_secret_is_redacted_in_parsed_command_debug() {
    let cli = McpCli::try_parse_from([
        "mcp",
        "add",
        "private",
        "--url",
        "https://example.com/mcp",
        "--oauth-client-id",
        "registered-client",
        "--oauth-client-secret",
        "cli-secret-marker",
    ])
    .expect("parse confidential client arguments");
    let debug = format!("{cli:?}");
    assert!(!debug.contains("cli-secret-marker"));
    assert!(debug.contains("oauth_client_secret: Some(<redacted>)"));
    let McpSubcommand::Add(add) = cli.subcommand else {
        panic!("expected MCP add");
    };
    let http = add.transport_args.streamable_http.expect("HTTP arguments");
    assert_eq!(
        http.oauth_client_secret
            .as_ref()
            .map(|secret| secret.as_str()),
        Some("cli-secret-marker")
    );
}

#[test]
fn oauth_client_secret_requires_url_and_client_id_without_disclosure() {
    for args in [
        vec!["--url", "https://example.com/mcp"],
        vec!["--oauth-client-id", "registered-client"],
    ] {
        let error = McpCli::try_parse_from(
            [
                "mcp",
                "add",
                "private",
                "--oauth-client-secret",
                "cli-secret-marker",
            ]
            .into_iter()
            .chain(args),
        )
        .expect_err("client secret requires both HTTP URL and client ID");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        assert!(!error.to_string().contains("cli-secret-marker"));
    }
}

async fn isolated_test_home(test_name: &str) -> anyhow::Result<Option<PathBuf>> {
    const CHILD: &str = "CODEX_MCP_NETWORK_TEST_CHILD";
    let test_name = format!("mcp_cmd::tests::{test_name}");
    if std::env::var(CHILD).as_deref() == Ok(test_name.as_str()) {
        return Ok(Some(codex_core::config::find_codex_home()?.to_path_buf()));
    }

    // Only the child receives the temporary home; the production loader resolves
    // CODEX_HOME normally and the parent test process keeps its environment.
    let home = tempfile::tempdir()?;
    let output = timeout(
        Duration::from_secs(45),
        Command::new(std::env::current_exe()?)
            .args(["--exact", &test_name, "--nocapture"])
            .kill_on_drop(true)
            .env(CHILD, &test_name)
            .env("CODEX_HOME", home.path())
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .current_dir(home.path())
            .output(),
    )
    .await??;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed; 0 failed"),
        "{test_name}: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(None)
}

async fn config_with_application_network(
    home: &std::path::Path,
    requirements: &str,
    extra_config: &str,
) -> anyhow::Result<codex_core::config::Config> {
    std::fs::write(
        home.join("config.toml"),
        format!(
            "forced_login_method = 'api'\n\
             cli_auth_credentials_store = 'file'\n\
             mcp_oauth_credentials_store = 'file'\n\
             [features]\nplugins = false\nrecommended_plugins = false\n\
             {extra_config}"
        ),
    )?;
    let requirements_path = home.join("requirements.toml");
    std::fs::write(&requirements_path, requirements)?;
    crate::cloud_config::load_config(
        &codex_utils_cli::CliConfigOverrides::default(),
        codex_core::config::LoaderOverrides {
            system_config_path: Some(home.join("absent-system-config.toml")),
            system_requirements_path: Some(requirements_path),
            ignore_project_config: true,
            ..codex_core::config::LoaderOverrides::without_managed_config_for_tests()
        },
    )
    .await
}

fn http_add_args(url: &str) -> super::AddArgs {
    let cli = McpCli::try_parse_from(["mcp", "add", "policy-probe", "--url", url])
        .expect("parse HTTP add arguments");
    let McpSubcommand::Add(args) = cli.subcommand else {
        panic!("expected MCP add");
    };
    args
}

#[tokio::test]
async fn application_network_mcp_add_denies_initial_oauth_discovery_before_connect()
-> anyhow::Result<()> {
    let Some(home) = isolated_test_home(
        "application_network_mcp_add_denies_initial_oauth_discovery_before_connect",
    )
    .await?
    else {
        return Ok(());
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("https://{}/mcp", listener.local_addr()?);
    let config = config_with_application_network(&home, "[application.network]\n", "").await?;

    // Add deliberately saves the server when discovery is unknown. A successful
    // command is therefore not evidence that its network request was permitted.
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        super::run_add(&config, http_add_args(&url)),
    )
    .await??;
    let saved: toml::Value = toml::from_str(&std::fs::read_to_string(home.join("config.toml"))?)?;
    assert_eq!(
        saved["mcp_servers"]["policy-probe"]["url"].as_str(),
        Some(url.as_str())
    );
    // No accepted TCP connection also proves no HTTP discovery request arrived.
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[tokio::test]
async fn application_network_mcp_add_discovers_when_policy_is_disabled() -> anyhow::Result<()> {
    let Some(home) =
        isolated_test_home("application_network_mcp_add_discovers_when_policy_is_disabled").await?
    else {
        return Ok(());
    };
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let url = format!("{}/mcp", server.uri());
    let config =
        config_with_application_network(&home, "[application.network]\nenabled = false\n", "")
            .await?;
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        super::run_add(&config, http_add_args(&url)),
    )
    .await??;
    let requests = server.received_requests().await.expect("recorded requests");
    assert!(
        requests.iter().any(|request| request.url.path() == "/mcp"),
        "the positive control must perform real OAuth discovery"
    );
    // The 404 response has no OAuth support, so this cannot open a browser.
    Ok(())
}

#[tokio::test]
async fn application_network_mcp_login_denies_even_an_allowed_mcp_server() -> anyhow::Result<()> {
    let Some(home) =
        isolated_test_home("application_network_mcp_login_denies_even_an_allowed_mcp_server")
            .await?
    else {
        return Ok(());
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("https://{}/mcp", listener.local_addr()?);
    let requirements =
        format!("[application.network]\n[mcp_servers.policy-probe.identity]\nurl = {url:?}\n");
    let extra_config = format!("[mcp_servers.policy-probe]\nurl = {url:?}\n");
    let config = config_with_application_network(&home, &requirements, &extra_config).await?;
    let server = &config.mcp_servers.get()["policy-probe"];
    assert!(
        server.enabled,
        "MCP identity allowlist must permit this server"
    );
    assert!(server.disabled_reason.is_none());
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        super::run_login(
            &config,
            super::LoginArgs {
                name: "policy-probe".to_string(),
                no_browser: true,
                scopes: Vec::new(),
                oauth_client_registration: None,
            },
        ),
    )
    .await?
    .expect_err("MCP permission must not override application network restrictions");
    let message = format!("{error:#}");
    assert!(
        message.contains("destination denied by application network policy"),
        "{message}"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[tokio::test]
async fn application_network_loader_denies_unlisted_destination_before_connect()
-> anyhow::Result<()> {
    let Some(home) =
        isolated_test_home("application_network_loader_denies_unlisted_destination_before_connect")
            .await?
    else {
        return Ok(());
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("https://{}/must-not-connect", listener.local_addr()?);
    let config = config_with_application_network(
        &home,
        "[application.network.domains]\n'allowed.example' = 'allow'\n",
        "",
    )
    .await?;
    let factory = config.http_client_factory();
    factory
        .network_policy()
        .acquire(&"https://allowed.example/v1".parse()?)?;
    let client = factory.build_client(&url, ClientRouteClass::Other)?;
    assert!(matches!(
        timeout(Duration::from_secs(2), client.get(&url).send()).await?,
        Err(HttpError::Policy(NetworkPolicyDenied::Destination))
    ));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[tokio::test]
async fn application_network_loader_allows_listed_https_destination_to_connect()
-> anyhow::Result<()> {
    let Some(home) =
        isolated_test_home("application_network_loader_allows_listed_https_destination_to_connect")
            .await?
    else {
        return Ok(());
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}/allowed", listener.local_addr()?);
    let config = config_with_application_network(
        &home,
        "[application.network.domains]\n'127.0.0.1' = 'allow'\n",
        "",
    )
    .await?;
    let client = config
        .http_client_factory()
        .build_client(&url, ClientRouteClass::Other)?;
    let (response, connection) = timeout(Duration::from_secs(2), async {
        tokio::join!(client.get(&url).send(), async {
            let (stream, _) = listener.accept().await?;
            // This listener proves TCP permission; it deliberately does not provide TLS.
            drop(stream);
            Ok::<_, std::io::Error>(())
        })
    })
    .await?;
    connection?;
    assert!(response.is_err(), "the raw listener cannot complete TLS");
    assert!(!matches!(response, Err(HttpError::Policy(_))));
    Ok(())
}

#[tokio::test]
async fn application_network_loader_rejects_malformed_managed_policy() -> anyhow::Result<()> {
    let Some(home) =
        isolated_test_home("application_network_loader_rejects_malformed_managed_policy").await?
    else {
        return Ok(());
    };
    let error = config_with_application_network(
        &home,
        "[application.network.domains]\n'*.example.com' = 'allow'\n",
        "",
    )
    .await
    .expect_err("invalid application policy must not load as unrestricted");
    let message = format!("{error:#}");
    assert!(message.contains("requirements.toml"), "{message}");
    assert!(
        message.contains("application.network.domains requires exact ASCII domain names"),
        "{message}"
    );
    Ok(())
}
