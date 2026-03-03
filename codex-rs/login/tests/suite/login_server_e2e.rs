#![allow(clippy::unwrap_used)]
use std::io;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_login::ServerOptions;
use codex_login::run_login_server;
use core_test_support::skip_if_no_network;
use tempfile::tempdir;

// See spawn.rs for details

fn start_mock_issuer(chatgpt_account_id: &str) -> (SocketAddr, thread::JoinHandle<()>) {
    // Bind to a random available port
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tiny_http::Server::from_listener(listener, None).unwrap();
    let chatgpt_account_id = chatgpt_account_id.to_string();

    let handle = thread::spawn(move || {
        while let Ok(mut req) = server.recv() {
            let url = req.url().to_string();
            if url.starts_with("/oauth/token") {
                // Read body
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);
                // Build minimal JWT with plan=pro
                #[derive(serde::Serialize)]
                struct Header {
                    alg: &'static str,
                    typ: &'static str,
                }
                let header = Header {
                    alg: "none",
                    typ: "JWT",
                };
                let payload = serde_json::json!({
                    "email": "user@example.com",
                    "https://api.openai.com/auth": {
                        "chatgpt_plan_type": "pro",
                        "chatgpt_account_id": chatgpt_account_id,
                    }
                });
                let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
                let header_bytes = serde_json::to_vec(&header).unwrap();
                let payload_bytes = serde_json::to_vec(&payload).unwrap();
                let id_token = format!(
                    "{}.{}.{}",
                    b64(&header_bytes),
                    b64(&payload_bytes),
                    b64(b"sig")
                );

                let tokens = serde_json::json!({
                    "id_token": id_token,
                    "access_token": "access-123",
                    "refresh_token": "refresh-123",
                });
                let data = serde_json::to_vec(&tokens).unwrap();
                let mut resp = tiny_http::Response::from_data(data);
                resp.add_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap_or_else(|_| panic!("header bytes")),
                );
                let _ = req.respond(resp);
            } else {
                let _ = req
                    .respond(tiny_http::Response::from_string("not found").with_status_code(404));
            }
        }
    });

    (addr, handle)
}

#[tokio::test]
async fn end_to_end_login_flow_persists_auth_json() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let chatgpt_account_id = "12345678-0000-0000-0000-000000000000";
    let (issuer_addr, issuer_handle) = start_mock_issuer(chatgpt_account_id);
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir()?;
    let codex_home = tmp.path().to_path_buf();

    // Seed auth.json with stale API key + tokens that should be overwritten.
    let stale_auth = serde_json::json!({
        "OPENAI_API_KEY": "sk-stale",
        "tokens": {
            "id_token": "stale.header.payload",
            "access_token": "stale-access",
            "refresh_token": "stale-refresh",
            "account_id": "stale-acc"
        }
    });
    std::fs::write(
        codex_home.join("auth.json"),
        serde_json::to_string_pretty(&stale_auth)?,
    )?;

    let state = "test_state_123".to_string();

    // Run server in background
    let server_home = codex_home.clone();

    let opts = ServerOptions {
        codex_home: server_home,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state),
        forced_chatgpt_workspace_id: Some(chatgpt_account_id.to_string()),
    };
    let server = run_login_server(opts)?;
    assert!(
        server
            .auth_url
            .contains(format!("allowed_workspace_id={chatgpt_account_id}").as_str()),
        "auth URL should include forced workspace parameter"
    );
    let login_port = server.actual_port;

    // Simulate browser callback, and follow redirect to /success
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;
    let url = format!("http://127.0.0.1:{login_port}/auth/callback?code=abc&state=test_state_123");
    let resp = client.get(&url).send().await?;
    assert!(resp.status().is_success());

    // Wait for server shutdown
    server.block_until_done().await?;

    // Validate auth.json
    let auth_path = codex_home.join("auth.json");
    let data = std::fs::read_to_string(&auth_path)?;
    let json: serde_json::Value = serde_json::from_str(&data)?;
    // The following assert is here because of the old oauth flow that exchanges tokens for an
    // API key. See obtain_api_key in server.rs for details. Once we remove this old mechanism
    // from the code, this test should be updated to expect that the API key is no longer present.
    assert_eq!(json["OPENAI_API_KEY"], "access-123");
    assert_eq!(json["tokens"]["access_token"], "access-123");
    assert_eq!(json["tokens"]["refresh_token"], "refresh-123");
    assert_eq!(json["tokens"]["account_id"], chatgpt_account_id);

    // Stop mock issuer
    drop(issuer_handle);
    Ok(())
}

