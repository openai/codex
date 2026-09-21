use super::*;
use crate::mcp::tests::test_elicitation_config;
use async_channel::Receiver;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::GranularApprovalConfig;
use pretty_assertions::assert_eq;
use rmcp::model::ElicitRequestParams;
use rmcp::model::ElicitationSchema;
use rmcp::model::RequestMetaObject;
use serde_json::Map;
use serde_json::json;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

type ReviewerResponse = std::result::Result<Option<ElicitationResponse>, &'static str>;

struct RecordingReviewer {
    calls: AtomicUsize,
    active_elicitations: Arc<AtomicUsize>,
    response: ReviewerResponse,
}

impl RecordingReviewer {
    fn new(response: ReviewerResponse) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::default(),
            active_elicitations: Arc::default(),
            response,
        })
    }
}

impl ElicitationReviewer for RecordingReviewer {
    fn review(
        &self,
        request: ElicitationReviewRequest,
    ) -> BoxFuture<'static, Result<Option<ElicitationResponse>>> {
        assert_eq!(request.server_name, "independent-mcp");
        self.calls.fetch_add(/*val*/ 1, Relaxed);
        let active_elicitations = self.active_elicitations.clone();
        let active_during_review = usize::from(
            request
                .elicitation
                .meta()
                .is_some_and(|meta| meta.get(STRICT_AUTO_REVIEW_KEY) == Some(&Value::Bool(true))),
        );
        let response = self.response.clone();
        async move {
            assert_eq!(active_elicitations.load(Relaxed), active_during_review);
            tokio::task::yield_now().await;
            assert_eq!(active_elicitations.load(Relaxed), active_during_review);
            response.map_err(anyhow::Error::msg)
        }
        .boxed()
    }
}

struct LifecycleRegistration(Arc<AtomicUsize>);

impl Drop for LifecycleRegistration {
    fn drop(&mut self) {
        self.0.fetch_sub(/*val*/ 1, Relaxed);
    }
}

fn approved_response() -> ElicitationResponse {
    ElicitationResponse {
        action: ElicitationAction::Accept,
        content: Some(json!({})),
        meta: Some(json!({ "approvals_reviewer": "auto_review" })),
    }
}

fn elicitation_fixture(
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    reviewer: Option<Arc<RecordingReviewer>>,
) -> (ElicitationRequestManager, Receiver<Event>, SendElicitation) {
    let lifecycle = reviewer.as_ref().map(|reviewer| {
        let active_elicitations = reviewer.active_elicitations.clone();
        ElicitationLifecycle::new(move || {
            active_elicitations.fetch_add(/*val*/ 1, Relaxed);
            LifecycleRegistration(active_elicitations.clone())
        })
    });
    let mut config = test_elicitation_config(
        "independent-mcp",
        approval_policy,
        permission_profile.clone(),
    );
    Arc::make_mut(&mut config)
        .server_permission_profiles
        .insert("another-independent-mcp".to_string(), permission_profile);
    let manager = ElicitationRequestManager::new(
        config,
        reviewer.map(|reviewer| reviewer as Arc<dyn ElicitationReviewer>),
        lifecycle,
        ElicitationRequestRouter::default(),
    );
    let (tx_event, events) = async_channel::bounded(1);
    let sender = manager.make_sender(
        "independent-mcp".to_string(),
        Some(tx_event),
        &ClientMcpExtensions::default(),
    );
    (manager, events, sender)
}

async fn send_elicitation(sender: &SendElicitation, marker: Option<Value>) -> ElicitationResponse {
    let elicitation = Elicitation::Mcp(ElicitRequestParams::FormElicitationParams {
        meta: marker.map(|value| {
            RequestMetaObject::from(Map::from_iter([(STRICT_AUTO_REVIEW_KEY.into(), value)]))
        }),
        message: "Review this request".to_string(),
        requested_schema: ElicitationSchema::builder().build().unwrap(),
    });
    sender(RequestId::Number(7), elicitation)
        .await
        .expect("elicitation must receive a terminal response")
}

