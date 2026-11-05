//! Regression coverage for remote mutations and account-bound storage transfers.

use super::*;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::test_support::load_plugins_config;
use crate::test_support::test_auth_manager;
use crate::test_support::test_plugins_manager_with_auth_manager;
use crate::test_support::write_file;
use codex_protocol::auth::AuthMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tempfile::TempDir;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn uninstall_serializes_backend_mutations_and_preserves_cache_on_failure() {
    for uninstall_fails in [false, true] {
        let home = TempDir::new().unwrap();
        let server = MockServer::start().await;
        let mut config = load_plugins_config(home.path(), home.path()).await;
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        let auth_manager = test_auth_manager(Some(AuthMode::Chatgpt));
        let auth = auth_manager.auth().await;
        let manager = Arc::new(test_plugins_manager_with_auth_manager(
            home.path().to_path_buf(),
            /*restriction_product*/ None,
            auth_manager,
        ));
        let remote_id = "b1234567-89ab-4cde-8f01-234567890abc";
        let plugin_id = PluginId::new(
            "sample".to_string(),
            REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
        )
        .unwrap();
        let cache = manager.store.plugin_base_root(&plugin_id);
        write_file(
            cache.join("1.0.0/.codex-plugin/plugin.json").as_path(),
            r#"{"name":"sample","version":"1.0.0"}"#,
        );
        Mock::given(method("GET"))
            .and(path(format!("/backend-api/ps/plugins/{remote_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": remote_id, "name": "sample", "scope": "GLOBAL",
                "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE",
                "release": {"display_name": "Sample", "description": "Sample plugin", "interface": {}}
            }))).mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"plugins": [], "pagination": {"next_page_token": null}})),
            )
            .expect(if uninstall_fails { 0 } else { 1 })
            .mount(&server)
            .await;
        let gate_held = Arc::new(AtomicBool::new(false));
        let gate_held_at_post = Arc::clone(&gate_held);
        let manager_at_post = Arc::clone(&manager);
        Mock::given(method("POST"))
            .and(path(format!(
                "/backend-api/ps/plugins/{remote_id}/uninstall"
            )))
            .respond_with(move |_: &wiremock::Request| {
                gate_held_at_post.store(
                    manager_at_post
                        .remote_installed_plugin_bundle_sync_gate
                        .available_permits()
                        == 0,
                    Ordering::SeqCst,
                );
                ResponseTemplate::new(if uninstall_fails { 400 } else { 200 })
                    .set_body_json(json!({"id": remote_id, "enabled": false}))
            })
            .expect(1)
            .mount(&server)
            .await;
        let refreshed = Arc::new(Notify::new());
        let refreshed_callback = Arc::clone(&refreshed);
        let outcome = manager
            .uninstall_remote_plugin(
                &config,
                auth.as_ref(),
                remote_id,
                Some(Arc::new(move |_| refreshed_callback.notify_one())),
            )
            .await;
        assert_eq!(
            (
                gate_held.load(Ordering::SeqCst),
                manager
                    .remote_installed_plugin_bundle_sync_gate
                    .available_permits(),
                cache.as_path().exists(),
                outcome.is_err()
            ),
            (true, 1, uninstall_fails, uninstall_fails),
        );
        if !uninstall_fails {
            tokio::time::timeout(std::time::Duration::from_secs(5), refreshed.notified())
                .await
                .expect("successful uninstall should refresh installed state");
        }
    }
}

/// Switch only after metadata has been read, immediately before constructing the storage request.
/// Metadata-body cancellation would not exercise the separate request these tests protect.
#[derive(Clone)]
struct TransferHttpClients {
    delegate: codex_http_client::RouteAwareClientPool,
    controller: codex_http_client::NetworkPolicyController,
    switch_before_storage: Arc<AtomicBool>,
    api_origin: String,
    storage_origin: String,
}

impl std::fmt::Debug for TransferHttpClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferHttpClients")
            .finish_non_exhaustive()
    }
}

impl crate::http_client_selector::HttpClientSelector for TransferHttpClients {
    fn for_current_account(&self) -> Arc<dyn crate::http_client_selector::HttpClientSelector> {
        Arc::new(Self {
            delegate: self.delegate.for_current_account(),
            ..self.clone()
        })
    }

    fn request(&self, method: http::Method, url: &str) -> codex_http_client::RequestBuilder {
        if url.starts_with("https://storage.example/")
            && self.switch_before_storage.swap(false, Ordering::SeqCst)
        {
            let policy = self.controller.policy();
            policy.invalidate();
            assert!(self.controller.publish(
                policy.revision(),
                codex_http_client::DestinationPolicy::Unrestricted,
            ));
        }
        self.delegate.request(
            method,
            url.replace("https://storage.example", &self.storage_origin)
                .replace("https://chatgpt.com", &self.api_origin),
        )
    }