#[tokio::test]
async fn creates_missing_codex_home_dir() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (issuer_addr, _issuer_handle) = start_mock_issuer("org-123");
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir()?;
    let codex_home = tmp.path().join("missing-subdir"); // does not exist

    let state = "state2".to_string();

    // Run server in background
    let server_home = codex_home.clone();
    let opts = ServerOptions {
        codex_home: server_home,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state),
        forced_chatgpt_workspace_id: None,
    };
    let server = run_login_server(opts)?;
    let login_port = server.actual_port;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{login_port}/auth/callback?code=abc&state=state2");
    let resp = client.get(&url).send().await?;
    assert!(resp.status().is_success());

    server.block_until_done().await?;

    let auth_path = codex_home.join("auth.json");
    assert!(
        auth_path.exists(),
        "auth.json should be created even if parent dir was missing"
    );
    Ok(())
}

#[tokio::test]
async fn forced_chatgpt_workspace_id_mismatch_blocks_login() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (issuer_addr, _issuer_handle) = start_mock_issuer("org-actual");
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir()?;
    let codex_home = tmp.path().to_path_buf();
    let state = "state-mismatch".to_string();

    let opts = ServerOptions {
        codex_home: codex_home.clone(),
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state.clone()),
        forced_chatgpt_workspace_id: Some("org-required".to_string()),
    };
    let server = run_login_server(opts)?;
    assert!(
        server
            .auth_url
            .contains("allowed_workspace_id=org-required"),
        "auth URL should include forced workspace parameter"
    );
    let login_port = server.actual_port;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{login_port}/auth/callback?code=abc&state={state}");
    let resp = client.get(&url).send().await?;
    assert!(resp.status().is_success());
    let body = resp.text().await?;
    assert!(
        body.contains("Login is restricted to workspace id org-required"),
        "error body should mention workspace restriction"
    );

    let result = server.block_until_done().await;
    assert!(
        result.is_err(),
        "login should fail due to workspace mismatch"
    );
    let err = result.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);

    let auth_path = codex_home.join("auth.json");
    assert!(
        !auth_path.exists(),
        "auth.json should not be written when the workspace mismatches"
    );

    Ok(())
}

#[tokio::test]
async fn oauth_access_denied_missing_entitlement_blocks_login_with_clear_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (issuer_addr, _issuer_handle) = start_mock_issuer("org-123");
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir()?;
    let codex_home = tmp.path().to_path_buf();
    let state = "state-entitlement".to_string();

    let opts = ServerOptions {
        codex_home: codex_home.clone(),
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state.clone()),
        forced_chatgpt_workspace_id: None,
    };
    let server = run_login_server(opts)?;
    let login_port = server.actual_port;

    let client = reqwest::Client::new();
    let url = format!(
        "http://127.0.0.1:{login_port}/auth/callback?state={state}&error=access_denied&error_description=missing_codex_entitlement"
    );
    let resp = client.get(&url).send().await?;
    assert!(resp.status().is_success());
    let body = resp.text().await?;
    assert!(
        body.contains("You do not have access to Codex"),
        "error body should clearly explain the Codex access denial"
    );
    assert!(
        body.contains("Contact your workspace administrator"),
        "error body should tell the user how to get access"
    );
    assert!(
        body.contains("access_denied"),
        "error body should still include the oauth error code"
    );
    assert!(
        !body.contains("missing_codex_entitlement"),
        "known entitlement errors should be mapped to user-facing copy"
    );

    let result = server.block_until_done().await;
    assert!(result.is_err(), "login should fail for access_denied");
    let err = result.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string()
            .contains("Contact your workspace administrator"),
        "terminal error should also tell the user what to do next"
    );

    let auth_path = codex_home.join("auth.json");
    assert!(
        !auth_path.exists(),
        "auth.json should not be written when oauth callback is denied"
    );

    Ok(())
}