fn form_elicitations(meta: Option<Value>, properties: Value) -> [Elicitation; 3] {
    let schema = json!({"type": "object", "properties": properties});
    [
        Elicitation::Mcp(ElicitRequestParams::FormElicitationParams {
            meta: meta
                .as_ref()
                .map(|meta| RequestMetaObject::from(meta.as_object().unwrap().clone())),
            message: "Review this request".into(),
            requested_schema: serde_json::from_value(schema.clone()).unwrap(),
        }),
        Elicitation::OpenAiForm {
            meta: meta.clone(),
            message: "Review this request".into(),
            requested_schema: schema.clone(),
        },
        Elicitation::OpenAiElicitationForm {
            meta,
            message: "Review this request".into(),
            requested_schema: schema,
        },
    ]
}

#[tokio::test]
async fn permission_elicitation_preserves_automatic_review_decisions() {
    for (strict_auto_review, approval_policy, expected_reviews) in [
        (false, AskForApproval::OnRequest, 1),
        (true, AskForApproval::OnRequest, 1),
        (true, AskForApproval::Never, 1),
        (false, AskForApproval::Never, 0),
    ] {
        for response in [
            approved_response(),
            ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: Some(json!({"approvals_reviewer": "auto_review", "message": "Denied"})),
            },
        ] {
            let reviewer = RecordingReviewer::new(Ok(Some(response.clone())));
            let (manager, events, sender) = elicitation_fixture(
                approval_policy,
                PermissionProfile::Disabled,
                Some(reviewer.clone()),
            );
            let [request, _, _] = form_elicitations(
                Some(json!({
                    "codex_request_type": "approval_request",
                    "codex_approval_kind": "mcp_tool_call",
                    "codex_strict_auto_review": strict_auto_review,
                    "tool_name": "access_browser_origin",
                    "tool_params": {"origin": "https://example.com"},
                })),
                json!({}),
            );
            let expected = if expected_reviews == 0 {
                ElicitationResponse {
                    meta: None,
                    ..approved_response()
                }
            } else {
                response
            };
            assert_eq!(
                sender(RequestId::Number(7), request).await.unwrap(),
                expected
            );
            assert_eq!(reviewer.calls.load(Relaxed), expected_reviews);
            assert_eq!(reviewer.active_elicitations.load(Relaxed), 0);
            assert!(events.is_empty());
            assert!(manager.router.requests.lock().unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn elicitation_without_an_automatic_decision_prompts() {
    for reviewer in [None, Some(RecordingReviewer::new(Ok(None)))] {
        let (manager, events, sender) = elicitation_fixture(
            AskForApproval::OnRequest,
            PermissionProfile::read_only(),
            reviewer.clone(),
        );
        let mut requests = Vec::from(form_elicitations(/*meta*/ None, json!({})));
        requests.extend(form_elicitations(
            Some(json!({"codex_requires_user_input": true})),
            json!({"answer": {"type": "string"}}),
        ));
        requests.push(Elicitation::Mcp(
            ElicitRequestParams::UrlElicitationParams {
                meta: None,
                message: "Sign in to continue".into(),
                url: "https://example.com/auth".into(),
                elicitation_id: "test-auth".into(),
            },
        ));
        let expected_reviews = requests.len() + 1;
        for elicitation in requests {
            let pending = tokio::spawn(sender(RequestId::Number(7), elicitation));
            let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
                .await
                .expect("elicitation should prompt")
                .expect("event channel should remain open");
            let EventMsg::ElicitationRequest(request) = event.msg else {
                panic!("expected MCP elicitation");
            };
            assert!(!pending.is_finished());
            let ProtocolRequestId::String(request_id) = request.id else {
                panic!("expected Codex-owned string request ID");
            };
            let response = ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({"answer": "continue"})),
                meta: None,
            };
            manager
                .router
                .resolve(
                    request.server_name,
                    RequestId::String(request_id.into()),
                    response.clone(),
                )
                .await
                .unwrap();
            assert_eq!(pending.await.unwrap().unwrap(), response);
        }
        assert_eq!(
            send_elicitation(&sender, Some(json!(true))).await,
            strict_auto_review_decline()
        );
        if let Some(reviewer) = reviewer {
            assert_eq!(reviewer.calls.load(Relaxed), expected_reviews);
            assert_eq!(reviewer.active_elicitations.load(Relaxed), 0);
        }
        assert!(events.is_empty());
        assert!(manager.router.requests.lock().unwrap().is_empty());
    }
}

async fn assert_declined(marker: Value, response: Option<ReviewerResponse>) {
    let expected_calls = usize::from(marker == Value::Bool(true));
    let reviewer = response.map(RecordingReviewer::new);
    let (_, events, sender) = elicitation_fixture(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        reviewer.clone(),
    );
    assert_eq!(
        send_elicitation(&sender, Some(marker)).await,
        strict_auto_review_decline()
    );
    if let Some(reviewer) = reviewer {
        assert_eq!(reviewer.calls.load(Relaxed), expected_calls);
    }
    assert!(events.is_empty());
}

#[test]
fn closed_event_channel_immediately_cleans_up_pending_elicitation() {
    let active_elicitations = Arc::new(AtomicUsize::new(0));
    let registrations = active_elicitations.clone();
    let lifecycle = ElicitationLifecycle::new(move || {
        registrations.fetch_add(/*val*/ 1, Relaxed);
        LifecycleRegistration(registrations.clone())
    });
    let (manager, events, sender) = elicitation_fixture(
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    assert!(manager.update(
        test_elicitation_config(
            "independent-mcp",
            AskForApproval::OnRequest,
            PermissionProfile::Disabled
        ),
        /*reviewer*/ None,
        Some(lifecycle),
    ));
    drop(events);

    let elicitation = Elicitation::Mcp(ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: "Review this request".to_string(),
        requested_schema: ElicitationSchema::builder().build().unwrap(),
    });
    let error = sender(RequestId::Number(7), elicitation)
        .now_or_never()
        .expect("closed event channel must not leave an elicitation pending")
        .expect_err("closed event channel must fail the elicitation");

    assert_eq!(
        error.to_string(),
        "failed to deliver MCP elicitation request"
    );
    assert!(
        manager
            .router
            .requests
            .lock()
            .expect("pending request router should be available")
            .is_empty()
    );
    assert_eq!(active_elicitations.load(Relaxed), 0);
}

#[tokio::test]
async fn strict_auto_review_respects_explicit_elicitation_denials() {
    for policy in [
        AskForApproval::OnRequest,
        AskForApproval::UnlessTrusted,
        AskForApproval::Never,
        AskForApproval::Granular(GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }),
    ] {
        let explicitly_denied = matches!(
            policy,
            AskForApproval::Granular(config) if !config.allows_mcp_elicitations()
        );
        let reviewer = RecordingReviewer::new(Ok(Some(approved_response())));
        let (manager, events, sender) =
            elicitation_fixture(policy, PermissionProfile::Disabled, Some(reviewer.clone()));
        assert_eq!(
            send_elicitation(&sender, Some(json!(true))).await,
            if explicitly_denied {
                strict_auto_review_decline()
            } else {
                approved_response()
            }
        );
        if policy == AskForApproval::Never {
            for (server_name, marker) in [
                ("independent-mcp", Some(json!(false))),
                ("another-independent-mcp", None),
            ] {
                let sender = manager.make_sender(
                    server_name.into(),
                    /*tx_event*/ None,
                    &ClientMcpExtensions::default(),
                );
                assert_eq!(
                    send_elicitation(&sender, marker).await,
                    ElicitationResponse {
                        meta: None,
                        ..approved_response()
                    },
                );
            }
        }
        manager.router.set_auto_deny(/*auto_deny*/ true);
        assert_eq!(
            send_elicitation(&sender, Some(json!(true))).await,
            ElicitationResponse {
                meta: None,
                ..strict_auto_review_decline()
            },
        );
        assert_eq!(
            (
                reviewer.calls.load(Relaxed),
                reviewer.active_elicitations.load(Relaxed)
            ),
            (usize::from(!explicitly_denied), 0),
        );
        assert!(events.is_empty(), "strict review must not emit an event");
    }
}

