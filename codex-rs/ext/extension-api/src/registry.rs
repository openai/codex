use std::sync::Arc;

use codex_protocol::protocol::ReviewDecision;

use crate::ApprovalReviewContributor;
use crate::ConfigContributor;
use crate::ContextContributor;
use crate::ExtensionData;
use crate::ExtensionEventSink;
use crate::InitialGoalContributor;
use crate::NoopExtensionEventSink;
use crate::ThreadLifecycleContributor;
use crate::TokenUsageContributor;
use crate::ToolContributor;
use crate::ToolLifecycleContributor;
use crate::TurnInputContributor;
use crate::TurnItemContributor;
use crate::TurnLifecycleContributor;

/// Mutable registry used while hosts register typed runtime contributions.
pub struct ExtensionRegistryBuilder<C: Sync> {
    event_sink: Arc<dyn ExtensionEventSink>,
    initial_goal_contributor: Option<Arc<dyn InitialGoalContributor>>,
    thread_lifecycle_contributors: Vec<Arc<dyn ThreadLifecycleContributor<C>>>,
    turn_lifecycle_contributors: Vec<Arc<dyn TurnLifecycleContributor>>,
    config_contributors: Vec<Arc<dyn ConfigContributor<C>>>,
    token_usage_contributors: Vec<Arc<dyn TokenUsageContributor>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    turn_input_contributors: Vec<Arc<dyn TurnInputContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    tool_lifecycle_contributors: Vec<Arc<dyn ToolLifecycleContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C: Sync> Default for ExtensionRegistryBuilder<C> {
    fn default() -> Self {
        Self {
            event_sink: Arc::new(NoopExtensionEventSink),
            initial_goal_contributor: None,
            thread_lifecycle_contributors: Vec::new(),
            turn_lifecycle_contributors: Vec::new(),
            config_contributors: Vec::new(),
            token_usage_contributors: Vec::new(),
            approval_review_contributors: Vec::new(),
            context_contributors: Vec::new(),
            turn_input_contributors: Vec::new(),
            tool_contributors: Vec::new(),
            tool_lifecycle_contributors: Vec::new(),
            turn_item_contributors: Vec::new(),
        }
    }
}

impl<C: Sync> ExtensionRegistryBuilder<C> {
    /// Creates an empty registry builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty registry builder with a host-provided event sink.
    pub fn with_event_sink(event_sink: Arc<dyn ExtensionEventSink>) -> Self {
        Self {
            event_sink,
            ..Self::default()
        }
    }

    /// Returns the host event sink to pass into extension constructors.
    pub fn event_sink(&self) -> Arc<dyn ExtensionEventSink> {
        Arc::clone(&self.event_sink)
    }

    /// Registers one approval-review contributor.
    pub fn approval_review_contributor(&mut self, contributor: Arc<dyn ApprovalReviewContributor>) {
        self.approval_review_contributors.push(contributor);
    }

    /// Registers one thread-lifecycle contributor.
    pub fn thread_lifecycle_contributor(
        &mut self,
        contributor: Arc<dyn ThreadLifecycleContributor<C>>,
    ) {
        self.thread_lifecycle_contributors.push(contributor);
    }

    /// Registers the extension that owns persisted thread goals.
    pub fn initial_goal_contributor(&mut self, contributor: Arc<dyn InitialGoalContributor>) {
        self.initial_goal_contributor = Some(contributor);
    }

    /// Registers one turn-lifecycle contributor.
    pub fn turn_lifecycle_contributor(&mut self, contributor: Arc<dyn TurnLifecycleContributor>) {
        self.turn_lifecycle_contributors.push(contributor);
    }

    /// Registers one config contributor.
    pub fn config_contributor(&mut self, contributor: Arc<dyn ConfigContributor<C>>) {
        self.config_contributors.push(contributor);
    }

    /// Registers one token-usage contributor.
    pub fn token_usage_contributor(&mut self, contributor: Arc<dyn TokenUsageContributor>) {
        self.token_usage_contributors.push(contributor);
    }

    /// Registers one prompt contributor.
    pub fn prompt_contributor(&mut self, contributor: Arc<dyn ContextContributor>) {
        self.context_contributors.push(contributor);
    }

    /// Registers one turn-input contributor.
    pub fn turn_input_contributor(&mut self, contributor: Arc<dyn TurnInputContributor>) {
        self.turn_input_contributors.push(contributor);
    }

    /// Registers one native tool contributor.
    pub fn tool_contributor(&mut self, contributor: Arc<dyn ToolContributor>) {
        self.tool_contributors.push(contributor);
    }

    /// Registers one tool-lifecycle contributor.
    pub fn tool_lifecycle_contributor(&mut self, contributor: Arc<dyn ToolLifecycleContributor>) {
        self.tool_lifecycle_contributors.push(contributor);
    }

