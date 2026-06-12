use std::sync::Arc;
use std::sync::Mutex;

use codex_extension_api::ApprovalReviewContributor;
use codex_extension_api::ApprovalReviewError;
use codex_extension_api::ApprovalReviewInput;
use codex_extension_api::ApprovalReviewOutcome;
use codex_extension_api::ApprovalReviewSource;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::TokenUsageContributor;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_extension_api::TurnItemContributor;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::empty_extension_registry;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::HookPromptItem;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::WarningEvent;
use pretty_assertions::assert_eq;

struct AllContributors;

impl ContextContributor for AllContributors {
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(std::future::ready(Vec::new()))
    }
}

impl ThreadLifecycleContributor<()> for AllContributors {}

impl TurnLifecycleContributor for AllContributors {}

impl ConfigContributor<()> for AllContributors {}

impl TokenUsageContributor for AllContributors {}

impl TurnInputContributor for AllContributors {
    fn contribute<'a>(
        &'a self,
        input: TurnInputContext,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
            Vec::new()
        })
    }
}

impl ToolContributor for AllContributors {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        Vec::new()
    }
}

impl ToolLifecycleContributor for AllContributors {}

impl TurnItemContributor for AllContributors {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        _item: &'a mut TurnItem,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let _self = self;
            Ok(())
        })
    }
}

impl ApprovalReviewContributor for AllContributors {
    fn review<'a>(
        &'a self,
        _input: ApprovalReviewInput<'a>,
    ) -> ExtensionFuture<'a, Result<ApprovalReviewOutcome, ApprovalReviewError>> {
        Box::pin(async move {
            let _self = self;
            Ok(ApprovalReviewOutcome::Decision(
                ReviewDecision::ApprovedForSession,
            ))
        })
    }
}

#[tokio::test]
async fn build_round_trips_every_contributor_category() {
    let contributor = Arc::new(AllContributors);
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    builder.thread_lifecycle_contributor(contributor.clone());
    builder.turn_lifecycle_contributor(contributor.clone());
    builder.config_contributor(contributor.clone());
    builder.token_usage_contributor(contributor.clone());
    builder.prompt_contributor(contributor.clone());
    builder.turn_input_contributor(contributor.clone());
    builder.tool_contributor(contributor.clone());
    builder.tool_lifecycle_contributor(contributor.clone());
    builder.turn_item_contributor(contributor.clone());
    builder.approval_review_contributor(contributor);
    let registry = builder.build();

    assert_eq!(registry.thread_lifecycle_contributors().len(), 1);
    assert_eq!(registry.turn_lifecycle_contributors().len(), 1);
    assert_eq!(registry.config_contributors().len(), 1);
    assert_eq!(registry.token_usage_contributors().len(), 1);
    assert_eq!(registry.context_contributors().len(), 1);
    assert_eq!(registry.turn_input_contributors().len(), 1);
    assert_eq!(registry.tool_contributors().len(), 1);
    assert_eq!(registry.tool_lifecycle_contributors().len(), 1);
    assert_eq!(registry.turn_item_contributors().len(), 1);
    assert_eq!(
        registry
            .approval_review(approval_review_input(
                &ExtensionData::new("session"),
                &ExtensionData::new("thread"),
                &ExtensionData::new("turn"),
                &test_action(),
                &AskForApproval::OnRequest,
            ))
            .await,
        Ok(ApprovalReviewOutcome::Decision(
            ReviewDecision::ApprovedForSession
        ))
    );
}

struct NamedContextContributor(&'static str);

impl ContextContributor for NamedContextContributor {
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(std::future::ready(vec![PromptFragment::developer_policy(
            self.0,
        )]))
    }
}

struct RecordingTurnItemContributor {
    name: &'static str,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl TurnItemContributor for RecordingTurnItemContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        _item: &'a mut TurnItem,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(|error| panic!("turn item calls lock poisoned: {error}"))
                .push(self.name);
            Ok(())
        })
    }
}