#[tokio::test]
async fn oauth_access_denied_unknown_reason_uses_generic_error_page() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (issuer_addr, _issuer_handle) = start_mock_issuer("org-123");
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir()?;
    let codex_home = tmp.path().to_path_buf();
    let state = "state-generic-denial".to_string();

    let opts = ServerOptions {
        codex_home: codex_home.clone(),
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state.clone()),
        forced_chatgpt_workspace_id: None,
    };
    let server = run_login_server(opts)?;
    let login_port = server.actual_port;

    let client = reqwest::Client::new();
    let url = format!(
        "http://127.0.0.1:{login_port}/auth/callback?state={state}&error=access_denied&error_description=some_other_reason"
    );
    let resp = client.get(&url).send().await?;
    assert!(resp.status().is_success());
    let body = resp.text().await?;
    assert!(
        body.contains("Sign-in could not be completed"),
        "generic oauth denial should use the generic error page title"
    );
    assert!(
        body.contains("Sign-in failed: some_other_reason"),
        "generic oauth denial should preserve the oauth error details"
    );
    assert!(
        body.contains("Return to Codex to retry"),
        "generic oauth denial should keep the generic help text"
    );
    assert!(
        body.contains("access_denied"),
        "generic oauth denial should include the oauth error code"
    );
    assert!(
        body.contains("some_other_reason"),
        "generic oauth denial should include the oauth error description"
    );
    assert!(
        !body.contains("You do not have access to Codex"),
        "generic oauth denial should not show the entitlement-specific title"
    );
    assert!(
        !body.contains("get access to Codex"),
        "generic oauth denial should not show the entitlement-specific admin guidance"
    );

    let result = server.block_until_done().await;
    assert!(result.is_err(), "login should fail for access_denied");
    let err = result.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string()
            .contains("Sign-in failed: some_other_reason"),
        "terminal error should preserve generic oauth details"
    );

    let auth_path = codex_home.join("auth.json");
    assert!(
        !auth_path.exists(),
        "auth.json should not be written when oauth callback is denied"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancels_previous_login_server_when_port_is_in_use() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (issuer_addr, _issuer_handle) = start_mock_issuer("org-123");
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let first_tmp = tempdir()?;
    let first_codex_home = first_tmp.path().to_path_buf();

    let first_opts = ServerOptions {
        codex_home: first_codex_home,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer: issuer.clone(),
        port: 0,
        open_browser: false,
        force_state: Some("cancel_state".to_string()),
        forced_chatgpt_workspace_id: None,
    };

    let first_server = run_login_server(first_opts)?;
    let login_port = first_server.actual_port;
    let first_server_task = tokio::spawn(async move { first_server.block_until_done().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let second_tmp = tempdir()?;
    let second_codex_home = second_tmp.path().to_path_buf();

    let second_opts = ServerOptions {
        codex_home: second_codex_home,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: login_port,
        open_browser: false,
        force_state: Some("cancel_state_2".to_string()),
        forced_chatgpt_workspace_id: None,
    };

    let second_server = run_login_server(second_opts)?;
    assert_eq!(second_server.actual_port, login_port);

    let cancel_result = first_server_task
        .await
        .expect("first login server task panicked")
        .expect_err("login server should report cancellation");
    assert_eq!(cancel_result.kind(), io::ErrorKind::Interrupted);

    let client = reqwest::Client::new();
    let cancel_url = format!("http://127.0.0.1:{login_port}/cancel");
    let resp = client.get(cancel_url).send().await?;
    assert!(resp.status().is_success());

    second_server
        .block_until_done()
        .await
        .expect_err("second login server should report cancellation");
    Ok(())
}
