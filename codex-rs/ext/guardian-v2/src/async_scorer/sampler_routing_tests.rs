//! Keeps classifier evidence within its owning account and thread routing binding.

use super::connect_sampler;
use super::proxy_websocket_servers;
use super::sample_request;
use super::sampler_config;
use anyhow::Result;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_login::WorkspaceRouting;
use codex_login::WorkspaceRoutingRequest;
use codex_login::WorkspaceRoutingResolver;
use codex_model_provider::ModelProviderFuture;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

type RoutingFuture<'a> = ModelProviderFuture<'a, std::io::Result<Option<WorkspaceRouting>>>;

struct UnavailableRouting;

impl WorkspaceRoutingResolver for UnavailableRouting {
    fn resolve(&self, _request: WorkspaceRoutingRequest) -> RoutingFuture<'_> {
        Box::pin(async { Err(std::io::Error::other("discovery unavailable")) })
    }
}

#[tokio::test]
async fn failed_routing_blocks_a_prewarmed_classifier_connection() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let response = vec![vec![ev_assistant_message("r1", "low"), ev_completed("r1")]];
    let server = responses::start_websocket_server(vec![response.clone(), response]).await;
    let base_url = proxy_websocket_servers(&[&server, &server])
        .await?
        .replace("/v1", "/backend-api/codex");
    let auth =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let mut config = sampler_config(base_url.clone());
    config.provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(Some(base_url)),
        Some(auth.clone()),
    );
    let sampler = connect_sampler(config).await?;
    let resolver: Arc<dyn WorkspaceRoutingResolver> = Arc::new(UnavailableRouting);
    auth.set_workspace_routing_resolver(Arc::downgrade(&resolver));
    let error = sampler.sample(sample_request("turn-1")).await.unwrap_err();
    assert!(
        error.to_string().contains("discovery unavailable"),
        "{error}"
    );
    assert!(server.connections().iter().all(Vec::is_empty));
    Ok(())
}

struct AuthAfterRouting {
    initial: CodexAuth,
    updated: CodexAuth,
    resolved: AtomicBool,
}

impl ExternalAuth for AuthAfterRouting {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            Ok(if self.resolved.load(Ordering::SeqCst) {
                self.updated.clone()
            } else {
                self.initial.clone()
            })
        })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        ExternalAuth::resolve(self)
    }
}

impl WorkspaceRoutingResolver for AuthAfterRouting {
    fn resolve(&self, _request: WorkspaceRoutingRequest) -> RoutingFuture<'_> {
        Box::pin(async {
            self.resolved.store(/*val*/ true, Ordering::SeqCst);
            Ok(None)
        })
    }
}

#[tokio::test]
async fn classification_retries_refreshes_but_never_switches_accounts() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    // Both tokens identify the same user; their signatures differ after refresh.
    let token =
        "header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoidXNlci1hIn19";
    for account in ["workspace-a", "workspace-b"] {
        let server = responses::start_mock_server().await;
        let response = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                ev_assistant_message("score", "low"),
                ev_completed("score"),
            ]),
        )
        .await;
        let home = tempfile::tempdir()?;
        let changes = Arc::new(AuthAfterRouting {
            initial: CodexAuth::from_external_chatgpt_tokens(
                &format!("{token}.initial"),
                "workspace-a",
                /*chatgpt_plan_type*/ None,
            )?,
            updated: CodexAuth::from_external_chatgpt_tokens(
                &format!("{token}.refreshed"),
                account,
                /*chatgpt_plan_type*/ None,
            )?,
            resolved: AtomicBool::new(/*v*/ false),
        });
        let auth = AuthManager::from_auth_for_testing_with_home(
            changes.initial.clone(),
            home.path().to_owned(),
        );
        auth.set_external_auth(changes.clone()).await?;
        let resolver: Arc<dyn WorkspaceRoutingResolver> = changes;
        auth.set_workspace_routing_resolver(Arc::downgrade(&resolver));
        let mut config = sampler_config(format!("{}/backend-api/codex", server.uri()));
        config.provider = create_model_provider(config.provider.info().clone(), Some(auth));
        let sampler = super::LunaSampler::new(config);
        let result = sampler.sample(sample_request("turn-1")).await;
        if account == "workspace-a" {
            assert_eq!(result?, "low");
            let request = response.single_request();
            assert_eq!(
                (
                    request.header("chatgpt-account-id"),
                    request.header("authorization")
                ),
                (
                    Some("workspace-a".into()),
                    Some(format!("Bearer {token}.refreshed"))
                ),
            );
        } else {
            assert!(result.unwrap_err().to_string().contains("account changed"));
            assert!(
                response.requests().is_empty(),
                "evidence must not reach the new account"
            );
        }
    }
    Ok(())
}

struct ChangedBootstrap(AtomicBool);