#[tokio::test]
async fn contributors_preserve_registration_order() {
    let turn_item_calls = Arc::new(Mutex::new(Vec::new()));
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    builder.prompt_contributor(Arc::new(NamedContextContributor("first")));
    builder.prompt_contributor(Arc::new(NamedContextContributor("second")));
    for name in ["first", "second"] {
        builder.turn_item_contributor(Arc::new(RecordingTurnItemContributor {
            name,
            calls: Arc::clone(&turn_item_calls),
        }));
    }
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let turn_store = ExtensionData::new("turn");

    let mut fragments = Vec::new();
    for contributor in registry.context_contributors() {
        fragments.extend(contributor.contribute(&session_store, &thread_store).await);
    }
    let mut item = TurnItem::HookPrompt(HookPromptItem {
        id: "item".to_string(),
        fragments: Vec::new(),
    });
    for contributor in registry.turn_item_contributors() {
        contributor
            .contribute(&thread_store, &turn_store, &mut item)
            .await
            .expect("turn item contribution should succeed");
    }

    assert_eq!(
        fragments,
        vec![
            PromptFragment::developer_policy("first"),
            PromptFragment::developer_policy("second"),
        ]
    );
    assert_eq!(
        turn_item_calls
            .lock()
            .expect("turn item calls lock")
            .as_slice(),
        ["first", "second"]
    );
}

#[derive(Debug, PartialEq)]
struct ApprovalCall {
    contributor: &'static str,
    session_id: String,
    thread_id: String,
    turn_store_id: String,
    review_id: String,
    turn_id: String,
    target_item_id: Option<String>,
    prompt: String,
    action: GuardianAssessmentAction,
    reviewer: ApprovalsReviewer,
    approval_policy: AskForApproval,
    retry_reason: Option<String>,
    source: ApprovalReviewSource,
}

struct RecordingApprovalContributor {
    name: &'static str,
    result: Result<ApprovalReviewOutcome, ApprovalReviewError>,
    calls: Arc<Mutex<Vec<ApprovalCall>>>,
}

impl ApprovalReviewContributor for RecordingApprovalContributor {
    fn review<'a>(
        &'a self,
        input: ApprovalReviewInput<'a>,
    ) -> ExtensionFuture<'a, Result<ApprovalReviewOutcome, ApprovalReviewError>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(|error| panic!("approval calls lock poisoned: {error}"))
                .push(ApprovalCall {
                    contributor: self.name,
                    session_id: input.session_store.level_id().to_string(),
                    thread_id: input.thread_store.level_id().to_string(),
                    turn_store_id: input.turn_store.level_id().to_string(),
                    review_id: input.review_id.to_string(),
                    turn_id: input.turn_id.to_string(),
                    target_item_id: input.target_item_id.map(str::to_string),
                    prompt: input.prompt.to_string(),
                    action: input.action.clone(),
                    reviewer: input.reviewer,
                    approval_policy: input.approval_policy.clone(),
                    retry_reason: input.retry_reason.map(str::to_string),
                    source: input.source,
                });
            self.result.clone()
        })
    }
}

fn test_action() -> GuardianAssessmentAction {
    GuardianAssessmentAction::RequestPermissions {
        reason: Some("run delegated command".to_string()),
        permissions: Default::default(),
    }
}

fn approval_review_input<'a>(
    session_store: &'a ExtensionData,
    thread_store: &'a ExtensionData,
    turn_store: &'a ExtensionData,
    action: &'a GuardianAssessmentAction,
    approval_policy: &'a AskForApproval,
) -> ApprovalReviewInput<'a> {
    ApprovalReviewInput {
        session_store,
        thread_store,
        turn_store,
        review_id: "review-1",
        turn_id: "turn-1",
        target_item_id: Some("item-1"),
        prompt: "allow delegated command?",
        action,
        reviewer: ApprovalsReviewer::AutoReview,
        approval_policy,
        retry_reason: Some("initial review timed out"),
        source: ApprovalReviewSource::DelegatedSubagent,
    }
}

