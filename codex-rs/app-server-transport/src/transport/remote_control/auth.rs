//! Credentials and recovery bound to one remote-control login lifetime.
//! A request can refresh credentials, but cannot adopt a replacement authentication owner.

use axum::http::HeaderMap;
use axum::http::HeaderValue;
use codex_api::SharedAuthProvider;
use codex_features::RuntimeAuthMode;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::UnauthorizedRecovery;
use codex_runtime_agent_instance::RuntimeAuthSlot;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::info;
use tracing::warn;

#[derive(Clone)]
pub(super) struct RemoteControlAuth {
    manager: Arc<AuthManager>,
    runtime_auth: Option<(RuntimeAuthSlot, RuntimeAuthMode)>,
    pub(super) owner: crate::ConnectionAuth,
    pub(super) network_policy: codex_http_client::NetworkPolicy,
}

pub(super) struct RemoteControlRecovery {
    auth: RemoteControlAuth,
    recovery: UnauthorizedRecovery,
}

impl RemoteControlAuth {
    pub(super) fn capture(
        manager: Arc<AuthManager>,
        runtime_auth: Option<(RuntimeAuthSlot, RuntimeAuthMode)>,
    ) -> (Self, bool) {
        loop {
            let owner = crate::ConnectionAuth::capture(&manager);
            let authenticated = manager
                .auth_cached()
                .is_some_and(|auth| auth.uses_codex_backend() && auth.get_account_id().is_some());
            if owner.is_current() {
                return (
                    Self {
                        network_policy: manager.application_network_policy(),
                        manager,
                        runtime_auth,
                        owner,
                    },
                    authenticated,
                );
            }
        }
    }

    pub(super) fn ensure_current(&self) -> io::Result<()> {
        self.owner.ensure_current()
    }

    pub(super) fn unauthorized_recovery(&self) -> RemoteControlRecovery {
        RemoteControlRecovery {
            auth: self.clone(),
            recovery: self.manager.unauthorized_recovery(),
        }
    }

    pub(super) fn auth_change_receiver(&self) -> watch::Receiver<u64> {
        self.manager.auth_change_receiver()
    }
}

pub(super) use codex_http_client::CHATGPT_ACCOUNT_ID_HEADER as REMOTE_CONTROL_ACCOUNT_ID_HEADER;

pub(super) struct RemoteControlConnectionAuth {
    pub(super) http_client_factory: codex_http_client::HttpClientFactory,
    pub(super) auth: CodexAuth,
    pub(super) runtime_auth: Option<(RuntimeAuthSlot, RuntimeAuthMode)>,
    pub(super) account_id: String,
}

impl RemoteControlConnectionAuth {
    pub(super) async fn request_context(
        &self,
        endpoint: &str,
    ) -> io::Result<(codex_http_client::HttpClient, HeaderMap)> {
        if self.runtime_auth.is_some() {
            codex_model_provider::validate_runtime_destination(Some(endpoint))
                .map_err(io::Error::other)?;
        }
        let runtime = self
            .runtime_auth
            .as_ref()
            .filter(|_| codex_runtime_agent_instance::is_openai_api_url(endpoint))
            .map(|(slot, mode)| {
                slot.get()
                    .cloned()
                    .map(|manager| (manager, *mode))
                    .ok_or_else(|| {
                        io::Error::new(
                            ErrorKind::WouldBlock,
                            "runtime authentication is not initialized",
                        )
                    })
            })
            .transpose()?;
        // Keep runtime auth resolution from growing the caller's stack frame.
        let resolved = Box::pin(codex_model_provider::auth_provider_for_request(
            &self.auth,
            runtime.as_ref(),
            &HeaderMap::new(),
        ))
        .await
        .map_err(|error| auth_error(error.into()))?;
        let factory = resolved
            .bind_http_client_factory(self.http_client_factory.clone())
            .map_err(auth_error)?;
        let client = request_client(&factory, endpoint).await?;
        let headers = request_headers(&resolved.auth, &self.account_id).await?;
        resolved
            .validate_headers(&headers)
            .map_err(io::Error::other)?;
        Ok((client, headers))
    }
}

async fn request_headers(
    auth_provider: &SharedAuthProvider,
    account_id: &str,
) -> io::Result<HeaderMap> {
    let mut headers = auth_provider
        .apply_auth_headers(codex_login::default_client::default_headers())
        .await
        .map_err(auth_error)?;
    headers.insert(
        REMOTE_CONTROL_ACCOUNT_ID_HEADER,
        HeaderValue::from_str(account_id).map_err(|err| {
            io::Error::new(
                ErrorKind::InvalidInput,
                format!("invalid remote control account id header: {err}"),
            )
        })?,
    );
    Ok(headers)
}

