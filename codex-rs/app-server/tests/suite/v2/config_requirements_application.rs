//! Public requirements responses expose managed application policy.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);

async fn start_server(home: &TempDir) -> Result<TestAppServer> {
    TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await
}

async fn read_requirements(server: &mut TestAppServer) -> Result<Value> {
    let request_id = server.send_config_requirements_read_request().await?;
    timeout(READ_TIMEOUT, server.read_response(request_id)).await?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn application_network_defaults_and_null_base_url_are_exposed() -> Result<()> {
    for (contents, application) in [
        ("", Value::Null),
        ("[application]", json!({"network": null})),
        (
            "[application.network]",
            json!({"network": {"enabled": true, "domains": {}}}),
        ),
        (
            "[application.network]\nenabled = false",
            json!({"network": {"enabled": false, "domains": {}}}),
        ),
    ] {
        let home = TempDir::new()?;
        std::fs::write(
            home.path().join("requirements.toml"),
            format!("allow_remote_control = false\n{contents}"),
        )?;
        let mut server = start_server(&home).await?;
        let wire = read_requirements(&mut server).await?;
        let requirements = wire["requirements"]
            .as_object()
            .expect("requirements object");
        assert_eq!(
            (
                requirements.get("application"),
                requirements.get("chatgptBaseUrl")
            ),
            (Some(&application), Some(&Value::Null)),
        );
    }
    let home = TempDir::new()?;
    let mut server = start_server(&home).await?;
    assert_eq!(
        read_requirements(&mut server).await?,
        json!({"requirements": null, "supportsIndependentSpeedModes": true})
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn application_destinations_stay_separate_from_agent_network() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("requirements.toml"),
        r#"
chatgpt_base_url = "https://managed.example.com/backend-api/"
[experimental_network.domains]
"github.com" = "allow"
[application.network.domains]
"GOV.example.com." = "allow"
"github.com" = "deny"
"#,
    )?;
    let mut server = start_server(&home).await?;
    let wire = read_requirements(&mut server).await?;
    assert_eq!(
        (
            &wire["requirements"]["application"],
            &wire["requirements"]["network"]["domains"],
            &wire["requirements"]["chatgptBaseUrl"]
        ),
        (
            &json!({"network": {"enabled": true, "domains": {"gov.example.com": "allow", "github.com": "deny"}}}),
            &json!({"github.com": "allow"}),
            &json!("https://managed.example.com/backend-api/")
        ),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn application_requirements_read_rejects_invalid_configuration() -> Result<()> {
    let home = TempDir::new()?;
    let mut server = start_server(&home).await?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "[application.network.domains]\n'*.example.com' = 'allow'",
    )?;
    let request_id = server.send_config_requirements_read_request().await?;
    let error = timeout(
        READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(
        error.error.message.contains("application.network.domains"),
        "{error:?}"
    );
    Ok(())
}
