//! Exercise actor-authenticated remote control operations and separate relay capabilities.

use super::auth::RemoteControlAuth;
use super::auth::load_remote_control_auth;
use super::clients::list_remote_control_clients;
use super::clients::revoke_remote_control_client;
use super::protocol::StartRemoteControlPairingRequest;
use super::protocol::normalize_remote_control_url;
use super::server_api::enroll_remote_control_server;
use super::server_api::refresh_remote_control_server;
use base64::Engine;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_features::RuntimeAuthMode;
use codex_http_client::DestinationPolicy;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::AuthRouteConfig;
use codex_login::default_client::ResidencyRequirement;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeAuthSlot;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actor_requests_share_runtime_and_pairing_keeps_its_server_token() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({
            "iss": "https://auth.openai.com/", "session_id": "remote-control-login",
            "https://api.openai.com/auth": {"user_id": "actor", "chatgpt_account_id": "account_id"}
        })
        .to_string(),
    );
    let token = format!("header.{claims}.signature");
    if let Ok(server) = std::env::var(PROBE_SERVER) {
        set_default_client_residency_requirement(Some(ResidencyRequirement::Us));
        let home = tempfile::tempdir()?;
        codex_login::auth::login_with_chatgpt_auth_tokens(
            home.path(),
            &token,
            "account_id",
            /*chatgpt_plan_type*/ None,
        )?;
        let controller = NetworkPolicyController::default();
        let policy = controller.policy();
        let credentials = Arc::new(
            AuthManager::new(
                home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::Ephemeral,
                /*forced_chatgpt_workspace_id*/ None,
                /*chatgpt_base_url*/ None,
                AuthKeyringBackendKind::Direct,
                AuthRouteConfig::from_http_client_factory(
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
                        .with_network_policy(policy.clone()),
                ),
            )
            .await,
        );
        let runtime = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            "https://auth.openai.com/agent/register".parse()?,
        ));
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let slot = RuntimeAuthSlot::default();
            let (owner, _) = RemoteControlAuth::capture(
                Arc::clone(&credentials),
                Some((Arc::clone(&slot), mode)),
            );
            let auth = load_remote_control_auth(&owner).await?;
            let Err(error) = enroll_remote_control_server(
                &normalize_remote_control_url("https://chatgpt.com/backend-api")?,
                &auth,
                "installation",
                "server",
            )
            .await
            else {
                panic!("enrollment requires the harness runtime");
            };
            assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
            assert!(
                slot.set(Arc::clone(&runtime)).is_ok(),
                "the harness should fill its empty runtime slot"
            );
            controller.unavailable(policy.revision());
            let auth = load_remote_control_auth(&owner).await?;
            let Err(error) = enroll_remote_control_server(
                &normalize_remote_control_url("https://chatgpt.com/backend-api")?,
                &auth,
                "installation",
                "server",
            )
            .await
            else {
                panic!("managed policy must block the request");
            };
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            assert!(super::auth::is_policy_denial(&error));
            assert!(!super::auth::is_auth_error(&error));
            assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
            for base in [
                "https://chatgpt.com/backend-api".to_owned(),
                format!("{server}/custom/backend-api"),
            ] {
                let auth = load_remote_control_auth(&owner).await?;
                let mut enrollment = enroll_remote_control_server(
                    &normalize_remote_control_url(&base)?,
                    &auth,
                    "installation",
                    "server",
                )
                .await?;
                enrollment.remote_control_token = None;
                refresh_remote_control_server(&auth, "installation", &mut enrollment).await?;
                let clients = list_remote_control_clients(
                    &base,
                    &owner,
                    RemoteControlClientsListParams {
                        environment_id: "env".into(),
                        cursor: None,
                        limit: None,
                        order: None,
                    },
                )
                .await?;
                assert!(clients.data.is_empty());
                revoke_remote_control_client(
                    &base,
                    &owner,
                    RemoteControlClientsRevokeParams {
                        environment_id: "env".into(),
                        client_id: "client".into(),
                    },
                )
                .await?;
                let pairing = enrollment
                    .start_pairing(
                        &auth.http_client_factory,
                        StartRemoteControlPairingRequest { manual_code: false },
                    )
                    .await?;
                assert_eq!(pairing.pairing_code, "pairing");
            }
        }
        return Ok(());
    }
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-remote-control", "agent_host_id": "host",
            "actor": {"kind": "chatgpt", "user_id": "actor", "account_id": "account_id"}
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    for prefix in ["/backend-api", "/custom/backend-api"] {
        for suffix in ["server/enroll", "server/refresh"] {
            Mock::given(method("POST")).and(path(format!("{prefix}/wham/remote/control/{suffix}")))
                .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "server_id": "server", "environment_id": "env", "remote_control_token": "relay-capability", "expires_at": "2999-01-01T00:00:00Z"
                }))).expect(/*r*/ 2).mount(&server).await;
        }
        Mock::given(method("GET"))
            .and(path(format!(
                "{prefix}/wham/remote/control/environments/env/clients"
            )))
            .respond_with(ResponseTemplate::new(/*s*/ 307).insert_header(
                "location",
                format!("{prefix}/wham/remote/control/environments/env/redirected-clients"),
            ))
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "{prefix}/wham/remote/control/environments/env/redirected-clients"
            )))
            .respond_with(
                ResponseTemplate::new(/*s*/ 200)
                    .set_body_json(json!({"items": [], "cursor": null})),
            )
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!(
                "{prefix}/wham/remote/control/environments/env/clients/client"
            )))
            .respond_with(ResponseTemplate::new(/*s*/ 307).insert_header(
                "location",
                format!("https://api.openai.org{prefix}/wham/remote/control/environments/env/client-redirected"),
            ))
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!(
                "{prefix}/wham/remote/control/environments/env/client-redirected"
            )))
            .respond_with(ResponseTemplate::new(/*s*/ 204))
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path(format!("{prefix}/wham/remote/control/server/pair")))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "server_id": "server", "environment_id": "env", "pairing_code": "pairing", "manual_pairing_code": null, "expires_at": "2999-01-01T00:00:00Z"
            }))).expect(/*r*/ 2).mount(&server).await;
    }
    run_harness_through_proxy(&server, "transport::remote_control::runtime_auth_tests::actor_requests_share_runtime_and_pairing_keeps_its_server_token", /*realtime_target*/ None).await?;
    let requests = server.received_requests().await.unwrap();
    let mut proofs = HashSet::new();
    let mut dual = 0;
    for request in requests.iter().filter(|r| {
        r.url.path().contains("/wham/remote/control/")
            && !r.url.path().ends_with("/redirected-clients")
            && !r.url.path().ends_with("/client-redirected")
    }) {
        let authorization = request.headers["authorization"].to_str()?;
        if !request.url.path().ends_with("/pair") {
            assert_eq!(request.headers["x-openai-internal-codex-residency"], "us");
        }
        if request.url.path().ends_with("/pair") {
            assert_eq!(authorization, "Bearer relay-capability");
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        } else if request.url.path().starts_with("/custom/") {
            assert_eq!(authorization, format!("Bearer {token}"));
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        } else {
            assert_eq!(request.headers["chatgpt-account-id"], "account_id");
            let proof = if let Some(proof) = request.headers.get("x-openai-agent-assertion") {
                dual += 1;
                assert_eq!(authorization, format!("Bearer {token}"));
                proof.to_str()?
            } else {
                authorization.strip_prefix("AgentAssertion ").unwrap()
            };
            let payload: serde_json::Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(proof.split('.').nth(/*n*/ 2).unwrap())?,
            )?;
            assert_eq!(payload["runtime_agent_instance"], "runtime-remote-control");
            assert!(proofs.insert(proof.to_owned()));
        }
    }
    assert_eq!((dual, proofs.len()), (4, 8));
    for prefix in ["/backend-api", "/custom/backend-api"] {
        let original = format!("{prefix}/wham/remote/control/environments/env/clients");
        let redirected =
            format!("{prefix}/wham/remote/control/environments/env/redirected-clients");
        let originals: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == original)
            .collect();
        let redirects: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == redirected)
            .collect();
        assert_eq!(originals.len(), 2);
        assert_eq!(redirects.len(), 2);
        for (original, redirected) in originals.into_iter().zip(redirects) {
            assert_eq!(
                redirected.headers.get("authorization"),
                original.headers.get("authorization")
            );
            assert_eq!(
                redirected.headers.get("x-openai-agent-assertion"),
                original.headers.get("x-openai-agent-assertion")
            );
        }
    }
    let cross_origin: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().ends_with("/client-redirected"))
        .collect();
    assert_eq!(cross_origin.len(), 4);
    for request in cross_origin {
        assert!(!request.headers.contains_key("authorization"));
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    server.verify().await;
    Ok(())
}
