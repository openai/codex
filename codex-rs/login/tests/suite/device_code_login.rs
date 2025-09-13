#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_core::auth::get_auth_file;
use codex_core::auth::try_read_auth_json;
use codex_login::ServerOptions;
use codex_login::run_device_code_login;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::tempdir;
use tiny_http::Header;
use tiny_http::Response;

const CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "CODEX_SANDBOX_NETWORK_DISABLED";

fn skip_if_network_disabled(test_name: &str) -> bool {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        eprintln!("skipping {test_name}: networking disabled in sandbox");
        true
    } else {
        false
    }
}

fn make_jwt(payload: serde_json::Value) -> String {
    let header = json!({ "alg": "none", "typ": "JWT" });
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    let signature_b64 = URL_SAFE_NO_PAD.encode(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

fn json_response(value: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = value.to_string();
    let mut response = Response::from_string(body);
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]) {
        response.add_header(header);
    }
    response
}

#[tokio::test]
async fn device_code_login_integration_succeeds() {
    if skip_if_network_disabled("device_code_login_integration_succeeds") {
        return;
    }

    let codex_home = tempdir().unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let issuer = format!("http://127.0.0.1:{port}");

    let poll_calls = Arc::new(AtomicUsize::new(0));
    let poll_calls_thread = poll_calls.clone();
    let jwt = make_jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct_321"
        }
    }));
    let jwt_thread = jwt.clone();

    let server_handle = std::thread::spawn(move || {
        for request in server.incoming_requests() {
            match request.url() {
                "/devicecode/usercode" => {
                    let resp = json_response(json!({
                        "user_code": "CODE-1234",
                        "interval": 0
                    }));
                    request.respond(resp).unwrap();
                }
                "/deviceauth/token" => {
                    let attempt = poll_calls_thread.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        let resp = json_response(json!({ "error": "token_pending" }))
                            .with_status_code(400);
                        request.respond(resp).unwrap();
                    } else {
                        let resp = json_response(json!({
                            "id_token": jwt_thread,
                            "access_token": "access-token-321",
                            "refresh_token": "refresh-token-321"
                        }));
                        request.respond(resp).unwrap();
                    }
                }
                "/oauth/token" => {
                    let resp = json_response(json!({ "access_token": "api-key-321" }));
                    request.respond(resp).unwrap();
                    break;
                }
                _ => {
                    let _ = request.respond(Response::from_string("").with_status_code(404));
                }
            }
        }
    });

    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;

    run_device_code_login(opts)
        .await
        .expect("device code login integration should succeed");

    server_handle.join().unwrap();

    let auth_path = get_auth_file(codex_home.path());
    let auth = try_read_auth_json(&auth_path).expect("auth.json written");
    assert_eq!(auth.openai_api_key.as_deref(), Some("api-key-321"));
    let tokens = auth.tokens.expect("tokens persisted");
    assert_eq!(tokens.access_token, "access-token-321");
    assert_eq!(tokens.refresh_token, "refresh-token-321");
    assert_eq!(tokens.id_token.raw_jwt, jwt);
    assert_eq!(tokens.account_id.as_deref(), Some("acct_321"));
    assert_eq!(poll_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn device_code_login_integration_handles_error_payload() {
    if skip_if_network_disabled("device_code_login_integration_handles_error_payload") {
        return;
    }

    let codex_home = tempdir().unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let issuer = format!("http://127.0.0.1:{port}");

    let server_handle = std::thread::spawn(move || {
        for request in server.incoming_requests() {
            match request.url() {
                "/devicecode/usercode" => {
                    let resp = json_response(json!({
                        "user_code": "CODE-ERR",
                        "interval": 0
                    }));
                    request.respond(resp).unwrap();
                }
                "/deviceauth/token" => {
                    let resp = json_response(json!({
                        "error": "authorization_declined",
                        "error_description": "Denied"
                    }))
                    .with_status_code(400);
                    request.respond(resp).unwrap();
                    break;
                }
                _ => {
                    let _ = request.respond(Response::from_string("").with_status_code(404));
                }
            }
        }
    });

    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;

    let err = run_device_code_login(opts)
        .await
        .expect_err("integration failure path should return error");
    assert_eq!(
        err.to_string(),
        "device auth failed: authorization_declined: Denied"
    );

    server_handle.join().unwrap();

    let auth_path = get_auth_file(codex_home.path());
    assert!(
        !auth_path.exists(),
        "auth.json should not be created when device auth fails"
    );
}

#[tokio::test]
async fn device_code_login_integration_handles_usercode_http_failure() {
    if skip_if_network_disabled("device_code_login_integration_handles_usercode_http_failure") {
        return;
    }

    let codex_home = tempdir().unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let issuer = format!("http://127.0.0.1:{port}");

    let server_handle = std::thread::spawn(move || {
        for request in server.incoming_requests() {
            match request.url() {
                "/devicecode/usercode" => {
                    let resp = Response::from_string("").with_status_code(503);
                    request.respond(resp).unwrap();
                    break;
                }
                _ => {
                    let _ = request.respond(Response::from_string("").with_status_code(404));
                }
            }
        }
    });

    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;

    let err = run_device_code_login(opts)
        .await
        .expect_err("usercode HTTP failure should bubble up");
    assert!(
        err.to_string()
            .contains("device code request failed with status")
    );

    server_handle.join().unwrap();

    let auth_path = get_auth_file(codex_home.path());
    assert!(!auth_path.exists());
}

#[tokio::test]
async fn device_code_login_integration_persists_without_api_key_on_exchange_failure() {
    if skip_if_network_disabled(
        "device_code_login_integration_persists_without_api_key_on_exchange_failure",
    ) {
        return;
    }

    let codex_home = tempdir().unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let issuer = format!("http://127.0.0.1:{port}");

    let poll_calls = Arc::new(AtomicUsize::new(0));
    let poll_calls_thread = poll_calls.clone();
    let jwt = make_jwt(json!({}));
    let jwt_thread = jwt.clone();

    let server_handle = std::thread::spawn(move || {
        for request in server.incoming_requests() {
            match request.url() {
                "/devicecode/usercode" => {
                    let resp = json_response(json!({
                        "user_code": "CODE-NOAPI",
                        "interval": 0
                    }));
                    request.respond(resp).unwrap();
                }
                "/deviceauth/token" => {
                    let attempt = poll_calls_thread.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        let resp = json_response(json!({ "error": "token_pending" }))
                            .with_status_code(400);
                        request.respond(resp).unwrap();
                    } else {
                        let resp = json_response(json!({
                            "id_token": jwt_thread,
                            "access_token": "access-token-999",
                            "refresh_token": "refresh-token-999"
                        }));
                        request.respond(resp).unwrap();
                    }
                }
                "/oauth/token" => {
                    let resp = Response::from_string("").with_status_code(500);
                    request.respond(resp).unwrap();
                    break;
                }
                _ => {
                    let _ = request.respond(Response::from_string("").with_status_code(404));
                }
            }
        }
    });

    let mut opts = ServerOptions::new(codex_home.path().to_path_buf(), "client-id".to_string());
    opts.issuer = issuer;
    opts.open_browser = false;

    run_device_code_login(opts)
        .await
        .expect("device login should succeed without API key exchange");

    server_handle.join().unwrap();

    let auth_path = get_auth_file(codex_home.path());
    let auth = try_read_auth_json(&auth_path).expect("auth.json written");
    assert!(auth.openai_api_key.is_none());
    let tokens = auth.tokens.expect("tokens persisted");
    assert_eq!(tokens.access_token, "access-token-999");
    assert_eq!(tokens.refresh_token, "refresh-token-999");
    assert_eq!(tokens.id_token.raw_jwt, jwt);
    assert_eq!(poll_calls.load(Ordering::SeqCst), 2);
}
