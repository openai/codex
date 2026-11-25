#![cfg(test)]

//! Verify local voice calls use the selected auth mode and retain their credential lifecycle.

use super::Client;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use base64::Engine;
use codex_app_server_protocol::Thread;
use codex_config::LoaderOverrides;
use codex_config::config_toml::RealtimeWsMode;
use codex_features::Feature;
use codex_features::RuntimeAuthMode;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyController;
use codex_login::AuthCredentialsStoreMode;
use codex_login::login_with_api_key;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ModelProviderAuthInfo;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy_with_env;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

const KEY_A: &str = "test-realtime-key-a";
const KEY_B: &str = "test-realtime-key-b";

fn sign_in(config: &Config, key: &str) -> Result<()> {
    write_chatgpt_auth(
        config.codex_home.as_path(),
        ChatGptAuthFixture::new(key)
            .account_id(key)
            .chatgpt_account_id(key)
            .chatgpt_user_id(key),
        config.cli_auth_credentials_store_mode,
    )
}

async fn create_call(client: &Client, config: &Config) -> Result<()> {
    let thread: Thread = serde_json::from_value(json!({
        "id": "00000000-0000-0000-0000-000000000014",
        "sessionId": "00000000-0000-0000-0000-000000000014",
        "preview": "", "ephemeral": true, "modelProvider": "openai",
        "createdAt": 0, "updatedAt": 0, "status": {"type": "idle"},
        "cwd": config.cwd, "cliVersion": "test", "source": "cli", "turns": [],
    }))?;
    let call = client
        .create(config, &thread, "offer-sdp".to_string())
        .await
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    assert_eq!(
        (call.call_id, String::from(call.answer_sdp)),
        ("rtc_tui".to_string(), "answer-sdp".to_string()),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_calls_share_runtime_and_follow_auth_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let modes = [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly];
    if let Ok(server) = std::env::var(PROBE_SERVER) {
        for mode in modes {
            let home = tempfile::tempdir()?;
            let mut config = ConfigBuilder::default()
                .codex_home(home.path().to_path_buf())
                .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
                .build()
                .await?;
            config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::File;
            config.realtime.session_type = RealtimeWsMode::Conversational;
            config.runtime_auth_mode = mode;
            config
                .features
                .set_enabled(Feature::RuntimeAgentInstances, /*enabled*/ false)?;
            let controller = NetworkPolicyController::default();
            let policy = controller.policy();
            config.application_network_policy = policy.clone();
            sign_in(&config, KEY_A)?;
            assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
            let destination = "https://chatgpt.com/backend-api/codex/realtime/calls".parse()?;
            let account_a_permit = policy.acquire(&destination)?;
            let client = Client::default();
            create_call(&client, &config).await?;

            config.features.enable(Feature::RuntimeAgentInstances)?;
            tokio::try_join!(create_call(&client, &config), create_call(&client, &config))?;
            sign_in(&config, KEY_B)?;
            // The application owner refreshes the managed policy before starting the call.
            policy.invalidate();
            assert!(account_a_permit.check().is_err());
            assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
            let account_b_permit = policy.acquire(&destination)?;
            create_call(&client, &config).await?;
            account_b_permit.check()?;

            config
                .features
                .set_enabled(Feature::RuntimeAgentInstances, /*enabled*/ false)?;
            create_call(&client, &config).await?;
            config.features.enable(Feature::RuntimeAgentInstances)?;
            config.experimental_realtime_webrtc_call_base_url = Some(format!("{server}/v1"));
            create_call(&client, &config).await?;

            config.forced_login_method = Some(ForcedLoginMethod::Api);
            assert!(create_call(&client, &config).await.is_err());
        }
        return Ok(());
    }

    let server = MockServer::start().await;
    for (key, runtime) in [(KEY_A, "runtime-a"), (KEY_B, "runtime-b")] {
        Mock::given(method("POST"))
            .and(path("/agent/register"))
            .and(body_partial_json(
                json!({"actor_auth": {"credential": key}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": runtime, "agent_host_id": "host-tui",
                "actor": {"kind": "chatgpt", "user_id": key, "account_id": key},
            })))
            .expect(2)
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/backend-api/codex/realtime/calls"))
        .respond_with(|request: &wiremock::Request| {
            if !request.headers.contains_key("authorization") {
                return ResponseTemplate::new(401);
            }
            ResponseTemplate::new(201)
                .insert_header("location", "/realtime/calls/rtc_tui")
                .set_body_string("answer-sdp")
        })
        .mount(&server)
        .await;
    run_harness_through_proxy_with_env(
        &server,
        "realtime_call::tests::local_calls_share_runtime_and_follow_auth_changes",
        /*realtime_target*/ None,
        &[
            ("CODEX_API_KEY", None),
            ("OPENAI_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", None),
        ],
    )
    .await?;
    server.verify().await;
    let requests = server
        .received_requests()
        .await
        .context("captured requests")?;
    let calls = requests
        .iter()
        .filter(|request| request.url.path() == "/backend-api/codex/realtime/calls")
        .collect::<Vec<_>>();
    for request in &calls {
        if !request.headers.contains_key("authorization") {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        }
    }
    let calls = calls
        .into_iter()
        .filter(|request| request.headers.contains_key("authorization"))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 12);
    let mut proofs = BTreeSet::new();
    for (mode, calls) in modes.into_iter().zip(calls.as_chunks::<6>().0) {
        for (index, request) in calls.iter().enumerate() {
            let key = if index < 3 { KEY_A } else { KEY_B };
            let authorization = request.headers["authorization"].to_str()?;
            if matches!(index, 0 | 4) {
                assert_eq!(authorization, format!("Bearer {key}"));
                assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                continue;
            }
            let proof = match mode {
                RuntimeAuthMode::DualHeader => {
                    assert_eq!(authorization, format!("Bearer {key}"));
                    request.headers["x-openai-agent-assertion"].to_str()?
                }
                RuntimeAuthMode::AssertionOnly => {
                    assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                    authorization
                        .strip_prefix("AgentAssertion ")
                        .context("runtime auth")?
                }
            };
            assert!(
                proofs.insert(proof.to_string()),
                "each call needs a fresh assertion"
            );
            let payload = proof
                .strip_prefix("1.")
                .context("assertion version")?
                .split('.')
                .nth(1)
                .context("assertion payload")?;
            let claims: Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)?,
            )?;
            assert_eq!(
                claims["runtime_agent_instance"],
                if index < 3 { "runtime-a" } else { "runtime-b" },
            );
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_calls_require_chatgpt_even_with_provider_credentials() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var(PROBE_SERVER).is_ok() {
        let home = tempfile::tempdir()?;
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::File;
        config.realtime.session_type = RealtimeWsMode::Conversational;
        config
            .features
            .set_enabled(Feature::RuntimeAgentInstances, /*enabled*/ false)?;
        let client = Client::default();
        login_with_api_key(
            config.codex_home.as_path(),
            KEY_A,
            config.cli_auth_credentials_store_mode,
            config.auth_keyring_backend_kind(),
        )?;
        assert!(create_call(&client, &config).await.is_err());
        sign_in(&config, KEY_A)?;
        create_call(&client, &config).await?;
        std::fs::write(home.path().join("auth.json"), "malformed credentials")?;

        let original_provider = config.model_provider.clone();
        config.model_provider.experimental_bearer_token = Some(KEY_B.into());
        assert!(create_call(&client, &config).await.is_err());

        config.model_provider = original_provider.clone();
        config.model_provider.env_key = Some("TUI_TEST_PROVIDER_KEY".to_string());
        assert!(create_call(&client, &config).await.is_err());

        config.model_provider = original_provider.clone();
        config.model_provider.requires_openai_auth = false;
        let (command, args) = if cfg!(windows) {
            ("cmd.exe", vec!["/C".to_string(), format!("echo {KEY_B}")])
        } else {
            ("sh", vec!["-c".to_string(), format!("printf %s {KEY_B}")])
        };
        config.model_provider.auth = Some(ModelProviderAuthInfo {
            command: command.to_string(),
            args: args.into_iter().map(Into::into).collect(),
            timeout_ms: NonZeroU64::new(5_000).context("nonzero timeout")?,
            refresh_interval_ms: 300_000,
            cwd: config.cwd.clone(),
        });
        assert!(create_call(&client, &config).await.is_err());

        config.model_provider = original_provider;
        assert!(create_call(&client, &config).await.is_err());
        return Ok(());
    }

    let server = MockServer::start().await;
    for (key, calls) in [(KEY_A, 1), (KEY_B, 0)] {
        Mock::given(method("POST"))
            .and(path("/backend-api/codex/realtime/calls"))
            .and(wiremock::matchers::header(
                "authorization",
                format!("Bearer {key}"),
            ))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("location", "/realtime/calls/rtc_tui")
                    .set_body_string("answer-sdp"),
            )
            .expect(calls)
            .mount(&server)
            .await;
    }
    run_harness_through_proxy_with_env(
        &server,
        "realtime_call::tests::local_calls_require_chatgpt_even_with_provider_credentials",
        /*realtime_target*/ None,
        &[
            ("CODEX_API_KEY", None),
            ("OPENAI_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", None),
            ("TUI_TEST_PROVIDER_KEY", Some(KEY_B)),
        ],
    )
    .await?;
    server.verify().await;
    Ok(())
}
