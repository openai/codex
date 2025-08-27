#![allow(clippy::unwrap_used)]

use std::net::SocketAddr;
use std::net::TcpListener;
use std::thread;

use base64::Engine;
use codex_login::LoginError;
use codex_login::login_with_native_browser;
use std::sync::Mutex;
use std::sync::OnceLock;
use tempfile::tempdir;

// Skip tests when running in a sandbox with network disabled.
const CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "CODEX_SANDBOX_NETWORK_DISABLED";

#[inline]
fn set_env(key: &str, val: &str) {
    // Environment mutation is unsafe in Rust 2024 due to global process state.
    unsafe { std::env::set_var(key, val) }
}

#[inline]
fn unset_env(key: &str) {
    unsafe { std::env::remove_var(key) }
}

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn start_mock_issuer() -> (SocketAddr, thread::JoinHandle<()>) {
    // Bind to a random available port
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tiny_http::Server::from_listener(listener, None).unwrap();

    let handle = thread::spawn(move || {
        while let Ok(mut req) = server.recv() {
            let url = req.url().to_string();
            if url.starts_with("/oauth/token") {
                // Read body (application/x-www-form-urlencoded)
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);

                // Decide which grant
                if body.contains("grant_type=authorization_code") {
                    // Build minimal JWT with plan=pro and email
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
                        "access_token": "access-abc",
                        "refresh_token": "refresh-abc",
                    });
                    let data = serde_json::to_vec(&tokens).unwrap();
                    let mut resp = tiny_http::Response::from_data(data);
                    resp.add_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/json"[..],
                        )
                        .unwrap(),
                    );
                    let _ = req.respond(resp);
                } else if body.contains(
                    "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Atoken-exchange",
                ) && body.contains("requested_token=openai-api-key")
                {
                    let exchange = serde_json::json!({ "access_token": "sk-test-api-key" });
                    let data = serde_json::to_vec(&exchange).unwrap();
                    let mut resp = tiny_http::Response::from_data(data);
                    resp.add_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/json"[..],
                        )
                        .unwrap(),
                    );
                    let _ = req.respond(resp);
                } else {
                    let _ = req.respond(
                        tiny_http::Response::from_string("bad request").with_status_code(400),
                    );
                }
            } else {
                let _ = req
                    .respond(tiny_http::Response::from_string("not found").with_status_code(404));
            }
        }
    });

    (addr, handle)
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[serial_test::serial]
async fn persists_auth_json_on_success() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping native browser test due to network-disabled sandbox");
        return;
    }

    let (issuer_addr, _h) = start_mock_issuer();
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());

    // Force deterministic helper output (bypass UI) and state.
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_env("CODEX_LOGIN_ISSUER_BASE", &issuer);
        set_env("CODEX_LOGIN_FORCE_STATE", "state-ok");
        set_env(
            "CODEX_LOGIN_TEST_HELPER_JSON",
            r#"{"code":"abc","state":"state-ok"}"#,
        );
    }

    let tmp = tempdir().unwrap();
    let codex_home = tmp.path().to_path_buf();

    login_with_native_browser(&codex_home)
        .await
        .expect("native login should succeed");

    // Validate auth.json
    let auth_path = codex_home.join("auth.json");
    let data = std::fs::read_to_string(&auth_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(json["OPENAI_API_KEY"], serde_json::json!("sk-test-api-key"));
    assert_eq!(json["tokens"]["access_token"], "access-abc");
    assert_eq!(json["tokens"]["refresh_token"], "refresh-abc");
    assert_eq!(json["tokens"]["account_id"], "acc-123");
    // cleanup env for other tests
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unset_env("CODEX_LOGIN_ISSUER_BASE");
        unset_env("CODEX_LOGIN_FORCE_STATE");
        unset_env("CODEX_LOGIN_TEST_HELPER_JSON");
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[serial_test::serial]
async fn abort_propagates_from_helper() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping native browser test due to network-disabled sandbox");
        return;
    }

    // Helper aborts before any network requests.
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_env("CODEX_LOGIN_TEST_HELPER_JSON", "ABORT");
    }
    let tmp = tempdir().unwrap();
    let err = login_with_native_browser(tmp.path()).await.unwrap_err();
    assert!(matches!(err, LoginError::Aborted));
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unset_env("CODEX_LOGIN_TEST_HELPER_JSON");
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[serial_test::serial]
async fn state_mismatch_is_rejected() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping native browser test due to network-disabled sandbox");
        return;
    }

    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_env("CODEX_LOGIN_FORCE_STATE", "expected");
        set_env(
            "CODEX_LOGIN_TEST_HELPER_JSON",
            r#"{"code":"abc","state":"wrong"}"#,
        );
    }
    let tmp = tempdir().unwrap();
    let err = login_with_native_browser(tmp.path()).await.unwrap_err();
    assert!(matches!(err, LoginError::StateMismatch));
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unset_env("CODEX_LOGIN_FORCE_STATE");
        unset_env("CODEX_LOGIN_TEST_HELPER_JSON");
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[serial_test::serial]
async fn invalid_helper_json_is_rejected() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        return;
    }
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_env("CODEX_LOGIN_TEST_HELPER_JSON", "not-json");
    }
    let tmp = tempdir().unwrap();
    let err = login_with_native_browser(tmp.path()).await.unwrap_err();
    match err {
        LoginError::InvalidHelperResponse => {}
        other => panic!("unexpected error: {other:?}"),
    }
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unset_env("CODEX_LOGIN_TEST_HELPER_JSON");
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[serial_test::serial]
async fn token_exchange_failure_is_bubbled() {
    use tiny_http::Server;
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        return;
    }

    // Issuer always responds 500 to /oauth/token for the first exchange
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = Server::from_listener(listener, None).unwrap();
    let _h = std::thread::spawn(move || {
        while let Ok(req) = server.recv() {
            if req.url().starts_with("/oauth/token") {
                let _ = req.respond(tiny_http::Response::from_string("oops").with_status_code(500));
            } else {
                let _ = req
                    .respond(tiny_http::Response::from_string("not found").with_status_code(404));
            }
        }
    });
    let issuer = format!("http://{}:{}", addr.ip(), addr.port());
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_env("CODEX_LOGIN_ISSUER_BASE", &issuer);
        set_env("CODEX_LOGIN_FORCE_STATE", "s");
        set_env(
            "CODEX_LOGIN_TEST_HELPER_JSON",
            r#"{"code":"abc","state":"s"}"#,
        );
    }
    let tmp = tempdir().unwrap();
    let err = login_with_native_browser(tmp.path()).await.unwrap_err();
    match err {
        LoginError::TokenExchangeFailed(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unset_env("CODEX_LOGIN_ISSUER_BASE");
        unset_env("CODEX_LOGIN_FORCE_STATE");
        unset_env("CODEX_LOGIN_TEST_HELPER_JSON");
    }
}

