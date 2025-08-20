#![allow(clippy::unwrap_used)]
use std::net::SocketAddr;
use std::net::TcpListener;
use std::thread;

use base64::Engine;
use codex_login::ServerOptions;
use codex_login::run_login_server;
use tempfile::tempdir;

// See spawn.rs for details
pub const CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "CODEX_SANDBOX_NETWORK_DISABLED";

fn start_mock_issuer() -> (SocketAddr, thread::JoinHandle<()>) {
    // Bind to a random available port
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tiny_http::Server::from_listener(listener, None).unwrap();

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
                        "chatgpt_account_id": "acc-123"
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
async fn end_to_end_login_flow_persists_auth_json() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let (issuer_addr, issuer_handle) = start_mock_issuer();
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir().unwrap();
    let codex_home = tmp.path().to_path_buf();

    let state = "test_state_123".to_string();

    // Run server in background
    let server_home = codex_home.clone();

    let opts = ServerOptions {
        codex_home: server_home,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state),
    };
    let server = run_login_server(opts).unwrap();
    let login_port = server.actual_port;

    // Simulate browser callback, and follow redirect to /success
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{login_port}/auth/callback?code=abc&state=test_state_123");
    let resp = client.get(&url).send().await.unwrap();
    assert!(resp.status().is_success());

    // Wait for server shutdown
    server.block_until_done().await.unwrap();

    // Validate auth.json
    let auth_path = codex_home.join("auth.json");
    let data = std::fs::read_to_string(&auth_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert!(
        !json["OPENAI_API_KEY"].is_null(),
        "OPENAI_API_KEY should be set"
    );
    assert_eq!(json["tokens"]["access_token"], "access-123");
    assert_eq!(json["tokens"]["refresh_token"], "refresh-123");
    assert_eq!(json["tokens"]["account_id"], "acc-123");

    // Stop mock issuer
    drop(issuer_handle);
}

#[tokio::test]
async fn creates_missing_codex_home_dir() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let (issuer_addr, _issuer_handle) = start_mock_issuer();
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    let tmp = tempdir().unwrap();
    let codex_home = tmp.path().join("missing-subdir"); // does not exist

    let state = "state2".to_string();

    // Run server in background
    let server_home = codex_home.clone();
    let opts = ServerOptions {
        codex_home: server_home,
        client_id: codex_login::CLIENT_ID.to_string(),
        issuer,
        port: 0,
        open_browser: false,
        force_state: Some(state),
    };
    let server = run_login_server(opts).unwrap();
    let login_port = server.actual_port;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{login_port}/auth/callback?code=abc&state=state2");
    let resp = client.get(&url).send().await.unwrap();
    assert!(resp.status().is_success());

    server.block_until_done().await.unwrap();

    let auth_path = codex_home.join("auth.json");
    assert!(
        auth_path.exists(),
        "auth.json should be created even if parent dir was missing"
    );
}
