use codex_core::TurnInputRequest;
use codex_login::AuthHeaders;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_login::auth::BedrockAccessKeysAuth;
use codex_model_provider_info::AwsAuthRefreshConfig;
use codex_model_provider_info::AwsCredentialExportConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use codex_protocol::protocol::AuthRecoveryEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use codex_utils_redacted_string::RedactedString;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const CHATGPT_ACCOUNT_ID: &str = "workspace-one";
const INITIAL_ACCESS_TOKEN: &str = "header.e30.initial";
const REFRESHED_ACCESS_TOKEN: &str = "header.e30.refreshed";

struct ScriptedExternalAuth {
    current: Mutex<CodexAuth>,
    refreshed: CodexAuth,
}

impl ExternalAuth for ScriptedExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        let auth = self
            .current
            .lock()
            .map_err(|_| std::io::Error::other("external auth lock is poisoned"))
            .map(|current| current.clone());
        Box::pin(async move { auth })
    }

    fn refresh(&self, context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        let auth = if context.previous_account_id.as_deref() != Some(CHATGPT_ACCOUNT_ID) {
            Err(std::io::Error::other(
                "external auth refresh changed the ChatGPT workspace",
            ))
        } else {
            self.current
                .lock()
                .map_err(|_| std::io::Error::other("external auth lock is poisoned"))
                .map(|mut current| {
                    *current = self.refreshed.clone();
                    self.refreshed.clone()
                })
        };
        Box::pin(async move { auth })
    }
}