#[tokio::test]
async fn strict_auto_review_preserves_guardian_denials_and_cancellations() {
    for response in [
        ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: Some(json!({
                "approvals_reviewer": "auto_review",
                "message": "The user has not authorized sending this data. Ask the user for approval.",
            })),
        },
        ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: Some(json!({ "approvals_reviewer": "auto_review" })),
        },
    ] {
        let reviewer = RecordingReviewer::new(Ok(Some(response.clone())));
        let (_, events, sender) = elicitation_fixture(
            AskForApproval::Never,
            PermissionProfile::Disabled,
            Some(reviewer.clone()),
        );
        assert_eq!(send_elicitation(&sender, Some(json!(true))).await, response);
        assert_eq!(reviewer.calls.load(Relaxed), 1);
        assert!(events.is_empty(), "strict review must not emit an event");
    }
}

#[tokio::test]
async fn strict_auto_review_fails_closed_without_a_canonical_decision() {
    for marker in ["null", "\"true\"", "1", "{}", "[true]"] {
        let marker = serde_json::from_str(marker).expect("valid malformed marker");
        assert_declined(marker, Some(Ok(Some(approved_response())))).await;
    }
    for response in [Ok(None), Err("reviewer failed")] {
        assert_declined(json!(true), Some(response)).await;
    }
    let invalid_decisions: [fn(&mut ElicitationResponse); 6] = [
        |response| {
            response.action = ElicitationAction::Decline;
            response.meta = Some(json!({ "message": "Ask the user to approve this request." }));
        },
        |response| response.action = ElicitationAction::Cancel,
        |response| response.meta = None,
        |response| response.meta = Some(json!({ "approvals_reviewer": "user" })),
        |response| response.meta = Some(json!({ "approvals_reviewer": "guardian_subagent" })),
        |response| response.content = Some(json!({ "approved_for_session": true })),
    ];
    for make_invalid in invalid_decisions {
        let mut response = approved_response();
        make_invalid(&mut response);
        assert_declined(json!(true), Some(Ok(Some(response)))).await;
    }
    assert_declined(json!(true), /*response*/ None).await;
}

