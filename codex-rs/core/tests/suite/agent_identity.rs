#![cfg(not(target_os = "windows"))]

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use codex_app_server_protocol::AuthMode;
use codex_core::CodexAuth;
use codex_core::X_OPENAI_INTERNAL_CODEX_TASK_ID_HEADER;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::AuthDotJson;
use codex_core::auth::save_auth;
use codex_core::features::Feature;
use codex_core::token_data::IdTokenInfo;
use codex_core::token_data::TokenData;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use crypto_box::PublicKey as CryptoBoxPublicKey;
use ed25519_dalek::VerifyingKey;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

#[derive(Clone, Default)]
struct RegisteredAgentState {
    public_key: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
struct AgentRegisterResponder {
    state: RegisteredAgentState,
}

impl Respond for AgentRegisterResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let public_key = request_json_body(request)
            .get("agent_public_key")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing agent_public_key"))
            .to_string();
        *self
            .state
            .public_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(public_key);
        ResponseTemplate::new(200).set_body_json(json!({
            "agent_runtime_id": "agent-runtime-1",
        }))
    }
}

#[derive(Clone)]
struct EncryptedTaskResponder {
    state: RegisteredAgentState,
    task_ids: Arc<Mutex<VecDeque<String>>>,
}

impl EncryptedTaskResponder {
    fn new(state: RegisteredAgentState, task_ids: Vec<&str>) -> Self {
        Self {
            state,
            task_ids: Arc::new(Mutex::new(VecDeque::from(
                task_ids.into_iter().map(str::to_string).collect::<Vec<_>>(),
            ))),
        }
    }
}