#[cfg(all(target_os = "macos", unix))]
#[tokio::test]
#[serial_test::serial]
async fn auth_json_permissions_are_restrictive() {
    use std::os::unix::fs::PermissionsExt;
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        return;
    }

    let (issuer_addr, _h) = start_mock_issuer();
    let issuer = format!("http://{}:{}", issuer_addr.ip(), issuer_addr.port());
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_env("CODEX_LOGIN_ISSUER_BASE", &issuer);
        set_env("CODEX_LOGIN_FORCE_STATE", "state-ok");
        set_env(
            "CODEX_LOGIN_TEST_HELPER_JSON",
            r#"{"code":"abc","state":"state-ok"}"#,
        );
    }

    let tmp = tempdir().unwrap();
    login_with_native_browser(tmp.path()).await.unwrap();
    let p = tmp.path().join("auth.json");
    let mode = std::fs::metadata(&p).unwrap().permissions().mode();
    // Only user perms should be set (rw------- => 0o600)
    assert_eq!(mode & 0o077, 0, "group/other permissions should be zero");
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unset_env("CODEX_LOGIN_ISSUER_BASE");
        unset_env("CODEX_LOGIN_FORCE_STATE");
        unset_env("CODEX_LOGIN_TEST_HELPER_JSON");
    }
}