#[tokio::test]
async fn reused_elicitation_senders_follow_each_servers_latest_permission_authority() {
    let mut config = crate::mcp::tests::test_mcp_config(std::env::temp_dir());
    config.approval_policy = codex_config::Constrained::allow_any(AskForApproval::Never);
    config.permission_profile = PermissionProfile::Disabled;
    config.apps_enabled = true;
    let auth = codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let hosted_server = crate::codex_apps_mcp_server_config(
        "https://example.com",
        /*apps_mcp_product_sku*/ None,
        /*originator*/ None,
    );
    let mut attached_server = hosted_server.clone();
    attached_server.environment_id = "attached".to_string();
    let mut catalog = crate::ResolvedMcpCatalog::builder();
    catalog.register(crate::McpServerRegistration::from_config(
        "attached".to_string(),
        attached_server,
    ));
    catalog.register(crate::McpServerRegistration::from_hosted_apps(
        "host",
        /*contribution_order*/ 0,
        hosted_server,
    ));
    config.mcp_server_catalog = catalog.build();
    let servers = crate::effective_mcp_servers(&config, Some(&auth));
    config.set_server_permission_profiles(
        &servers,
        [("attached".to_string(), PermissionProfile::read_only())],
    );

    let manager = ElicitationRequestManager::new(
        Arc::new(config.clone()),
        /*reviewer*/ None,
        /*lifecycle*/ None,
        ElicitationRequestRouter::default(),
    );
    let attached = manager.make_sender(
        "attached".to_string(),
        /*tx_event*/ None,
        &ClientMcpExtensions::default(),
    );
    let hosted = manager.make_sender(
        crate::CODEX_APPS_MCP_SERVER_NAME.to_string(),
        /*tx_event*/ None,
        &ClientMcpExtensions::default(),
    );

    assert_eq!(
        send_elicitation(&attached, /*marker*/ None).await.action,
        ElicitationAction::Decline
    );
    assert_eq!(
        send_elicitation(&hosted, /*marker*/ None).await.action,
        ElicitationAction::Accept
    );

    config.set_server_permission_profiles(
        &servers,
        [("attached".to_string(), PermissionProfile::Disabled)],
    );
    assert!(manager.update(
        Arc::new(config.clone()),
        /*reviewer*/ None,
        /*lifecycle*/ None,
    ));
    assert_eq!(
        send_elicitation(&attached, /*marker*/ None).await.action,
        ElicitationAction::Accept
    );

    let mut configured_servers = config.mcp_server_catalog.configured_servers();
    configured_servers
        .get_mut("attached")
        .expect("attached server should be registered")
        .enabled = false;
    config.mcp_server_catalog = config
        .mcp_server_catalog
        .with_materialized_servers(configured_servers);
    let servers = crate::effective_mcp_servers(&config, Some(&auth));
    config.set_server_permission_profiles(
        &servers,
        [("attached".to_string(), PermissionProfile::Disabled)],
    );
    assert!(manager.update(
        Arc::new(config.clone()),
        /*reviewer*/ None,
        /*lifecycle*/ None,
    ));
    assert_eq!(
        send_elicitation(&attached, /*marker*/ None).await.action,
        ElicitationAction::Decline
    );

    let servers = crate::effective_mcp_servers(&config, /*auth*/ None);
    config.set_server_permission_profiles(&servers, std::iter::empty());
    assert!(manager.update(
        Arc::new(config.clone()),
        /*reviewer*/ None,
        /*lifecycle*/ None,
    ));
    assert_eq!(
        send_elicitation(&hosted, /*marker*/ None).await.action,
        ElicitationAction::Decline
    );
}

