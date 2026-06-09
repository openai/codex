use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use codex_extension_api::ApprovalReviewContributor;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
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
use codex_protocol::items::HookPromptItem;
use codex_protocol::items::TurnItem;
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
    ) -> Pin<Box<dyn Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(std::future::ready(Vec::new()))
    }
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<()> for AllContributors {}

#[async_trait::async_trait]
impl TurnLifecycleContributor for AllContributors {}

impl ConfigContributor<()> for AllContributors {}

#[async_trait::async_trait]
impl TokenUsageContributor for AllContributors {}

#[async_trait::async_trait]
impl TurnInputContributor for AllContributors {
    async fn contribute(
        &self,
        _input: TurnInputContext,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
        _turn_store: &ExtensionData,
    ) -> Vec<Box<dyn ContextualUserFragment + Send>> {
        Vec::new()
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

#[async_trait::async_trait]
impl TurnItemContributor for AllContributors {
    async fn contribute(
        &self,
        _thread_store: &ExtensionData,
        _turn_store: &ExtensionData,
        _item: &mut TurnItem,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl ApprovalReviewContributor for AllContributors {
    async fn contribute(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
        _prompt: &str,
    ) -> Option<ReviewDecision> {
        Some(ReviewDecision::ApprovedForSession)
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
            .approval_review(
                &ExtensionData::new("session"),
                &ExtensionData::new("thread"),
                "review this",
            )
            .await,
        Some(ReviewDecision::ApprovedForSession)
    );
}

struct NamedContextContributor(&'static str);

impl ContextContributor for NamedContextContributor {
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
    ) -> Pin<Box<dyn Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(std::future::ready(vec![PromptFragment::developer_policy(
            self.0,
        )]))
    }
}

struct RecordingTurnItemContributor {
    name: &'static str,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait::async_trait]
impl TurnItemContributor for RecordingTurnItemContributor {
    async fn contribute(
        &self,
        _thread_store: &ExtensionData,
        _turn_store: &ExtensionData,
        _item: &mut TurnItem,
    ) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap_or_else(|error| panic!("turn item calls lock poisoned: {error}"))
            .push(self.name);
        Ok(())
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

#[derive(Debug, PartialEq, Eq)]
struct ApprovalCall {
    contributor: &'static str,
    session_id: String,
    thread_id: String,
    prompt: String,
}

struct RecordingApprovalContributor {
    name: &'static str,
    decision: Option<ReviewDecision>,
    calls: Arc<Mutex<Vec<ApprovalCall>>>,
}

#[async_trait::async_trait]
impl ApprovalReviewContributor for RecordingApprovalContributor {
    async fn contribute(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        prompt: &str,
    ) -> Option<ReviewDecision> {
        self.calls
            .lock()
            .unwrap_or_else(|error| panic!("approval calls lock poisoned: {error}"))
            .push(ApprovalCall {
                contributor: self.name,
                session_id: session_store.level_id().to_string(),
                thread_id: thread_store.level_id().to_string(),
                prompt: prompt.to_string(),
            });
        self.decision.clone()
    }
}

#[tokio::test]
async fn approval_review_returns_first_claim_and_short_circuits() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    for (name, decision) in [
        ("first", None),
        ("second", Some(ReviewDecision::Approved)),
        ("third", Some(ReviewDecision::Denied)),
    ] {
        builder.approval_review_contributor(Arc::new(RecordingApprovalContributor {
            name,
            decision,
            calls: Arc::clone(&calls),
        }));
    }
    let registry = builder.build();

    let decision = registry
        .approval_review(
            &ExtensionData::new("session-1"),
            &ExtensionData::new("thread-1"),
            "allow command?",
        )
        .await;

    assert_eq!(decision, Some(ReviewDecision::Approved));
    assert_eq!(
        calls.lock().expect("approval calls lock").as_slice(),
        [
            ApprovalCall {
                contributor: "first",
                session_id: "session-1".to_string(),
                thread_id: "thread-1".to_string(),
                prompt: "allow command?".to_string(),
            },
            ApprovalCall {
                contributor: "second",
                session_id: "session-1".to_string(),
                thread_id: "thread-1".to_string(),
                prompt: "allow command?".to_string(),
            },
        ]
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

    assert_eq!(
        registry
            .approval_review(
                &ExtensionData::new("session"),
                &ExtensionData::new("thread"),
                "unclaimed",
            )
            .await,
        None
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
