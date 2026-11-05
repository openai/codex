//! Cover credential refresh between Cloud startup and environment discovery.

#![cfg(test)]

use base64::Engine;
use chrono::Utc;
use codex_utils_cli::CliConfigOverrides;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn access_token(expires_at: i64) -> String {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({
            "exp": expires_at,
            "iss": "https://auth.openai.com/",
            "sub": "user-a",
            "session_id": "session-a",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-a",
                "chatgpt_user_id": "user-a",
                "chatgpt_plan_type": "plus",
            },
        })
        .to_string(),
    );
    format!("header.{claims}.signature")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_discovery_refreshes_after_cloud_startup() {
    const SERVER: &str = "CODEX_CLOUD_REFRESH_TEST_SERVER";
    if let Ok(server) = std::env::var(SERVER) {
        let overrides = CliConfigOverrides {
            raw_overrides: vec![
                "cli_auth_credentials_store=\"file\"".into(),
                "features.runtime_agent_instances=false".into(),
                "features.use_agent_identity=false".into(),
            ],
        };
        let context = super::init_backend("cloud-refresh-test", &overrides)
            .await
            .expect("Cloud startup");
        for _ in 0..2 {
            let environments = crate::env_detect::list_environments(
                &context.environment_http,
                &server,
                &crate::util::build_chatgpt_headers(),
            )
            .await
            .expect("environment discovery after token refresh");
            assert_eq!(environments.len(), 1);
            assert_eq!(environments[0].id, "env-a");
        }
        return;
    }

    let server = MockServer::start().await;
    let home = tempfile::tempdir().expect("isolated Cloud credentials");
    let expired = access_token(Utc::now().timestamp() - 60);
    // The startup token is near expiry, so discovery must refresh it through AuthManager.
    let startup = access_token(Utc::now().timestamp() + 240);
    let current = access_token(Utc::now().timestamp() + 3_600);
    std::fs::write(
        home.path().join("auth.json"),
        json!({
            "tokens": {
                "id_token": expired, "access_token": expired,
                "refresh_token": "refresh-initial", "account_id": "account-a",
            },
            "last_refresh": Utc::now(),
        })
        .to_string(),
    )
    .expect("write initial credentials");
    let refreshed = current.clone();
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(move |request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().unwrap();
            let token = match body["refresh_token"].as_str().unwrap() {
                "refresh-initial" => &startup,
                "refresh-startup" => &refreshed,
                other => panic!("unexpected test refresh token: {other}"),
            };
            ResponseTemplate::new(200).set_body_json(json!({
                "id_token": token, "access_token": token,
                "refresh_token": "refresh-startup",
            }))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/api/codex/environments"))
        .respond_with(ResponseTemplate::new(401))
        .with_priority(10)
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/codex/environments"))
        .and(header("authorization", format!("Bearer {current}")))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": "env-a", "label": "Environment A"},
        ])))
        .expect(2)
        .mount(&server)
        .await;

    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "backend_auth_tests::environment_discovery_refreshes_after_cloud_startup",
            "--nocapture",
        ])
        .current_dir(home.path())
        .env(SERVER, server.uri())
        .env("CODEX_HOME", home.path())
        .env(
            "CODEX_REFRESH_TOKEN_URL_OVERRIDE",
            format!("{}/token", server.uri()),
        )
        .env(
            "CODEX_CLOUD_TASKS_BASE_URL",
            "https://chatgpt.com/backend-api",
        )
        .env_remove("CODEX_CLOUD_TASKS_MODE")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY");
    let output = tokio::task::spawn_blocking(move || command.output())
        .await
        .expect("Cloud test child task")
        .expect("Cloud test child process");
    assert!(
        output.status.success(),
        "Cloud startup regression failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_list_preserves_existing_agent_identity_during_runtime_rollout() -> anyhow::Result<()>
{
    use core_test_support::runtime_auth::PROBE_SERVER;
    use core_test_support::runtime_auth::run_harness_through_proxy;

    const TEST: &str =
        "backend_auth_tests::cloud_list_preserves_existing_agent_identity_during_runtime_rollout";
    const MODE: &str = "CODEX_CLOUD_PROGRAMMATIC_TEST_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        let enabled = mode != "off";
        let mode = if enabled {
            mode.as_str()
        } else {
            "dual_header"
        };
        let overrides = CliConfigOverrides {
            raw_overrides: vec![
                "cli_auth_credentials_store=\"file\"".into(),
                format!("features.runtime_agent_instances={{ enabled={enabled}, mode={mode:?} }}"),
                "features.use_agent_identity=false".into(),
            ],
        };
        let context = super::init_backend("cloud-programmatic-test", &overrides).await?;
        let page = codex_cloud_tasks_client::CloudBackend::list_tasks(
            &*context.backend,
            /*env*/ None,
            /*limit*/ Some(10),
            /*cursor*/ None,
        )
        .await?;
        assert!(page.tasks.is_empty());
        let environments = crate::env_detect::list_environments(
            &context.environment_http,
            "https://chatgpt.com",
            &crate::util::build_chatgpt_headers(),
        )
        .await?;
        assert_eq!(environments.len(), 1);
        assert_eq!(environments[0].id, "env-a");
        return Ok(());
    }
    if std::env::var(PROBE_SERVER).is_ok() {
        let home = tempfile::tempdir()?;
        let key = codex_agent_identity::generate_agent_key_material()?;
        std::fs::write(
            home.path().join("auth.json"),
            json!({
                "auth_mode": "agentIdentity",
                "agent_identity": {
                    "agent_runtime_id": "existing-agent",
                    "agent_private_key": key.private_key_pkcs8_base64,
                    "account_id": "account-a",
                    "chatgpt_user_id": "user-a",
                    "plan_type": "plus",
                    "chatgpt_account_is_fedramp": false,
                    "task_id": "existing-task",
                },
            })
            .to_string(),
        )?;
        // Keep each credential/configuration case in its own process, retaining the proxy
        // from the outer fixture without mutating environment variables in a running test.
        for mode in ["off", "dual_header", "assertion_only"] {
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command
                .args(["--exact", TEST, "--nocapture"])
                .current_dir(home.path())
                .env(MODE, mode)
                .env("CODEX_HOME", home.path())
                .env(
                    "CODEX_CLOUD_TASKS_BASE_URL",
                    "https://chatgpt.com/backend-api",
                )
                .env_remove("CODEX_CLOUD_TASKS_MODE")
                .env_remove("CODEX_ACCESS_TOKEN")
                .env_remove("CODEX_API_KEY")
                .env_remove("OPENAI_API_KEY");
            let output = tokio::task::spawn_blocking(move || command.output()).await??;
            assert!(
                output.status.success(),
                "{mode}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        return Ok(());
    }
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/tasks/list"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"items": [], "cursor": null})),
        )
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/codex/environments"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{ "id": "env-a", "label": "Environment A" }])),
        )
        .expect(3)
        .mount(&server)
        .await;
    run_harness_through_proxy(&server, TEST, /*realtime_target*/ None).await?;
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 6);
    for request in requests {
        let assertion = request.headers["authorization"]
            .to_str()?
            .strip_prefix("AgentAssertion ")
            .unwrap();
        let envelope: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(assertion)?,
        )?;
        assert_eq!(envelope["agent_runtime_id"], "existing-agent");
        assert_eq!(envelope["task_id"], "existing-task");
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    server.verify().await;
    Ok(())
}