impl Respond for EncryptedTaskResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let agent_public_key = self
            .state
            .public_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| panic!("agent registration should happen before task registration"));
        let task_id = self
            .task_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| panic!("missing queued task id"));
        let encrypted_task_id = encrypt_task_id_for_public_key(&agent_public_key, &task_id);
        ResponseTemplate::new(200).set_body_json(json!({
            "encrypted_task_id": encrypted_task_id,
        }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn feature_off_skips_agent_identity_requests() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let base_url = server.uri();
            move |config| {
                config.chatgpt_base_url = base_url;
                config.model_provider.supports_websockets = false;
            }
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policy("hello", workspace_write_with_network())
        .await?;

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        count_requests_for_path(&requests, "/backend-api/v1/agent/register"),
        0
    );
    assert_eq!(
        count_requests_for_path(
            &requests,
            "/backend-api/v1/agent/agent-runtime-1/task/register"
        ),
        0
    );
    assert_eq!(
        response_mock
            .single_request()
            .header(X_OPENAI_INTERNAL_CODEX_TASK_ID_HEADER),
        None
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_identity_registers_once_and_reuses_task_within_thread() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_agent_identity_endpoints(&server, vec!["task-1"]).await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let base_url = server.uri();
            move |config| {
                config.chatgpt_base_url = base_url;
                config.model_provider.supports_websockets = false;
                config
                    .features
                    .enable(Feature::UseAgentIdentity)
                    .expect("test config should allow feature update");
            }
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policy("hello", workspace_write_with_network())
        .await?;
    test.submit_turn_with_policy("again", workspace_write_with_network())
        .await?;

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        count_requests_for_path(&requests, "/backend-api/v1/agent/register"),
        1
    );
    assert_eq!(
        count_requests_for_path(
            &requests,
            "/backend-api/v1/agent/agent-runtime-1/task/register"
        ),
        1
    );
    assert_eq!(response_mock.requests().len(), 2);

    let register_request = request_json_body(
        requests
            .iter()
            .find(|request| request.url.path() == "/backend-api/v1/agent/register")
            .expect("missing agent register request"),
    );
    assert!(
        register_request["agent_public_key"]
            .as_str()
            .unwrap_or_default()
            .starts_with("ssh-ed25519 ")
    );
    assert!(register_request.get("metadata").is_none());
    assert!(register_request.get("on_behalf_of").is_none());

    let task_request = request_json_body(
        requests
            .iter()
            .find(|request| {
                request.url.path() == "/backend-api/v1/agent/agent-runtime-1/task/register"
            })
            .expect("missing task register request"),
    );
    assert!(task_request.get("agent_id").is_none());
    assert!(task_request.get("agent_runtime_id").is_none());
    assert!(task_request.get("metadata").is_none());
    assert!(
        !task_request["timestamp"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
    );
    assert!(
        !task_request["signature"]
            .as_str()
            .unwrap_or_default()
            .is_empty()
    );

    for request in response_mock.requests() {
        assert_eq!(
            request
                .header(X_OPENAI_INTERNAL_CODEX_TASK_ID_HEADER)
                .as_deref(),
            Some("task-1")
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_thread_reuses_agent_identity_but_gets_new_task() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    mount_agent_identity_endpoints(&server, vec!["task-1", "task-2"]).await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let mut first_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let base_url = server.uri();
            move |config| {
                config.chatgpt_base_url = base_url;
                config.model_provider.supports_websockets = false;
                config
                    .features
                    .enable(Feature::UseAgentIdentity)
                    .expect("test config should allow feature update");
            }
        });
    let first = first_builder.build(&server).await?;
    first
        .submit_turn_with_policy("hello", workspace_write_with_network())
        .await?;

    let mut second_builder = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let base_url = server.uri();
            move |config| {
                config.chatgpt_base_url = base_url;
                config.model_provider.supports_websockets = false;
                config
                    .features
                    .enable(Feature::UseAgentIdentity)
                    .expect("test config should allow feature update");
            }
        });
    let second = second_builder.build(&server).await?;
    second
        .submit_turn_with_policy("hello from new thread", workspace_write_with_network())
        .await?;

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        count_requests_for_path(&requests, "/backend-api/v1/agent/register"),
        1
    );
    assert_eq!(
        count_requests_for_path(
            &requests,
            "/backend-api/v1/agent/agent-runtime-1/task/register"
        ),
        2
    );

    let response_requests = response_mock.requests();
    assert_eq!(response_requests.len(), 2);
    assert_eq!(
        response_requests[0]
            .header(X_OPENAI_INTERNAL_CODEX_TASK_ID_HEADER)
            .as_deref(),
        Some("task-1")
    );
    assert_eq!(
        response_requests[1]
            .header(X_OPENAI_INTERNAL_CODEX_TASK_ID_HEADER)
            .as_deref(),
        Some("task-2")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_account_change_registers_a_new_agent_identity() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    mount_agent_identity_endpoints(&server, vec!["task-1", "task-2"]).await;
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![
                    ev_response_created("resp-1"),
                    ev_completed("resp-1"),
                ])),
        )
        .mount(&server)
        .await;

    let mut first_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_auth(chatgpt_auth_for_account(home.path(), "account-1")?)
        .with_config({
            let base_url = server.uri();
            move |config| {
                config.chatgpt_base_url = base_url;
                config.model_provider.supports_websockets = false;
                config
                    .features
                    .enable(Feature::UseAgentIdentity)
                    .expect("test config should allow feature update");
            }
        });
    let first = first_builder.build(&server).await?;
    first
        .submit_turn_with_policy("hello", workspace_write_with_network())
        .await?;

    let mut second_builder = test_codex()
        .with_home(home)
        .with_auth(chatgpt_auth_for_account(first.home.path(), "account-2")?)
        .with_config({
            let base_url = server.uri();
            move |config| {
                config.chatgpt_base_url = base_url;
                config.model_provider.supports_websockets = false;
                config
                    .features
                    .enable(Feature::UseAgentIdentity)
                    .expect("test config should allow feature update");
            }
        });
    let second = second_builder.build(&server).await?;
    second
        .submit_turn_with_policy("hello from second account", workspace_write_with_network())
        .await?;

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        count_requests_for_path(&requests, "/backend-api/v1/agent/register"),
        2
    );
    assert_eq!(
        count_requests_for_path(
            &requests,
            "/backend-api/v1/agent/agent-runtime-1/task/register"
        ),
        2
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_key_auth_bypasses_agent_identity_flow() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = server.uri();
        move |config| {
            config.chatgpt_base_url = base_url;
            config.model_provider.supports_websockets = false;
            config
                .features
                .enable(Feature::UseAgentIdentity)
                .expect("test config should allow feature update");
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policy("hello", workspace_write_with_network())
        .await?;

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        count_requests_for_path(&requests, "/backend-api/v1/agent/register"),
        0
    );
    assert_eq!(
        count_requests_for_path(
            &requests,
            "/backend-api/v1/agent/agent-runtime-1/task/register"
        ),
        0
    );
    assert_eq!(
        response_mock
            .single_request()
            .header(X_OPENAI_INTERNAL_CODEX_TASK_ID_HEADER),
        None
    );

    Ok(())
}

async fn mount_agent_identity_endpoints(server: &wiremock::MockServer, task_ids: Vec<&str>) {
    let state = RegisteredAgentState::default();
    Mock::given(method("POST"))
        .and(path("/backend-api/v1/agent/register"))
        .respond_with(AgentRegisterResponder {
            state: state.clone(),
        })
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex("/backend-api/v1/agent/[^/]+/task/register"))
        .respond_with(EncryptedTaskResponder::new(state, task_ids))
        .mount(server)
        .await;
}

