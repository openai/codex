use codex_login::AuthHeaders;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_model_provider_info::AwsAuthRefreshConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use std::num::NonZeroU64;
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
    provider.experimental_bearer_token = Some("provider-token".to_string());
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
        std::fs::hard_link(&test_executable, &aws_executable)
            .or_else(|_| std::fs::copy(&test_executable, &aws_executable).map(|_| ()))?;
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = std::env::split_paths(&inherited_path).collect::<Vec<_>>();
        paths.insert(/*index*/ 0, fixture.path().to_path_buf());
        let credentials = fixture.path().join("credentials");
        std::fs::write(&credentials, OLD)?;
        let mut command = Command::new(test_executable);
        command
            .arg("--exact")
            .arg(TEST_NAME)
            .env(SUBPROCESS_ENV, "1")
            .env("PATH", std::env::join_paths(paths)?)
            .env("AWS_SHARED_CREDENTIALS_FILE", &credentials)
            .env("AWS_CONFIG_FILE", &credentials)
            .env("AWS_EC2_METADATA_DISABLED", "true")
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env_remove("AWS_BEARER_TOKEN_BEDROCK")
            .env_remove("AWS_WEB_IDENTITY_TOKEN_FILE")
            .env_remove("AWS_ROLE_ARN");
        let output = command.output().await?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }

    if std::env::args().any(|argument| argument == HELPER_ARG) {
        std::fs::write(std::env::var("AWS_SHARED_CREDENTIALS_FILE")?, NEW)?;
        return Ok(());
    }

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
        .expect(2)
        .mount(&server)
        .await;

    let mut provider =
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: Some("default".to_string()),
            region: Some("us-east-1".to_string()),
            auth_refresh: Some(AwsAuthRefreshConfig {
                command: "aws".to_string(),
                args: Vec::from(["--exact", TEST_NAME, "--skip", HELPER_ARG].map(str::to_string)),
                timeout_ms: NonZeroU64::new(30_000).expect("timeout should be non-zero"),
            }),
        }));
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.request_max_retries = Some(0);
    provider.stream_max_retries = Some(0);

    let mut builder = test_codex()
        .with_model("openai.gpt-5.5")
        .with_config(move |config| {
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("hello").await?;
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