    fn request_for_account(
        &self,
        method: http::Method,
        url: &str,
        binding: &codex_http_client::NetworkPolicy,
    ) -> Option<codex_http_client::RequestBuilder> {
        Some(
            self.delegate
                .with_account_binding(binding)?
                .request(method, url.replace("https://chatgpt.com", &self.api_origin)),
        )
    }

    fn outbound_proxy_policy(&self) -> codex_http_client::OutboundProxyPolicy {
        self.delegate.outbound_proxy_policy()
    }
}

struct PluginTransferTest {
    api: MockServer,
    storage: MockServer,
    config: PluginsConfigInput,
    manager: Arc<PluginsManager>,
    auth: CodexAuth,
    switch_before_storage: Arc<AtomicBool>,
}

impl PluginTransferTest {
    async fn new(
        home: &std::path::Path,
        mode: codex_features::RuntimeAuthMode,
        switch: bool,
    ) -> Self {
        use base64::Engine;
        use codex_http_client::ClientRouteClass;
        use codex_http_client::DestinationPolicy;
        use codex_http_client::HttpClientFactory;
        use codex_http_client::NetworkPolicyController;
        use codex_http_client::OutboundProxyPolicy;
        use codex_http_client::RouteAwareClientPool;
        use codex_runtime_agent_instance::AgentHostAuthorization;
        use codex_runtime_agent_instance::RuntimeAuthManager;
        use codex_runtime_agent_instance::RuntimeAuthSlot;
        use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
        use codex_runtime_agent_instance::RuntimeRole;

        let api = MockServer::start().await;
        let storage = MockServer::start().await;
        Mock::given(path("/agent/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": "runtime-plugins", "agent_host_id": "host-a",
                "actor": {"kind": "chatgpt", "user_id": "user-a", "account_id": "account-a"},
            })))
            .expect(1)
            .mount(&api)
            .await;
        let claims = json!({
            "iss": "https://auth.openai.com/", "session_id": "login-a",
            "https://api.openai.com/auth": {"user_id": "user-a", "chatgpt_account_id": "account-a"},
        });
        let token = format!(
            "header.{}.signature",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string()),
        );
        let auth = CodexAuth::from_external_chatgpt_tokens(
            &token,
            "account-a",
            /*chatgpt_plan_type*/ None,
        )
        .unwrap();
        let credentials = codex_login::AuthManager::from_auth_for_testing(auth.clone());
        let slot = RuntimeAuthSlot::default();
        slot.set(Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            format!("{}/agent/register", api.uri()).parse().unwrap(),
        )))
        .unwrap();
        let controller = NetworkPolicyController::default();
        assert!(controller.publish(
            controller.policy().revision(),
            DestinationPolicy::Unrestricted
        ));
        let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(controller.policy())
            .with_sensitive_header_protection();
        let switch_before_storage = Arc::new(AtomicBool::new(switch));
        let clients = Arc::new(TransferHttpClients {
            delegate: RouteAwareClientPool::with_chatgpt_cloudflare_cookies_without_request_logging(
                factory.clone(),
                ClientRouteClass::Api,
            ),
            controller,
            switch_before_storage: Arc::clone(&switch_before_storage),
            api_origin: api.uri(),
            storage_origin: storage.uri(),
        });
        let config = PluginsConfigInput::new(
            Default::default(),
            Default::default(),
            "openai".into(),
            /*plugins_enabled*/ true,
            /*remote_plugin_enabled*/ true,
            "https://chatgpt.com/backend-api".into(),
            factory,
            /*product_sku*/ None,
        )
        .with_runtime_auth_mode(Some(mode));
        config.runtime_http_clients.set(clients).unwrap();
        let manager = Arc::new(crate::PluginsManager::new_with_options(
            home.to_path_buf(),
            /*restriction_product*/ None,
            credentials,
            slot,
            crate::test_support::test_skill_root_loader(),
        ));
        Self {
            api,
            storage,
            config,
            manager,
            auth,
            switch_before_storage,
        }
    }

    async fn verify(&self, account_changed: bool, mode: codex_features::RuntimeAuthMode) {
        assert!(!self.switch_before_storage.load(Ordering::SeqCst));
        self.api.verify().await;
        let storage_requests = self.storage.received_requests().await.unwrap();
        assert_eq!(storage_requests.len(), usize::from(!account_changed));
        for request in storage_requests {
            for name in [
                "authorization",
                "x-openai-agent-assertion",
                "chatgpt-account-id",
            ] {
                assert!(!request.headers.contains_key(name));
            }
        }
        for request in self.api.received_requests().await.unwrap() {
            if request.url.path() == "/agent/register" {
                continue;
            }
            let authorization = request.headers["authorization"].to_str().unwrap();
            match mode {
                codex_features::RuntimeAuthMode::DualHeader => {
                    assert!(authorization.starts_with("Bearer "));
                    assert!(request.headers.contains_key("x-openai-agent-assertion"));
                }
                codex_features::RuntimeAuthMode::AssertionOnly => {
                    assert!(authorization.starts_with("AgentAssertion "));
                    assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                }
            }
        }
    }
}