fn verification_fixture(
    approval_policy: AskForApproval,
    reviewer: Option<Arc<RecordingReviewer>>,
) -> (ElicitationRequestManager, Receiver<Event>, SendElicitation) {
    let mut config = test_elicitation_config(
        crate::CODEX_APPS_MCP_SERVER_NAME,
        approval_policy,
        PermissionProfile::Disabled,
    );
    let mut catalog = crate::catalog::ResolvedMcpCatalog::builder();
    catalog.register(crate::catalog::McpServerRegistration::from_hosted_apps(
        "verification-test",
        /*contribution_order*/ 0,
        crate::mcp::codex_apps_mcp_server_config(
            "https://example.com",
            /*apps_mcp_product_sku*/ None,
            /*originator*/ None,
        ),
    ));
    Arc::make_mut(&mut config).mcp_server_catalog = catalog.build();
    let manager = ElicitationRequestManager::new(
        config,
        reviewer.map(|reviewer| reviewer as Arc<dyn ElicitationReviewer>),
        /*lifecycle*/ None,
        ElicitationRequestRouter::default(),
    );
    let (tx, events) = async_channel::bounded(1);
    let sender = manager.make_sender(
        crate::CODEX_APPS_MCP_SERVER_NAME.into(),
        Some(tx),
        &ClientMcpExtensions::new([(
            OPENAI_ELICITATION_EXTENSION_ID.to_string(),
            json!({"userVerification": {}}),
        )]),
    );
    (manager, events, sender)
}

