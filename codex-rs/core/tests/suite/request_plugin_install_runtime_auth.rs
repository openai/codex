//! Exercise recommended-plugin metadata after resuming with another account.

use super::*;
use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_protocol::mcp::ClientMcpExtensions;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy;
use pretty_assertions::assert_eq;
use test_case::test_case;

fn actor_token(actor: &str) -> String {
    let claims = json!({
        "exp": 4_102_444_800_u64,
        "session_id": format!("session-{actor}"),
        "iss": "https://auth.openai.com",
        "https://api.openai.com/auth": {
            "chatgpt_user_id": format!("user-{actor}"),
            "chatgpt_account_id": format!("account-{actor}"),
        },
    });
    format!(
        "e30.{}.c2ln",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string())
    )
}

#[test_case(RuntimeAuthMode::DualHeader; "dual_header")]
#[test_case(RuntimeAuthMode::AssertionOnly; "assertion_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_plugin_install_uses_the_threads_account(mode: RuntimeAuthMode) -> Result<()> {
    skip_if_no_network!(Ok(()));
    const PLUGIN_ID: &str = "plugin_connector_github";
    if std::env::var(PROBE_SERVER).is_ok() {
        let server = start_mock_server().await;
        let requests = mount_sse_sequence(
            &server,
            vec![
                sse(vec![ev_completed("original")]),
                sse(vec![
                    ev_function_call(
                        "install-github",
                        REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                        &json!({
                            "plugin_id": "github@openai-curated-remote",
                            "suggest_reason": "Use GitHub for this request",
                        })
                        .to_string(),
                    ),
                    ev_completed("install"),
                ]),
                sse(vec![ev_completed("finished")]),
            ],
        )
        .await;
        let mut builder = test_codex()
            .with_auth(CodexAuth::from_external_chatgpt_tokens(
                &actor_token("a"),
                "account-a",
                Some("plus"),
            )?)
            .with_config(move |config| {
                config
                    .permissions
                    .set_permission_profile(PermissionProfile::Disabled)
                    .expect("test config should allow disabled permissions");
                configure_apps_without_search_tool(config, "https://chatgpt.com");
                config
                    .features
                    .enable(Feature::RuntimeAgentInstances)
                    .unwrap();
                config.runtime_auth_mode = mode;
            });
        let test = builder.build_with_auto_env(&server).await?;
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Original account".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        test.codex.ensure_rollout_materialized().await;
        test.codex.flush_rollout().await?;
        let rollout = test.codex.rollout_path().context("original rollout")?;
        test.codex.shutdown_and_wait().await?;
        let _ = test
            .thread_manager
            .remove_thread(&test.session_configured.thread_id)
            .await;
        let history = test
            .thread_store
            .load_latest_model_context(codex_thread_store::LoadThreadHistoryParams {
                thread_id: test.session_configured.thread_id,
                include_archived: false,
            })
            .await?;
        let replacement = codex_core::test_support::auth_manager_from_auth_with_home(
            CodexAuth::from_external_chatgpt_tokens(&actor_token("b"), "account-b", Some("plus"))?,
            test.codex_home_path().to_path_buf(),
        );
        let resumed = test
            .thread_manager
            .resume_thread_with_history(
                test.config.clone(),
                InitialHistory::Resumed(ResumedHistory {
                    history_revision: None,
                    conversation_id: history.thread_id,
                    history: Arc::new(history.items),
                    rollout_path: Some(rollout),
                }),
                replacement,
                /*parent_trace*/ None,
                ClientMcpExtensions::default(),
            )
            .await?;
        resumed
            .thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Use GitHub".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        let elicitation = wait_for_event_match(&resumed.thread, |event| match event {
            EventMsg::ElicitationRequest(request) => Some(Ok(request.clone())),
            EventMsg::TurnComplete(_) => Some(Err(anyhow::anyhow!(
                "the resumed account must reach the plugin installation prompt"
            ))),
            _ => None,
        })
        .await?;
        let ElicitationRequest::Form {
            meta: Some(meta), ..
        } = &elicitation.request
        else {
            anyhow::bail!("expected plugin installation metadata");
        };
        assert_eq!(meta["remote_plugin_id"], PLUGIN_ID);
        resumed
            .thread
            .submit(Op::ResolveElicitation {
                server_name: elicitation.server_name,
                request_id: elicitation.id,
                decision: ElicitationAction::Decline,
                content: None,
                meta: None,
            })
            .await?;
        wait_for_event(&resumed.thread, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        assert_eq!(requests.requests().len(), 3);
        resumed.thread.shutdown_and_wait().await?;
        return Ok(());
    }

    let server = start_mock_server().await;
    let _apps = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{"id": PLUGIN_ID, "name": "github", "display_name": "GitHub"}],
        })),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = request.body_json().unwrap();
            let actor = if body["actor_auth"]["credential"] == actor_token("a") {
                "a"
            } else {
                "b"
            };
            assert_eq!(body["actor_auth"]["credential"], actor_token(actor));
            ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": format!("runtime-{actor}"),
                "agent_host_id": "host-a",
                "actor": {
                    "kind": "chatgpt",
                    "user_id": format!("user-{actor}"),
                    "account_id": format!("account-{actor}"),
                },
            }))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/ps/plugins/{PLUGIN_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": PLUGIN_ID, "name": "github", "scope": "GLOBAL", "status": "ENABLED",
            "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE",
            "release": {
                "display_name": "GitHub", "description": "Work with GitHub repositories.",
                "app_ids": [], "interface": {},
            },
        })))
        .expect(1)
        .mount(&server)
        .await;
    let case = match mode {
        RuntimeAuthMode::DualHeader => "dual_header",
        RuntimeAuthMode::AssertionOnly => "assertion_only",
    };
    run_harness_through_proxy(
        &server,
        &format!("suite::request_plugin_install::runtime_auth_tests::resumed_plugin_install_uses_the_threads_account::{case}"),
        /*realtime_target*/ None,
    ).await?;
    let requests = server
        .received_requests()
        .await
        .context("recorded requests")?;
    let metadata = requests
        .iter()
        .find(|request| request.url.path() == format!("/ps/plugins/{PLUGIN_ID}"))
        .context("plugin metadata request")?;
    let proof = match mode {
        RuntimeAuthMode::DualHeader => {
            assert_eq!(
                metadata.headers["authorization"],
                format!("Bearer {}", actor_token("b"))
            );
            metadata.headers["x-openai-agent-assertion"].to_str()?
        }
        RuntimeAuthMode::AssertionOnly => {
            assert!(!metadata.headers.contains_key("x-openai-agent-assertion"));
            metadata.headers["authorization"]
                .to_str()?
                .strip_prefix("AgentAssertion ")
                .context("assertion auth")?
        }
    };
    let claims: Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(proof.split('.').nth(2).context("assertion payload")?)?,
    )?;
    assert_eq!(claims["runtime_agent_instance"], "runtime-b");
    server.verify().await;
    Ok(())
}
