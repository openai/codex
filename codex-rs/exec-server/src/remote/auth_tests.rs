//! Exercise live authentication at the registry request boundary.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;

use super::*;
use crate::NoiseChannelIdentity;
use crate::relay::HarnessKeyValidator;
use crate::remote::RegistryHarnessKeyValidator;

#[derive(Default)]
struct RotatingAuth {
    refreshes: AtomicUsize,
    stall: Option<AuthStage>,
    reached: tokio::sync::Notify,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthStage {
    Initial,
    Refresh,
    Retry,
}

impl RotatingAuth {
    async fn stall_at(&self, stage: AuthStage) {
        if self.stall == Some(stage) {
            self.reached.notify_one();
            std::future::pending::<()>().await;
        }
    }
}

impl AuthProvider for RotatingAuth {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        let token = if self.refreshes.load(Ordering::SeqCst) == 0 {
            "Bearer old"
        } else {
            "Bearer new"
        };
        headers.insert(AUTHORIZATION, HeaderValue::from_static(token));
        Ok(())
    }

    fn resolve_auth_headers(&self) -> codex_api::AuthHeadersFuture<'_> {
        Box::pin(async {
            self.stall_at(if self.refreshes.load(Ordering::SeqCst) == 0 {
                AuthStage::Initial
            } else {
                AuthStage::Retry
            })
            .await;
            self.to_auth_headers()
        })
    }

    fn refresh_after_unauthorized(&self, rejected: HeaderMap) -> codex_api::AuthRefreshFuture<'_> {
        assert!(rejected.contains_key(AUTHORIZATION));
        Box::pin(async {
            self.stall_at(AuthStage::Refresh).await;
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            true
        })
    }
}

#[tokio::test]
async fn connect_deadline_covers_initial_auth_refresh_and_retry() -> anyhow::Result<()> {
    for stage in [AuthStage::Initial, AuthStage::Refresh, AuthStage::Retry] {
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::any())
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let provider = Arc::new(RotatingAuth {
            stall: Some(stage),
            ..Default::default()
        });
        let mut client = EnvironmentRegistryClient::new(server.uri(), provider.clone())?;
        client.connect_timeout = Duration::from_secs(60);
        let key = NoiseChannelIdentity::generate()?.public_key();
        let task = tokio::spawn(async move { client.connect_environment("env", key).await });
        provider.reached.notified().await;
        // Advance only after real socket I/O reached the selected auth stage.
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(61)).await;
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("connect must finish at its original deadline")?;
        assert!(
            matches!(result, Err(ExecServerError::EnvironmentRegistryRequest(error)) if error.is_timeout())
        );
        tokio::time::resume();
    }
    Ok(())
}

#[tokio::test]
async fn noise_registry_operations_retry_with_new_credentials() -> anyhow::Result<()> {
    for (operation, auth_mode) in [
        ("register", RegistryAuthMode::Legacy),
        ("connect", RegistryAuthMode::Legacy),
        ("validate", RegistryAuthMode::Legacy),
        ("register", RegistryAuthMode::ExecutorCredential),
        ("validate", RegistryAuthMode::ExecutorCredential),
    ] {
        let server = MockServer::start().await;
        let key = NoiseChannelIdentity::generate()?.public_key();
        Mock::given(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(header("authorization", "Bearer new"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "env", "url": "wss://relay.example/ws",
                "executor_registration_id": "registration",
                "security_profile": "noise_hybrid_ik_v1",
                "executor_public_key": key, "harness_key_authorization": "authorization",
                "valid": true,
            })))
            .expect(1)
            .mount(&server)
            .await;
        let provider = Arc::new(RotatingAuth::default());
        let mut client = EnvironmentRegistryClient::new(server.uri(), provider.clone())?;
        client.auth_mode = auth_mode;
        match operation {
            "register" => {
                client.register_environment_with_retry("env", &key).await?;
            }
            "connect" => {
                client.connect_environment("env", key).await?;
            }
            "validate" => {
                RegistryHarnessKeyValidator {
                    client,
                    environment_id: "env".into(),
                    executor_registration_id: "registration".into(),
                }
                .validate_harness_key(&key, "authorization", Some(uuid::Uuid::new_v4()))
                .await?;
            }
            _ => unreachable!(),
        }
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].body, requests[1].body);
        assert_eq!(
            requests[0].url.path(),
            format!("/cloud/environment/env/{operation}")
        );
        assert_eq!(
            requests[0].headers.get("x-codex-noise-stream-id"),
            requests[1].headers.get("x-codex-noise-stream-id")
        );
    }
    Ok(())
}

#[tokio::test]
async fn only_401_retries_and_only_once() -> anyhow::Result<()> {
    for (status, attempts, refreshes) in [(401, 2, 1), (403, 1, 0), (500, 1, 0)] {
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::any())
            .respond_with(ResponseTemplate::new(status))
            .expect(attempts)
            .mount(&server)
            .await;
        let provider = Arc::new(RotatingAuth::default());
        let client = EnvironmentRegistryClient::new(server.uri(), provider.clone())?;
        let (response, _) = client
            .send_authenticated(Request::new(Method::POST, server.uri()))
            .await?;
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), refreshes);
    }
    Ok(())
}