fn external_chatgpt_auth(access_token: &str) -> std::io::Result<CodexAuth> {
    CodexAuth::from_external_chatgpt_tokens(access_token, CHATGPT_ACCOUNT_ID, Some("enterprise"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn header_auth_is_attached_to_responses_requests() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer external"));
    headers.insert(
        "ChatGPT-Account-ID",
        HeaderValue::from_static("account-123"),
    );
    headers.insert("x-external-auth", HeaderValue::from_static("enabled"));
    let mut builder = test_codex().with_auth(CodexAuth::Headers(AuthHeaders::new(headers)));
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("hello").await?;

    let request = response_mock.single_request();
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer external")
    );
    assert_eq!(
        request.header("x-external-auth").as_deref(),
        Some("enabled")
    );
    assert_eq!(
        request.header("chatgpt-account-id").as_deref(),
        Some("account-123")
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_provider_does_not_receive_ambient_auth_headers() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer ambient"));
    headers.insert(
        "ChatGPT-Account-ID",
        HeaderValue::from_static("account-123"),
    );
    let provider =
        create_oss_provider_with_base_url(&format!("{}/v1", server.uri()), WireApi::Responses);
    let mut builder = test_codex()
        .with_auth(CodexAuth::Headers(AuthHeaders::new(headers)))
        .with_config(move |config| {
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("hello").await?;

    let request = response_mock.single_request();
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.header("chatgpt-account-id"), None);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_provider_uses_explicit_bearer_without_ambient_account() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer ambient"));
    headers.insert(
        "ChatGPT-Account-ID",
        HeaderValue::from_static("account-123"),
    );
    let mut provider =
        create_oss_provider_with_base_url(&format!("{}/v1", server.uri()), WireApi::Responses);
    provider.experimental_bearer_token = Some("provider-token".into());
    let mut builder = test_codex()
        .with_auth(CodexAuth::Headers(AuthHeaders::new(headers)))
        .with_config(move |config| {
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("hello").await?;

    let request = response_mock.single_request();
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer provider-token")
    );
    assert_eq!(request.header("chatgpt-account-id"), None);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_managed_access_keys_sign_requests() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let mut provider =
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: None,
            region: Some("us-east-1".to_string()),
            credential_export: None,
            auth_refresh: None,
        }));
    provider.base_url = Some(format!("{}/v1", server.uri()));

    let mut builder = test_codex()
        .with_model("openai.gpt-5.5")
        .with_auth(CodexAuth::BedrockAccessKeys(BedrockAccessKeysAuth {
            access_key_id: "managed-access-key-id".to_string(),
            secret_access_key: "managed-secret-access-key".to_string(),
            session_token: Some("managed-session-token".to_string()),
        }))
        .with_config(move |config| {
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("hello").await?;

    let request = response_mock.single_request();
    let authorization = request
        .header("authorization")
        .expect("managed AWS credentials should produce SigV4 authorization");
    assert!(authorization.starts_with("AWS4-HMAC-SHA256 "));
    assert!(authorization.contains("Credential=managed-access-key-id/"));
    assert_eq!(
        request.header("x-amz-security-token").as_deref(),
        Some("managed-session-token")
    );
    Ok(())
}

fn bedrock_credential_export_config(
    credentials_path: &Path,
    invocations_path: &Path,
) -> anyhow::Result<AwsCredentialExportConfig> {
    #[cfg(unix)]
    let (command, args) = {
        let script_path = credentials_path.with_extension("sh");
        codex_utils_cargo_bin::write_executable(
            &script_path,
            "#!/bin/sh\nprintf 'invoked\\n' >> \"$1\"\ncat \"$2\"\n",
        )?;
        (
            script_path.to_string_lossy().into_owned(),
            vec![
                invocations_path.to_string_lossy().into_owned(),
                credentials_path.to_string_lossy().into_owned(),
            ],
        )
    };

    #[cfg(windows)]
    let (command, args) = {
        let script_path = credentials_path.with_extension("cmd");
        std::fs::write(
            &script_path,
            "@echo off\r\n>> \"%~1\" echo invoked\r\ntype \"%~2\"\r\n",
        )?;
        (
            "cmd.exe".to_string(),
            vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                script_path.to_string_lossy().into_owned(),
                invocations_path.to_string_lossy().into_owned(),
                credentials_path.to_string_lossy().into_owned(),
            ],
        )
    };

    Ok(AwsCredentialExportConfig {
        command,
        args: args.into_iter().map(RedactedString::from).collect(),
        timeout_ms: NonZeroU64::new(5_000).expect("timeout should be non-zero"),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_credential_export_precedence_and_caching() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = tempfile::tempdir()?;
    let credentials_path = fixture.path().join("credentials.json");
    let invocations_path = fixture.path().join("invocations.txt");
    std::fs::write(
        &credentials_path,
        serde_json::to_vec(&serde_json::json!({
            "Credentials": {
                "AccessKeyId": "exported-access-key-id",
                "SecretAccessKey": "exported-secret-access-key",
                "SessionToken": "exported-session-token",
                "Expiration": "2099-01-01T00:00:00Z",
            },
        }))?,
    )?;

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let mut provider =
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: None,
            region: Some("us-east-1".to_string()),
            credential_export: Some(bedrock_credential_export_config(
                &credentials_path,
                &invocations_path,
            )?),
            auth_refresh: None,
        }));
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.request_max_retries = Some(0);
    provider.stream_max_retries = Some(2);

    let mut builder = test_codex()
        .with_model("openai.gpt-5.5")
        .with_auth(CodexAuth::BedrockAccessKeys(BedrockAccessKeysAuth {
            access_key_id: "managed-access-key-id".to_string(),
            secret_access_key: "managed-secret-access-key".to_string(),
            session_token: Some("managed-session-token".to_string()),
        }))
        .with_config(move |config| {
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("first request").await?;
    test.submit_turn("second request").await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    for request in requests {
        let authorization = request
            .header("authorization")
            .expect("exported AWS credentials should produce SigV4 authorization");
        assert!(authorization.starts_with("AWS4-HMAC-SHA256 "));
        assert!(authorization.contains("Credential=exported-access-key-id/"));
        assert_eq!(
            request.header("x-amz-security-token").as_deref(),
            Some("exported-session-token")
        );
    }
    let invocations = std::fs::read_to_string(&invocations_path)?;
    assert_eq!(invocations.lines().collect::<Vec<_>>(), vec!["invoked"]);

    std::fs::write(
        &credentials_path,
        serde_json::to_vec(&serde_json::json!({
            "Credentials": {
                "AccessKeyId": "rotated-access-key-id",
                "SecretAccessKey": "rotated-secret-access-key",
                "SessionToken": "rotated-session-token",
                "Expiration": "2099-01-01T00:00:00Z",
            },
        }))?,
    )?;
    let rotated_response_mock = mount_response_sequence(
        &server,
        vec![
            ResponseTemplate::new(403).set_body_string("ExpiredTokenException"),
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![
                    ev_response_created("resp-3"),
                    ev_completed("resp-3"),
                ])),
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![
                    ev_response_created("resp-4"),
                    ev_completed("resp-4"),
                ])),
        ],
    )
    .await;

    test.submit_turn("recover expired credentials").await?;
    test.submit_turn("reuse rotated credentials").await?;

    let rotated_requests = rotated_response_mock.requests();
    assert_eq!(rotated_requests.len(), 3);
    for (request, (access_key_id, session_token)) in rotated_requests.iter().zip([
        ("exported-access-key-id", "exported-session-token"),
        ("rotated-access-key-id", "rotated-session-token"),
        ("rotated-access-key-id", "rotated-session-token"),
    ]) {
        let authorization = request
            .header("authorization")
            .expect("exported AWS credentials should produce SigV4 authorization");
        assert!(authorization.contains(&format!("Credential={access_key_id}/")));
        assert_eq!(
            request.header("x-amz-security-token").as_deref(),
            Some(session_token)
        );
    }
    let invocations = std::fs::read_to_string(invocations_path)?;
    assert_eq!(
        invocations.lines().collect::<Vec<_>>(),
        vec!["invoked", "invoked"]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_aws_auth_refresh_resigns() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    const TEST_NAME: &str = "suite::external_auth::amazon_bedrock_aws_auth_refresh_resigns";
    const SUBPROCESS_ENV: &str = "CODEX_BEDROCK_AWS_REFRESH_TEST";
    const HELPER_ARG: &str = "CODEX_BEDROCK_AWS_REFRESH_COMMAND";
    const OLD: &str = "[default]\naws_access_key_id=OLD\naws_secret_access_key=s\n";
    const NEW: &str = "[default]\naws_access_key_id=NEW\naws_secret_access_key=s\n";

    if std::env::var_os(SUBPROCESS_ENV).is_none() {
        let fixture = tempfile::tempdir()?;
        let test_executable = std::env::current_exe()?;
        let aws_executable = fixture
            .path()
            .join(format!("aws{}", std::env::consts::EXE_SUFFIX));
        std::fs::hard_link(&test_executable, &aws_executable).or_else(|_| {
            codex_utils_cargo_bin::copy_executable(&test_executable, &aws_executable)
        })?;
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = std::env::split_paths(&inherited_path).collect::<Vec<_>>();
        paths.insert(/*index*/ 0, fixture.path().to_path_buf());
        let path = std::env::join_paths(paths)?;
        for mode in ["profile", "export", "export-failure", "export-expired"] {
            let credentials = fixture.path().join(format!("{mode}-credentials"));
            std::fs::write(&credentials, OLD)?;
            let mut command = Command::new(&test_executable);
            command
                .arg("--exact")
                .arg(TEST_NAME)
                .env(SUBPROCESS_ENV, mode)
                .env("PATH", &path)
                .env("AWS_SHARED_CREDENTIALS_FILE", &credentials)
                .env("AWS_CONFIG_FILE", &credentials)
                .env("AWS_EC2_METADATA_DISABLED", "true")
                .env_remove("AWS_ACCESS_KEY_ID")
                .env_remove("AWS_SECRET_ACCESS_KEY")
                .env_remove("AWS_BEARER_TOKEN_BEDROCK")
                .env_remove("AWS_WEB_IDENTITY_TOKEN_FILE")
                .env_remove("AWS_ROLE_ARN");
            let output = command.output().await?;
            assert!(output.status.success(), "{mode}: {output:?}");
        }
        return Ok(());
    }

    let mode = std::env::var(SUBPROCESS_ENV)?;
    let persistent_export_failure = mode == "export-failure";
    let credentials_path = PathBuf::from(std::env::var("AWS_SHARED_CREDENTIALS_FILE")?);
    let refresh_invocations = credentials_path.with_extension("refreshes");
    if std::env::args().any(|argument| argument == HELPER_ARG) {
        std::fs::write(&credentials_path, NEW)?;
        if !persistent_export_failure {
            std::fs::write(
                credentials_path.with_extension("json"),
                serde_json::to_vec(&serde_json::json!({
                    "Version": 1,
                    "AccessKeyId": "NEW",
                    "SecretAccessKey": "s",
                    "SessionToken": "exported-session-token",
                }))?,
            )?;
        }
        let mut invocations = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&refresh_invocations)?;
        std::io::Write::write_all(&mut invocations, b"refresh\n")?;
        return Ok(());
    }

    let export_mode = mode.starts_with("export");
    if mode == "export-expired" {
        std::fs::write(
            credentials_path.with_extension("json"),
            serde_json::to_vec(&serde_json::json!({
                "Version": 1,
                "AccessKeyId": "OLD",
                "SecretAccessKey": "s",
                "Expiration": "2000-01-01T00:00:00Z",
            }))?,
        )?;
    }
    let export_invocations = credentials_path.with_extension("exports");
    let credential_export = if export_mode {
        // Login creates or replaces this file after the initial export fails.
        Some(bedrock_credential_export_config(
            &credentials_path.with_extension("json"),
            &export_invocations,
        )?)
    } else {
        None
    };
    let server = start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(|request: &wiremock::Request| {
            let authorization = request.headers["authorization"]
                .to_str()
                .expect("SigV4 authorization should be valid");
            if authorization.contains("Credential=OLD/") {
                ResponseTemplate::new(401).set_body_string("ExpiredTokenException")
            } else {
                assert!(authorization.contains("Credential=NEW/"));
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse(vec![ev_response_created("r"), ev_completed("r")]))
            }
        })
        .expect(match mode.as_str() {
            "profile" => 2,
            "export-failure" => 0,
            _ => 1,
        })
        .mount(&server)
        .await;

    let mut provider =
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: (!export_mode).then(|| "default".to_string()),
            region: Some("us-east-1".to_string()),
            credential_export,
            auth_refresh: Some(AwsAuthRefreshConfig {
                command: "aws".to_string(),
                args: Vec::from(
                    ["--exact", TEST_NAME, "--skip", HELPER_ARG].map(RedactedString::from),
                ),
                timeout_ms: NonZeroU64::new(30_000).expect("timeout should be non-zero"),
            }),
        }));
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.request_max_retries = Some(0);
    let stream_max_retries = if export_mode { 2 } else { 0 };
    provider.stream_max_retries = Some(stream_max_retries);
    let recovery_attempts = stream_max_retries as usize + 1;

    let mut builder = test_codex()
        .with_model("openai.gpt-5.5")
        .with_config(move |config| {
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
        });
    let test = builder.build_with_auto_env(&server).await?;
    if persistent_export_failure {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "fail export even after login".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        let EventMsg::TurnComplete(completed) = wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await
        else {
            unreachable!("predicate guarantees a turn complete event");
        };
        let error = completed.error.expect("failed export should fail the turn");
        assert!(
            error
                .message
                .contains("AWS credential export command exited with"),
            "{error:?}"
        );
        assert_eq!(
            std::fs::read_to_string(refresh_invocations)?,
            "refresh\n".repeat(recovery_attempts)
        );
        assert_eq!(
            std::fs::read_to_string(export_invocations)?
                .lines()
                .collect::<Vec<_>>(),
            vec!["invoked"; 2 * recovery_attempts]
        );
        assert!(
            server
                .received_requests()
                .await
                .expect("mock server should capture requests")
                .is_empty()
        );
        server.verify().await;
        return Ok(());
    }
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let mut recovery_events = Vec::new();
    loop {
        match core_test_support::wait_for_event(&test.codex, |_| true).await {
            EventMsg::AuthRecoveryStarted(event) => recovery_events.push(("started", event)),
            EventMsg::AuthRecoveryCompleted(event) => recovery_events.push(("completed", event)),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert_eq!(
        recovery_events,
        vec![
            (
                "started",
                AuthRecoveryEvent {
                    provider: "Amazon Bedrock".to_string(),
                    message: "AWS session has expired. Reauthenticating...".to_string(),
                },
            ),
            (
                "completed",
                AuthRecoveryEvent {
                    provider: "Amazon Bedrock".to_string(),
                    message: "Signed in with AWS.".to_string(),
                },
            ),
        ]
    );
    server.verify().await;
    assert_eq!(std::fs::read_to_string(&refresh_invocations)?, "refresh\n");
    if export_mode {
        assert_eq!(
            std::fs::read_to_string(&export_invocations)?
                .lines()
                .collect::<Vec<_>>(),
            vec!["invoked", "invoked"]
        );
        let requests = server
            .received_requests()
            .await
            .expect("mock server should capture the exported request");
        assert_eq!(
            requests[0].headers.get("x-amz-security-token"),
            Some(&HeaderValue::from_static("exported-session-token"))
        );
    }

    server.reset().await;
    let rejected = mount_response_sequence(
        &server,
        vec![
            ResponseTemplate::new(403).set_body_string("ExpiredTokenException");
            2 * recovery_attempts
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "reject the refreshed credentials too".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let EventMsg::Error(error) =
        wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await
    else {
        unreachable!("predicate guarantees an error event");
    };
    assert!(error.message.contains("ExpiredTokenException"), "{error:?}");
    let EventMsg::TurnComplete(completed) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await
    else {
        unreachable!("predicate guarantees a turn complete event");
    };
    assert_eq!(completed.error, Some(error));
    assert_eq!(rejected.requests().len(), 2 * recovery_attempts);
    assert_eq!(
        std::fs::read_to_string(refresh_invocations)?,
        "refresh\n".repeat(1 + recovery_attempts)
    );
    if export_mode {
        assert_eq!(
            std::fs::read_to_string(export_invocations)?
                .lines()
                .collect::<Vec<_>>(),
            vec!["invoked"; 2 + recovery_attempts]
        );

        // A transient auth rejection can succeed on an outer stream retry.
        server.reset().await;
        let retried = mount_response_sequence(
            &server,
            vec![
                ResponseTemplate::new(403).set_body_string("ExpiredTokenException"),
                ResponseTemplate::new(403).set_body_string("ExpiredTokenException"),
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse(vec![
                        ev_response_created("retried"),
                        ev_completed("retried"),
                    ])),
            ],
        )
        .await;
        test.submit_turn("retry a temporary auth failure").await?;
        assert_eq!(retried.requests().len(), 3);
    }
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_auth_401_retry_uses_refreshed_chatgpt_headers() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "authorization",
            format!("Bearer {INITIAL_ACCESS_TOKEN}"),
        ))
        .and(header("chatgpt-account-id", CHATGPT_ACCOUNT_ID))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .and(header("chatgpt-account-id", CHATGPT_ACCOUNT_ID))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![
                    ev_response_created("resp-1"),
                    ev_completed("resp-1"),
                ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut builder = test_codex().with_auth(CodexAuth::from_api_key("seed"));
    let test = builder.build_with_auto_env(&server).await?;
    let external_auth = Arc::new(ScriptedExternalAuth {
        current: Mutex::new(external_chatgpt_auth(INITIAL_ACCESS_TOKEN)?),
        refreshed: external_chatgpt_auth(REFRESHED_ACCESS_TOKEN)?,
    });
    test.thread_manager
        .auth_manager()
        .set_external_auth(external_auth.clone())
        .await?;

    test.submit_turn("hello").await?;

    server.verify().await;
    let requests = server
        .received_requests()
        .await
        .expect("mock server should capture requests");
    let authorization_headers = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/responses")
        .filter_map(|request| request.headers.get("authorization"))
        .filter_map(|value| value.to_str().ok())
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        authorization_headers,
        vec![
            format!("Bearer {INITIAL_ACCESS_TOKEN}"),
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ]
    );
    Ok(())
}