pub(super) async fn load_remote_control_auth(
    auth: &RemoteControlAuth,
) -> io::Result<RemoteControlConnectionAuth> {
    auth.ensure_current()?;
    let credentials = load_auth_manager(&auth.manager, auth.runtime_auth.clone()).await?;
    auth.ensure_current()?;
    Ok(credentials)
}

async fn load_auth_manager(
    auth_manager: &Arc<AuthManager>,
    runtime_auth: Option<(RuntimeAuthSlot, RuntimeAuthMode)>,
) -> io::Result<RemoteControlConnectionAuth> {
    let mut reloaded = false;
    let (auth, http_client_factory) = loop {
        let Some((auth, http_client_factory)) = auth_manager.auth_with_http_client_factory().await
        else {
            if reloaded {
                return Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "remote control requires ChatGPT authentication",
                ));
            }
            auth_manager.reload().await;
            reloaded = true;
            continue;
        };
        if !auth.uses_codex_backend() {
            break (auth, http_client_factory);
        }
        if auth.get_account_id().is_none() && !reloaded {
            auth_manager.reload().await;
            reloaded = true;
            continue;
        }
        break (auth, http_client_factory);
    };

    if !auth.uses_codex_backend() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "remote control requires ChatGPT authentication; API key auth is not supported",
        ));
    }

    Ok(RemoteControlConnectionAuth {
        http_client_factory,
        runtime_auth,
        account_id: auth.get_account_id().ok_or_else(|| {
            io::Error::new(
                ErrorKind::WouldBlock,
                "remote control enrollment is waiting for a ChatGPT account id",
            )
        })?,
        auth,
    })
}

pub(super) async fn request_client(
    factory: &codex_http_client::HttpClientFactory,
    endpoint: &str,
) -> io::Result<codex_http_client::HttpClient> {
    codex_login::default_client::create_client_for_route_without_request_logging_async(
        factory.clone(),
        endpoint.to_string(),
        codex_http_client::ClientRouteClass::Api,
    )
    .await
}

pub(super) fn is_policy_denial(error: &io::Error) -> bool {
    let Some(cause) = error.get_ref() else {
        return false;
    };
    cause.is::<codex_http_client::NetworkPolicyDenied>()
}

pub(super) fn is_auth_error(error: &io::Error) -> bool {
    error.kind() == ErrorKind::PermissionDenied && !is_policy_denial(error)
}

fn auth_error(error: codex_api::AuthError) -> io::Error {
    match error {
        codex_api::AuthError::Policy(denied) => io::Error::new(ErrorKind::PermissionDenied, denied),
        error => io::Error::other(error),
    }
}

pub(super) fn request_error(error: codex_http_client::HttpError) -> io::Error {
    match error {
        codex_http_client::HttpError::Policy(denied) => {
            io::Error::new(ErrorKind::PermissionDenied, denied)
        }
        error => io::Error::other(error),
    }
}

pub(super) async fn recover_remote_control_auth(
    recovery: &mut RemoteControlRecovery,
    auth_change_rx: &mut watch::Receiver<u64>,
) -> bool {
    if recovery.auth.ensure_current().is_err() {
        return false;
    }
    let auth_recovery = &mut recovery.recovery;
    if !auth_recovery.has_next() {
        return false;
    }

    let mode = auth_recovery.mode_name();
    let step = auth_recovery.step_name();
    let auth_change_revision_before_recovery = *auth_change_rx.borrow();
    match auth_recovery.next().await {
        Ok(step_result) => {
            if recovery.auth.ensure_current().is_err() {
                return false;
            }
            if step_result.auth_state_changed() == Some(true) {
                mark_recovery_auth_change_seen(
                    auth_change_rx,
                    auth_change_revision_before_recovery,
                );
            }
            info!(
                "remote control auth recovery succeeded: mode={mode}, step={step}, auth_state_changed={:?}",
                step_result.auth_state_changed()
            );
            true
        }
        Err(err) => {
            warn!("remote control auth recovery failed: mode={mode}, step={step}: {err}");
            false
        }
    }
}