#[tokio::test]
async fn plugin_share_keeps_account_binding_through_storage_upload() {
    use codex_features::RuntimeAuthMode;
    use codex_http_client::NetworkPolicyDenied;
    use codex_http_client::RouteAwareRequestError;

    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        for account_changed in [false, true] {
            let home = TempDir::new().unwrap();
            let fixture = PluginTransferTest::new(home.path(), mode, account_changed).await;
            let plugin_path = home.path().join("sample");
            write_file(
                &plugin_path.join(".codex-plugin/plugin.json"),
                r#"{"name":"sample"}"#,
            );
            Mock::given(method("POST"))
                .and(path("/backend-api/public/plugins/workspace/upload-url"))
                .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                    "file_id": "file_123", "upload_url": "https://storage.example/upload",
                    "etag": "\"metadata-etag\"",
                })))
                .expect(1)
                .mount(&fixture.api)
                .await;
            Mock::given(method("PUT"))
                .and(path("/upload"))
                .respond_with(ResponseTemplate::new(201))
                .mount(&fixture.storage)
                .await;
            Mock::given(method("POST"))
                .and(path("/backend-api/public/plugins/workspace"))
                .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                    "plugin_id": "plugins_123", "share_url": null,
                })))
                .expect(u64::from(!account_changed))
                .mount(&fixture.api)
                .await;
            let result = remote::save_remote_plugin_share(
                &fixture
                    .manager
                    .remote_plugin_service_config(&fixture.config),
                Some(&fixture.auth),
                home.path(),
                &AbsolutePathBuf::try_from(plugin_path).unwrap(),
                /*remote_plugin_id*/ None,
                remote::RemotePluginShareAccessPolicy::default(),
            )
            .await;
            if account_changed {
                assert!(
                    matches!(
                        result,
                        Err(RemotePluginCatalogError::Request {
                            source: RouteAwareRequestError::Policy(NetworkPolicyDenied::Revoked),
                            ..
                        })
                    ),
                    "{result:?}"
                );
            } else {
                assert_eq!(result.unwrap().remote_plugin_id, "plugins_123");
            }
            fixture.verify(account_changed, mode).await;
        }
    }
}

#[tokio::test]
async fn plugin_install_keeps_account_binding_through_storage_download() {
    use codex_features::RuntimeAuthMode;
    use codex_http_client::NetworkPolicyDenied;
    use codex_http_client::RouteAwareRequestError;

    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        for account_changed in [false, true] {
            let home = TempDir::new().unwrap();
            let fixture = PluginTransferTest::new(home.path(), mode, account_changed).await;
            let remote_id = "b1234567-89ab-4cde-8f01-234567890abc";
            Mock::given(method("GET"))
                .and(path(format!("/backend-api/ps/plugins/{remote_id}")))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": remote_id, "name": "sample", "scope": "GLOBAL",
                    "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE",
                    "release": {
                        "version": "1.0.0", "display_name": "Sample",
                        "description": "Sample plugin", "interface": {},
                        "bundle_download_url": "https://storage.example/bundle",
                    }
                })))
                .expect(1)
                .mount(&fixture.api)
                .await;
            Mock::given(method("GET"))
                .and(path("/backend-api/ps/plugins/installed"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "plugins": [], "pagination": {"next_page_token": null},
                })))
                .expect(1)
                .mount(&fixture.api)
                .await;
            Mock::given(method("GET"))
                .and(path("/bundle"))
                .respond_with(ResponseTemplate::new(200).set_body_string("bundle"))
                .mount(&fixture.storage)
                .await;
            let result = fixture
                .manager
                .install_remote_plugin(
                    &fixture.config,
                    Some(&fixture.auth),
                    RemotePluginInstallRequest {
                        marketplace_name: REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
                        remote_plugin_id: remote_id.to_string(),
                        install_attempt_id: None,
                    },
                    /*on_effective_plugins_changed*/ None,
                )
                .await;
            let error = result.err().expect("test transfer should fail");
            if account_changed {
                assert!(
                    matches!(
                        *error.kind,
                        RemotePluginOperationErrorKind::Bundle(
                            RemotePluginBundleInstallError::DownloadRequest {
                                source: RouteAwareRequestError::Policy(
                                    NetworkPolicyDenied::Revoked
                                ),
                                ..
                            }
                        )
                    ),
                    "{error:?}"
                );
            } else {
                // The local HTTP control reaches storage, then fails the final-URL HTTPS check.
                assert!(
                    matches!(
                        *error.kind,
                        RemotePluginOperationErrorKind::Bundle(
                            RemotePluginBundleInstallError::UnsupportedBundleDownloadFinalUrl { .. }
                        )
                    ),
                    "{error:?}"
                );
            }
            fixture.verify(account_changed, mode).await;
        }
    }
}
