//! Exercise Cloud credential isolation through the CLI using a synthetic login.

use anyhow::Result;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::path;

#[tokio::test]
async fn cloud_list_respects_the_credential_store_and_trusted_destinations() -> Result<()> {
    let server = MockServer::start().await;
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "cli_auth_credentials_store = 'file'\n",
    )?;
    std::fs::write(
        codex_home.path().join("auth.json"),
        serde_json::to_vec(&json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "eyJhbGciOiJub25lIn0.e30.c2ln",
                "access_token": "synthetic-cloud-access-token",
                "refresh_token": "synthetic-cloud-refresh-token",
                "account_id": "synthetic-cloud-account",
            },
            "last_refresh": chrono::Utc::now(),
        }))?,
    )?;

    let command = || -> Result<assert_cmd::Command> {
        let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
        command
            .current_dir(codex_home.path())
            .env("CODEX_HOME", codex_home.path())
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_CLOUD_TASKS_MODE")
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .timeout(std::time::Duration::from_secs(15));
        Ok(command)
    };
    // Establish that the isolated profile has a usable ChatGPT login.
    command()?
        .args(["login", "status"])
        .assert()
        .success()
        .stderr(contains("Logged in using ChatGPT"));

    let output = command()?
        .env(
            "CODEX_CLOUD_TASKS_BASE_URL",
            format!("{}/backend-api", server.uri()),
        )
        .args(["cloud", "list", "--limit", "1", "--json"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    insta::assert_snapshot!(String::from_utf8(output)?, @"Error: CODEX_CLOUD_TASKS_BASE_URL must use a trusted HTTPS origin on port 443, without user information, a query, or a fragment; custom backends cannot use saved ChatGPT credentials");
    assert!(server.received_requests().await.unwrap().is_empty());

    // An explicit ephemeral store must exclude the saved login for Cloud itself.
    command()?
        .env(
            "CODEX_CLOUD_TASKS_BASE_URL",
            "https://chatgpt-staging.com/backend-api",
        )
        .env("HTTPS_PROXY", server.uri())
        .env("https_proxy", server.uri())
        .args([
            "-c",
            "cli_auth_credentials_store='ephemeral'",
            "cloud",
            "list",
            "--limit",
            "1",
            "--json",
        ])
        .assert()
        .failure()
        .stderr(contains("Not signed in. Please run 'codex login'"));
    assert!(server.received_requests().await.unwrap().is_empty());

    // Staging must reach explicit PAT authentication. Reject the synthetic PAT locally so
    // this test never sends a request to the real staging backend.
    let auth_server = MockServer::start().await;
    Mock::given(path("/v1/user-auth-credential/whoami"))
        .and(header("authorization", "Bearer at-synthetic-cloud"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&auth_server)
        .await;
    command()?
        .env(
            "CODEX_CLOUD_TASKS_BASE_URL",
            "https://chatgpt-staging.com/backend-api",
        )
        .env("CODEX_ACCESS_TOKEN", "at-synthetic-cloud")
        .env("CODEX_AUTHAPI_BASE_URL", auth_server.uri())
        .args(["cloud", "list", "--limit", "1", "--json"])
        .assert()
        .failure()
        .stderr(contains("Not signed in. Please run 'codex login'"));
    auth_server.verify().await;
    Ok(())
}