pub(super) fn mark_recovery_auth_change_seen(
    auth_change_rx: &mut watch::Receiver<u64>,
    auth_change_revision_before_recovery: u64,
) {
    let auth_change_revision_after_recovery = *auth_change_rx.borrow();
    if auth_change_revision_after_recovery == auth_change_revision_before_recovery.wrapping_add(1) {
        // Recovery updated the same watch that wakes the outer reconnect
        // loop. Mark only that single revision seen; if more revisions
        // arrived while recovery was in flight, leave them pending so the
        // reconnect loop still reacts to the later external auth change.
        auth_change_rx.borrow_and_update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_api::AuthProvider;
    use pretty_assertions::assert_eq;

    #[derive(Debug)]
    struct TestAuthProvider {
        account_ids: Vec<&'static str>,
    }

    impl AuthProvider for TestAuthProvider {
        fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
            headers.insert(
                axum::http::header::AUTHORIZATION,
                HeaderValue::from_static("Bearer test-token"),
            );
            headers.insert("x-openai-fedramp", HeaderValue::from_static("true"));
            for account_id in &self.account_ids {
                headers.append("ChatGPT-Account-ID", HeaderValue::from_static(account_id));
            }
            Ok(())
        }
    }

    async fn headers_for(
        account_id: &str,
        account_ids: Vec<&'static str>,
    ) -> io::Result<HeaderMap> {
        let provider: SharedAuthProvider = Arc::new(TestAuthProvider { account_ids });
        request_headers(&provider, account_id).await
    }

    #[tokio::test]
    async fn runtime_requests_reject_unsupported_openai_origins() -> anyhow::Result<()> {
        let credentials =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let (owner, _) = RemoteControlAuth::capture(
                Arc::clone(&credentials),
                Some((RuntimeAuthSlot::default(), mode)),
            );
            let auth = load_remote_control_auth(&owner).await?;
            for endpoint in [
                "https://chatgpt.com:8443/backend-api",
                "http://chatgpt.com/backend-api",
            ] {
                assert!(
                    auth.request_context(endpoint).await.is_err(),
                    "unsupported OpenAI origins must not fall back to bearer auth"
                );
            }

            let (_, headers) = auth
                .request_context("https://remote.example/control")
                .await?;
            assert_eq!(
                headers[axum::http::header::AUTHORIZATION],
                "Bearer Access Token"
            );
            assert_eq!(headers[REMOTE_CONTROL_ACCOUNT_ID_HEADER], "account_id");
            assert!(!headers.contains_key("x-openai-agent-assertion"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn runtime_request_rejects_a_different_registered_account() -> anyhow::Result<()> {
        use codex_features::RuntimeAuthMode;
        use codex_runtime_agent_instance::AgentHostAuthorization;
        use codex_runtime_agent_instance::RuntimeAuthManager;
        use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
        use codex_runtime_agent_instance::RuntimeRole;
        use serde_json::json;
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/agent/register"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "runtime_agent_instance_id": "runtime-other-account", "agent_host_id": "host",
                "actor": {"kind": "chatgpt", "user_id": "actor", "account_id": "another-account"},
            })))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let credentials =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let runtime = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: None,
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            format!("{}/agent/register", server.uri()).parse()?,
        ));
        let slot = RuntimeAuthSlot::default();
        assert!(
            slot.set(runtime).is_ok(),
            "the runtime slot should be empty"
        );
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let (owner, _) = RemoteControlAuth::capture(
                Arc::clone(&credentials),
                Some((Arc::clone(&slot), mode)),
            );
            let auth = load_remote_control_auth(&owner).await?;
            let Err(error) = auth
                .request_context(
                    "https://chatgpt.com/backend-api/wham/remote/control/server/enroll",
                )
                .await
            else {
                panic!("a different registered account must not authorize enrollment");
            };
            assert!(
                error
                    .to_string()
                    .contains("chatgpt-account-id does not match")
            );
        }
        server.verify().await;
        Ok(())
    }

    #[tokio::test]
    async fn request_headers_adds_account_header_when_provider_omits_it() {
        let headers = headers_for("selected-account", Vec::new())
            .await
            .expect("request headers should build");

        assert_eq!(
            headers
                .get_all(REMOTE_CONTROL_ACCOUNT_ID_HEADER)
                .iter()
                .map(|value| value.to_str().expect("account header should be text"))
                .collect::<Vec<_>>(),
            vec!["selected-account"]
        );
    }

    #[tokio::test]
    async fn request_headers_replaces_provider_accounts_and_preserves_other_headers() {
        let headers = headers_for(
            "selected-account",
            vec!["provider-account-a", "provider-account-b"],
        )
        .await
        .expect("request headers should build");

        assert_eq!(
            headers
                .get_all(REMOTE_CONTROL_ACCOUNT_ID_HEADER)
                .iter()
                .map(|value| value.to_str().expect("account header should be text"))
                .collect::<Vec<_>>(),
            vec!["selected-account"]
        );
        assert_eq!(
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer test-token")
        );
        assert_eq!(
            headers
                .get("x-openai-fedramp")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
    }

    #[tokio::test]
    async fn request_headers_rejects_invalid_account_header_value() {
        let err = headers_for("invalid\naccount", Vec::new())
            .await
            .expect_err("invalid account header should fail");

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string()
                .starts_with("invalid remote control account id header:")
        );
    }
}
