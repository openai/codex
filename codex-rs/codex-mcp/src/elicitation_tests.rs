use super::*;
use async_channel::Receiver;
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
        let response = self.response.clone();
        async move {
            assert_eq!(active_elicitations.load(Relaxed), 1);
            tokio::task::yield_now().await;
            assert_eq!(active_elicitations.load(Relaxed), 1);
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
    let manager = ElicitationRequestManager::new(
        approval_policy,
        permission_profile,
        reviewer.map(|reviewer| reviewer as Arc<dyn ElicitationReviewer>),
        lifecycle,
        ElicitationRequestRouter::default(),
    );
    let (tx_event, events) = async_channel::bounded(1);
    let sender = manager.make_sender("independent-mcp".to_string(), Some(tx_event));
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
        AskForApproval::OnRequest,
        PermissionProfile::Disabled,
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
                let sender = manager.make_sender(server_name.into(), /*tx_event*/ None);
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