#[tokio::test]
async fn approval_review_returns_first_claim_and_short_circuits() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    for (name, result) in [
        ("first", Ok(ApprovalReviewOutcome::Abstain)),
        (
            "second",
            Ok(ApprovalReviewOutcome::Decision(ReviewDecision::Approved)),
        ),
        (
            "third",
            Ok(ApprovalReviewOutcome::Decision(ReviewDecision::Denied)),
        ),
    ] {
        builder.approval_review_contributor(Arc::new(RecordingApprovalContributor {
            name,
            result,
            calls: Arc::clone(&calls),
        }));
    }
    let registry = builder.build();
    let session_store = ExtensionData::new("session-1");
    let thread_store = ExtensionData::new("thread-1");
    let turn_store = ExtensionData::new("turn-store-1");
    let action = test_action();
    let approval_policy = AskForApproval::OnRequest;

    let decision = registry
        .approval_review(approval_review_input(
            &session_store,
            &thread_store,
            &turn_store,
            &action,
            &approval_policy,
        ))
        .await;

    assert_eq!(
        decision,
        Ok(ApprovalReviewOutcome::Decision(ReviewDecision::Approved))
    );
    let expected_call = |contributor| ApprovalCall {
        contributor,
        session_id: "session-1".to_string(),
        thread_id: "thread-1".to_string(),
        turn_store_id: "turn-store-1".to_string(),
        review_id: "review-1".to_string(),
        turn_id: "turn-1".to_string(),
        target_item_id: Some("item-1".to_string()),
        prompt: "allow delegated command?".to_string(),
        action: action.clone(),
        reviewer: ApprovalsReviewer::AutoReview,
        approval_policy: AskForApproval::OnRequest,
        retry_reason: Some("initial review timed out".to_string()),
        source: ApprovalReviewSource::DelegatedSubagent,
    };
    assert_eq!(
        calls.lock().expect("approval calls lock").as_slice(),
        [expected_call("first"), expected_call("second")]
    );
}

#[tokio::test]
async fn approval_review_error_stops_dispatch() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    for (name, result) in [
        ("first", Ok(ApprovalReviewOutcome::Abstain)),
        ("second", Err(ApprovalReviewError::new("review failed"))),
        (
            "third",
            Ok(ApprovalReviewOutcome::Decision(ReviewDecision::Approved)),
        ),
    ] {
        builder.approval_review_contributor(Arc::new(RecordingApprovalContributor {
            name,
            result,
            calls: Arc::clone(&calls),
        }));
    }
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let turn_store = ExtensionData::new("turn");
    let action = test_action();
    let approval_policy = AskForApproval::OnRequest;

    let result = registry
        .approval_review(approval_review_input(
            &session_store,
            &thread_store,
            &turn_store,
            &action,
            &approval_policy,
        ))
        .await;

    assert_eq!(result, Err(ApprovalReviewError::new("review failed")));
    assert_eq!(
        calls
            .lock()
            .expect("approval calls lock")
            .iter()
            .map(|call| call.contributor)
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
}

#[derive(Default)]
struct RecordingEventSink {
    events: Mutex<Vec<(String, String)>>,
}

impl ExtensionEventSink for RecordingEventSink {
    fn emit(&self, event: Event) {
        let EventMsg::Warning(warning) = event.msg else {
            panic!("test sink only accepts warning events");
        };
        self.events
            .lock()
            .unwrap_or_else(|error| panic!("recording event sink lock poisoned: {error}"))
            .push((event.id, warning.message));
    }
}

#[test]
fn custom_event_sink_survives_registry_build() {
    let sink = Arc::new(RecordingEventSink::default());
    let builder = ExtensionRegistryBuilder::<()>::with_event_sink(sink.clone());

    builder
        .event_sink()
        .emit(warning_event("builder", "before"));
    let registry = builder.build();
    registry
        .event_sink()
        .emit(warning_event("registry", "after"));

    assert_eq!(
        sink.events
            .lock()
            .expect("recording event sink lock")
            .as_slice(),
        [
            ("builder".to_string(), "before".to_string()),
            ("registry".to_string(), "after".to_string()),
        ]
    );
}

#[tokio::test]
async fn empty_registry_does_not_claim_approval_review() {
    let registry = empty_extension_registry::<()>();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let turn_store = ExtensionData::new("turn");
    let action = test_action();
    let approval_policy = AskForApproval::OnRequest;

    assert_eq!(
        registry
            .approval_review(approval_review_input(
                &session_store,
                &thread_store,
                &turn_store,
                &action,
                &approval_policy,
            ))
            .await,
        Ok(ApprovalReviewOutcome::Abstain)
    );
}

fn warning_event(id: &str, message: &str) -> Event {
    Event {
        id: id.to_string(),
        msg: EventMsg::Warning(WarningEvent {
            message: message.to_string(),
        }),
    }
}