#[tokio::test]
async fn user_verification_requires_the_app_even_when_policy_would_approve_or_decline() {
    for approval_policy in [AskForApproval::Never, AskForApproval::OnRequest] {
        let reviewer = RecordingReviewer::new(Ok(Some(approved_response())));
        let (manager, events, sender) =
            verification_fixture(approval_policy, Some(reviewer.clone()));
        let pending = tokio::spawn(sender(
            RequestId::Number(7),
            Elicitation::UserVerification {
                meta: Some(json!({"example/display": {"label": "Operation"}})),
                title: "Approve purchase".into(),
                description: "Pay $200".into(),
                challenge: "AQID".into(),
            },
        ));
        let event = events.recv().await.unwrap();
        let EventMsg::ElicitationRequest(request) = event.msg else {
            panic!("expected typed elicitation");
        };
        assert_eq!(
            request.request,
            ElicitationRequest::UserVerification {
                meta: Some(json!({"example/display": {"label": "Operation"}})),
                title: "Approve purchase".into(),
                description: "Pay $200".into(),
                challenge: "AQID".into(),
            }
        );
        assert_eq!(reviewer.calls.load(Relaxed), 0);
        let ProtocolRequestId::String(id) = request.id else {
            panic!("expected routed request id");
        };
        let proof = ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(json!({"credentialId": "AQID", "signature": "BAUG"})),
            meta: None,
        };
        manager
            .router
            .resolve(
                request.server_name.clone(),
                RequestId::String(id.clone().into()),
                proof.clone(),
            )
            .await
            .unwrap();
        assert_eq!(pending.await.unwrap().unwrap(), proof);
        assert!(
            manager
                .router
                .resolve(request.server_name, RequestId::String(id.into()), proof)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn user_verification_cancels_when_no_app_can_receive_the_request() {
    let (manager, _, _) = verification_fixture(AskForApproval::OnRequest, /*reviewer*/ None);
    let sender = manager.make_sender(
        crate::CODEX_APPS_MCP_SERVER_NAME.into(),
        /*tx_event*/ None,
        &ClientMcpExtensions::new([(
            OPENAI_ELICITATION_EXTENSION_ID.to_string(),
            json!({"userVerification": {}}),
        )]),
    );
    assert_eq!(
        sender(
            RequestId::Number(7),
            Elicitation::UserVerification {
                meta: None,
                title: "Approve".into(),
                description: String::new(),
                challenge: "AQID".into(),
            }
        )
        .await
        .unwrap(),
        ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: None
        },
    );
}

#[tokio::test]
async fn user_verification_cancels_for_an_event_receiver_without_host_activation() {
    let (manager, _, _) = verification_fixture(AskForApproval::OnRequest, /*reviewer*/ None);
    let (tx, events) = async_channel::bounded(1);
    let sender = manager.make_sender(
        crate::CODEX_APPS_MCP_SERVER_NAME.into(),
        Some(tx),
        &ClientMcpExtensions::default(),
    );
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        sender(
            RequestId::Number(7),
            Elicitation::UserVerification {
                meta: None,
                title: "Approve".into(),
                description: String::new(),
                challenge: "AQID".into(),
            },
        ),
    )
    .await
    .expect("an inactive host must not wait for a response")
    .unwrap();
    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: None,
        },
    );
    assert!(events.try_recv().is_err());
    assert!(manager.router.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn user_verification_drops_pending_response_when_the_request_is_cancelled() {
    let (manager, events, sender) =
        verification_fixture(AskForApproval::OnRequest, /*reviewer*/ None);
    let active_elicitations = Arc::new(AtomicUsize::new(0));
    let registrations = active_elicitations.clone();
    manager
        .authority
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .lifecycle = Some(ElicitationLifecycle::new(move || {
        registrations.fetch_add(/*val*/ 1, Relaxed);
        LifecycleRegistration(registrations.clone())
    }));
    let pending = tokio::spawn(sender(
        RequestId::Number(7),
        Elicitation::UserVerification {
            meta: None,
            title: "Approve".into(),
            description: String::new(),
            challenge: "AQID".into(),
        },
    ));
    events.recv().await.unwrap();
    assert_eq!(active_elicitations.load(Relaxed), 1);
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert!(manager.router.requests.lock().unwrap().is_empty());
    assert_eq!(active_elicitations.load(Relaxed), 0);
}

#[tokio::test]
async fn user_verification_rejects_attached_servers_even_if_they_use_the_plugin_service_name() {
    for name in ["attached", crate::CODEX_APPS_MCP_SERVER_NAME] {
        let (manager, _, _) =
            verification_fixture(AskForApproval::OnRequest, /*reviewer*/ None);
        {
            let mut authority = manager.authority.lock().unwrap();
            let config = Arc::make_mut(&mut authority.as_mut().unwrap().config);
            let mut catalog = crate::catalog::ResolvedMcpCatalog::builder();
            catalog.register(crate::catalog::McpServerRegistration::from_config(
                name.into(),
                crate::mcp::codex_apps_mcp_server_config(
                    "https://example.com",
                    /*apps_mcp_product_sku*/ None,
                    /*originator*/ None,
                ),
            ));
            config.mcp_server_catalog = catalog.build();
        }
        let (tx, events) = async_channel::bounded(1);
        let sender = manager.make_sender(
            name.into(),
            Some(tx),
            &ClientMcpExtensions::new([(
                OPENAI_ELICITATION_EXTENSION_ID.to_string(),
                json!({"userVerification": {}}),
            )]),
        );
        assert_eq!(
            sender(
                RequestId::Number(7),
                Elicitation::UserVerification {
                    meta: None,
                    title: "Approve".into(),
                    description: String::new(),
                    challenge: "AQID".into()
                }
            )
            .await
            .unwrap(),
            ElicitationResponse {
                action: ElicitationAction::Cancel,
                content: None,
                meta: None
            },
        );
        assert!(events.try_recv().is_err());
    }
}