fn count_requests_for_path(requests: &[wiremock::Request], expected_path: &str) -> usize {
    requests
        .iter()
        .filter(|request| request.url.path() == expected_path)
        .count()
}

fn request_json_body(request: &wiremock::Request) -> Value {
    match serde_json::from_slice(&request.body) {
        Ok(body) => body,
        Err(err) => panic!("request body should be valid JSON: {err}"),
    }
}

fn encrypt_task_id_for_public_key(agent_public_key: &str, task_id: &str) -> String {
    let verifying_key = decode_ssh_ed25519_public_key(agent_public_key);
    let recipient_public_key = CryptoBoxPublicKey::from(verifying_key.to_montgomery().to_bytes());
    let encrypted_task_id = recipient_public_key
        .seal(&mut crypto_box::aead::OsRng, task_id.as_bytes())
        .unwrap_or_else(|err| panic!("encrypt task id: {err}"));
    BASE64_STANDARD.encode(encrypted_task_id)
}

fn decode_ssh_ed25519_public_key(agent_public_key: &str) -> VerifyingKey {
    let mut parts = agent_public_key.split_whitespace();
    let key_type = parts.next().unwrap_or_default();
    let encoded = parts
        .next()
        .unwrap_or_else(|| panic!("missing SSH public key payload"));
    assert_eq!(key_type, "ssh-ed25519");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .unwrap_or_else(|err| panic!("decode SSH public key payload: {err}"));
    let mut cursor = decoded.as_slice();
    let key_type_blob = read_ssh_string(&mut cursor);
    assert_eq!(key_type_blob, b"ssh-ed25519");
    let key_bytes = read_ssh_string(&mut cursor);
    assert!(
        cursor.is_empty(),
        "unexpected trailing SSH public key bytes"
    );
    let key_bytes: [u8; 32] = key_bytes
        .try_into()
        .unwrap_or_else(|_| panic!("ssh-ed25519 key should be 32 bytes"));
    VerifyingKey::from_bytes(&key_bytes)
        .unwrap_or_else(|err| panic!("build verifying key from SSH payload: {err}"))
}

fn read_ssh_string<'a>(cursor: &mut &'a [u8]) -> &'a [u8] {
    let (length_bytes, rest) = cursor
        .split_at_checked(4)
        .unwrap_or_else(|| panic!("missing SSH string length"));
    let length = u32::from_be_bytes(
        length_bytes
            .try_into()
            .unwrap_or_else(|_| panic!("SSH string length should be exactly 4 bytes")),
    ) as usize;
    let (value, remaining) = rest
        .split_at_checked(length)
        .unwrap_or_else(|| panic!("SSH string length {length} exceeds payload size"));
    *cursor = remaining;
    value
}

fn workspace_write_with_network() -> codex_protocol::protocol::SandboxPolicy {
    codex_protocol::protocol::SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: true,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

fn chatgpt_auth_for_account(codex_home: &std::path::Path, account_id: &str) -> Result<CodexAuth> {
    let mut id_token = IdTokenInfo::default();
    id_token.raw_jwt = fake_chatgpt_id_token(account_id)?;
    let auth_dot_json = AuthDotJson {
        auth_mode: Some(AuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token,
            access_token: format!("access-token-{account_id}"),
            refresh_token: "refresh-token".to_string(),
            account_id: Some(account_id.to_string()),
        }),
        last_refresh: Some(Utc::now()),
    };
    save_auth(codex_home, &auth_dot_json, AuthCredentialsStoreMode::File)?;
    let Some(auth) = CodexAuth::from_auth_storage(codex_home, AuthCredentialsStoreMode::File)?
    else {
        anyhow::bail!("auth should load from storage");
    };
    Ok(auth)
}

fn fake_chatgpt_id_token(account_id: &str) -> Result<String> {
    #[derive(serde::Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }

    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = json!({
        "email": "test@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_user_id": "user-123",
            "chatgpt_account_id": account_id,
        },
    });

    let header_bytes = serde_json::to_vec(&header)?;
    let payload_bytes = serde_json::to_vec(&payload)?;
    let header_b64 = URL_SAFE_NO_PAD.encode(header_bytes);
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_bytes);
    let signature_b64 = URL_SAFE_NO_PAD.encode("sig");
    Ok(format!("{header_b64}.{payload_b64}.{signature_b64}"))
}