impl WorkspaceRoutingResolver for ChangedBootstrap {
    fn resolve(&self, request: WorkspaceRoutingRequest) -> RoutingFuture<'_> {
        Box::pin(async move {
            if request.previously_routed {
                Err(std::io::Error::other(
                    "bootstrap changed; start a new thread",
                ))
            } else if self.0.swap(/*val*/ false, Ordering::SeqCst) {
                Ok(Some(WorkspaceRouting {
                    chatgpt_account_id: "account_id".into(),
                    backend_origin: "https://gov.chatgpt.com".into(),
                    account_routing_override: "us_cr".into(),
                }))
            } else {
                Ok(None)
            }
        })
    }
}

#[tokio::test]
async fn idle_classifier_retains_the_owning_threads_workspace_binding() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let test = core_test_support::test_codex::test_codex()
        .build_with_auto_env(&server)
        .await?;
    let auth =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let resolver: Arc<dyn WorkspaceRoutingResolver> =
        Arc::new(ChangedBootstrap(AtomicBool::new(/*v*/ true)));
    auth.set_workspace_routing_resolver(Arc::downgrade(&resolver));
    let mut config = test.config.clone();
    config.model_provider = ModelProviderInfo::create_openai_provider(Some(format!(
        "{}/backend-api/codex",
        server.uri()
    )));
    let session_store = codex_extension_api::ExtensionData::new("session-1");
    let input = codex_extension_api::ThreadStartInput {
        config: &config,
        session_source: &codex_protocol::protocol::SessionSource::Exec,
        persistent_thread_state_available: false,
        environments: &[],
        mcp_resource_client: None,
        extension_metrics: None,
        session_store: &session_store,
        thread_store: test.codex.thread_extension_data(),
    };
    let owning_context = input
        .thread_store
        .get::<codex_model_provider::WorkspaceRoutingContext>()
        .expect("Core seeds the owning thread's routing context before extensions start");
    let classifier =
        crate::async_scorer::startup::sampler_config(&input, auth.clone(), /*manager*/ None).await;
    classifier
        .provider
        .responses_api_provider(&owning_context)
        .await?;
    let sampler = super::LunaSampler::new(classifier);
    let error = sampler.sample(sample_request("turn-1")).await.unwrap_err();
    assert!(error.to_string().contains("start a new thread"), "{error}");
    assert!(server.received_requests().await.unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn account_switch_cancels_request_opening_and_first_token_waits() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let token = "header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoidXNlci1hIn19.signature";
    for transport in ["http", "websocket"] {
        let http = responses::start_mock_server().await;
        let opening = responses::mount_response_once(
            &http,
            responses::sse_response(String::new()).set_delay(Duration::from_secs(/*secs*/ 60)),
        )
        .await;
        let stalled = WebSocketConnectionConfig {
            requests: vec![Vec::new()],
            response_headers: Vec::new(),
            accept_delay: None,
            close_after_requests: false,
        };
        let websocket = responses::start_websocket_server_with_headers(vec![stalled; 2]).await;
        let changes = Arc::new(AuthAfterRouting {
            initial: CodexAuth::from_external_chatgpt_tokens(
                token,
                "workspace-a",
                /*chatgpt_plan_type*/ None,
            )?,
            updated: CodexAuth::from_external_chatgpt_tokens(
                token,
                "workspace-b",
                /*chatgpt_plan_type*/ None,
            )?,
            resolved: AtomicBool::new(/*v*/ false),
        });
        let home = tempfile::tempdir()?;
        let auth = AuthManager::from_auth_for_testing_with_home(
            changes.initial.clone(),
            home.path().to_owned(),
        );
        auth.set_external_auth(changes.clone()).await?;
        let base_url = if transport == "http" {
            format!("{}/v1", http.uri())
        } else {
            proxy_websocket_servers(&[&websocket, &websocket]).await?
        };
        let mut config = sampler_config(base_url);
        config.provider = create_model_provider(config.provider.info().clone(), Some(auth.clone()));
        let sampler = if transport == "http" {
            super::LunaSampler::new(config)
        } else {
            connect_sampler(config).await?
        };
        let sample = sampler.sample(sample_request("turn-1"));
        tokio::pin!(sample);
        tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
            loop {
                if !opening.requests().is_empty() || websocket.connections().iter().any(|requests| !requests.is_empty()) {
                    break Ok::<(), anyhow::Error>(());
                }
                tokio::select! {
                    result = &mut sample => anyhow::bail!("classification finished before account switch: {result:?}"),
                    () = tokio::task::yield_now() => {}
                }
            }
        }).await??;
        changes.resolved.store(/*val*/ true, Ordering::SeqCst);
        auth.set_external_auth(changes).await?;
        let error = tokio::time::timeout(Duration::from_secs(/*secs*/ 2), &mut sample)
            .await?
            .unwrap_err();
        assert!(
            error.to_string().contains("account changed"),
            "{transport}: {error}"
        );
        assert_eq!(opening.requests().len(), usize::from(transport == "http"));
        assert_eq!(
            websocket.connections().iter().map(Vec::len).sum::<usize>(),
            usize::from(transport == "websocket")
        );
    }
    Ok(())
}