    /// Registers one ordered turn-item contributor.
    pub fn turn_item_contributor(&mut self, contributor: Arc<dyn TurnItemContributor>) {
        self.turn_item_contributors.push(contributor);
    }

    /// Finishes construction and returns the immutable registry.
    pub fn build(self) -> ExtensionRegistry<C> {
        ExtensionRegistry {
            event_sink: self.event_sink,
            initial_goal_contributor: self.initial_goal_contributor,
            thread_lifecycle_contributors: self.thread_lifecycle_contributors,
            turn_lifecycle_contributors: self.turn_lifecycle_contributors,
            config_contributors: self.config_contributors,
            token_usage_contributors: self.token_usage_contributors,
            approval_review_contributors: self.approval_review_contributors,
            context_contributors: self.context_contributors,
            turn_input_contributors: self.turn_input_contributors,
            tool_contributors: self.tool_contributors,
            tool_lifecycle_contributors: self.tool_lifecycle_contributors,
            turn_item_contributors: self.turn_item_contributors,
        }
    }
}

/// Immutable typed registry produced after extensions are installed.
pub struct ExtensionRegistry<C: Sync> {
    event_sink: Arc<dyn ExtensionEventSink>,
    initial_goal_contributor: Option<Arc<dyn InitialGoalContributor>>,
    thread_lifecycle_contributors: Vec<Arc<dyn ThreadLifecycleContributor<C>>>,
    turn_lifecycle_contributors: Vec<Arc<dyn TurnLifecycleContributor>>,
    config_contributors: Vec<Arc<dyn ConfigContributor<C>>>,
    token_usage_contributors: Vec<Arc<dyn TokenUsageContributor>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    turn_input_contributors: Vec<Arc<dyn TurnInputContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    tool_lifecycle_contributors: Vec<Arc<dyn ToolLifecycleContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C: Sync> ExtensionRegistry<C> {
    /// Returns the host event sink retained by this registry.
    pub fn event_sink(&self) -> Arc<dyn ExtensionEventSink> {
        Arc::clone(&self.event_sink)
    }

    /// Returns the extension that owns initial goal replacement, when installed.
    pub fn initial_goal_contributor(&self) -> Option<&Arc<dyn InitialGoalContributor>> {
        self.initial_goal_contributor.as_ref()
    }

    /// Returns the registered thread-lifecycle contributors.
    pub fn thread_lifecycle_contributors(&self) -> &[Arc<dyn ThreadLifecycleContributor<C>>] {
        &self.thread_lifecycle_contributors
    }

    /// Returns the registered turn-lifecycle contributors.
    pub fn turn_lifecycle_contributors(&self) -> &[Arc<dyn TurnLifecycleContributor>] {
        &self.turn_lifecycle_contributors
    }

    /// Returns the registered config contributors.
    pub fn config_contributors(&self) -> &[Arc<dyn ConfigContributor<C>>] {
        &self.config_contributors
    }

    /// Returns the registered token-usage contributors.
    pub fn token_usage_contributors(&self) -> &[Arc<dyn TokenUsageContributor>] {
        &self.token_usage_contributors
    }

    /// Claims the first rendered approval-review prompt accepted by an
    /// installed contributor.
    pub async fn approval_review(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        prompt: &str,
    ) -> Option<ReviewDecision> {
        for contributor in &self.approval_review_contributors {
            if let Some(decision) = contributor
                .contribute(session_store, thread_store, prompt)
                .await
            {
                return Some(decision);
            }
        }

        None
    }

    /// Returns the registered prompt contributors.
    pub fn context_contributors(&self) -> &[Arc<dyn ContextContributor>] {
        &self.context_contributors
    }

    /// Returns the registered turn-input contributors.
    pub fn turn_input_contributors(&self) -> &[Arc<dyn TurnInputContributor>] {
        &self.turn_input_contributors
    }

    /// Returns the registered native tool contributors.
    pub fn tool_contributors(&self) -> &[Arc<dyn ToolContributor>] {
        &self.tool_contributors
    }

    /// Returns the registered tool-lifecycle contributors.
    pub fn tool_lifecycle_contributors(&self) -> &[Arc<dyn ToolLifecycleContributor>] {
        &self.tool_lifecycle_contributors
    }

    /// Returns the registered ordered turn-item contributors.
    pub fn turn_item_contributors(&self) -> &[Arc<dyn TurnItemContributor>] {
        &self.turn_item_contributors
    }
}

/// Creates an empty shared registry for hosts that do not register contributions.
pub fn empty_extension_registry<C: Sync>() -> Arc<ExtensionRegistry<C>> {
    Arc::new(ExtensionRegistryBuilder::new().build())
}
