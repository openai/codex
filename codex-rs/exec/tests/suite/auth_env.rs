#![allow(clippy::unwrap_used)]
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_uses_codex_api_key_env_var() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = start_mock_server().await;
    let repo_root = codex_utils_cargo_bin::repo_root()?;

    mount_sse_once_match(
        &server,
        header("Authorization", "Bearer dummy"),
        sse(vec![ev_completed("request_0")]),
    )
    .await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("echo testing codex api key")
        .assert()
        .success();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_api_key_cannot_discard_stored_workspace_network_policy() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let policy_server = start_mock_server().await;
    let inference_server = start_mock_server().await;
    let token = concat!(
        "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.",
        "eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJlbnRlcnByaXNlIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoid29ya3NwYWNlIn19.",
        "c2lnbmF0dXJl",
    );
    std::fs::write(
        test.home_path().join("auth.json"),
        serde_json::to_vec(&json!({
            "auth_mode": "chatgpt",
            "tokens": {"id_token": token, "access_token": "workspace-token",
                       "refresh_token": "refresh-token", "account_id": "workspace"},
            "last_refresh": "2099-01-01T00:00:00Z",
        }))?,
    )?;
    std::fs::write(
        test.home_path().join("config.toml"),
        format!(
            "chatgpt_base_url = '{}/backend-api/'\ncli_auth_credentials_store = 'file'\n",
            policy_server.uri()
        ),
    )?;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .and(header("authorization", "Bearer workspace-token"))
        .and(header("chatgpt-account-id", "workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "requirements_toml": {"enterprise_managed": [{
                "id": "workspace-network", "name": "Workspace network",
                "contents": "[application.network]\nenabled = true",
            }]},
        })))
        .expect(1..)
        .mount(&policy_server)
        .await;
    let inference = mount_sse_once_match(
        &inference_server,
        header("Authorization", "Bearer dummy"),
        sse(vec![ev_completed("unexpected-request")]),
    )
    .await;

    test.cmd_with_server(&inference_server)
        .arg("--skip-git-repo-check")
        .arg("echo testing workspace network policy")
        .assert()
        .failure();

    assert!(
        inference.requests().is_empty(),
        "an environment API key sent model traffic despite workspace policy"
    );
    policy_server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_bootstrap_rejects_redirected_oauth_and_cloud_policy() -> anyhow::Result<()> {
    for redirect_oauth in [true, false] {
        let test = test_codex_exec();
        let origin = start_mock_server().await;
        let destination = start_mock_server().await;
        let inference = start_mock_server().await;
        let token = concat!(
            "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.",
            "eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJlbnRlcnByaXNlIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoid29ya3NwYWNlIn19.",
            "c2lnbmF0dXJl",
        );
        let last_refresh = if redirect_oauth {
            "2000-01-01T00:00:00Z"
        } else {
            "2099-01-01T00:00:00Z"
        };
        std::fs::write(
            test.home_path().join("auth.json"),
            serde_json::to_vec(&json!({
                "auth_mode": "chatgpt",
                "tokens": {"id_token": token, "access_token": "workspace-token",
                           "refresh_token": "refresh-token", "account_id": "workspace"},
                "last_refresh": last_refresh,
            }))?,
        )?;
        std::fs::write(
            test.home_path().join("config.toml"),
            format!(
                "chatgpt_base_url = '{}/backend-api/'\ncli_auth_credentials_store = 'file'\n",
                origin.uri()
            ),
        )?;
        let cloud_response = if redirect_oauth {
            Mock::given(method("POST"))
                .and(path("/oauth/token"))
                .and(wiremock::matchers::body_string_contains("refresh-token"))
                .respond_with(
                    ResponseTemplate::new(307)
                        .insert_header("location", format!("{}/stolen", destination.uri())),
                )
                .expect(1..)
                .mount(&origin)
                .await;
            Mock::given(method("POST"))
                .and(path("/stolen"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id_token": token, "access_token": "replacement", "refresh_token": "replacement",
                })))
                .mount(&destination)
                .await;
            ResponseTemplate::new(200).set_body_json(json!({}))
        } else {
            Mock::given(method("GET"))
                .and(path("/policy"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "requirements_toml": {"enterprise_managed": [{
                        "id": "redirected", "name": "Redirected policy",
                        "contents": "[application.network.domains]\n'foreign.example' = 'allow'",
                    }]},
                })))
                .mount(&destination)
                .await;
            ResponseTemplate::new(308)
                .insert_header("location", format!("{}/policy", destination.uri()))
        };
        let cloud = Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .respond_with(cloud_response);
        if redirect_oauth {
            cloud.mount(&origin).await;
        } else {
            cloud.expect(1..).mount(&origin).await;
        }
        mount_sse_once_match(
            &inference,
            header("Authorization", "Bearer dummy"),
            sse(vec![ev_completed("request")]),
        )
        .await;

        let output = test
            .cmd_with_server(&inference)
            .env(
                codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                format!("{}/oauth/token", origin.uri()),
            )
            .arg("--skip-git-repo-check")
            .arg("exercise embedded discovery")
            .output()?;
        if !redirect_oauth {
            assert!(!output.status.success());
        }
        origin.verify().await;
        assert!(destination.received_requests().await.unwrap().is_empty());
    }
    Ok(())
}
