use std::sync::Arc;

use codex_protocol::protocol::ReviewDecision;

use crate::ApprovalAssessment;
use crate::ApprovalReviewContributor;
use crate::ApprovalReviewError;
use crate::ApprovalReviewInput;
use crate::ConfigContributor;
use crate::ContextContributor;
use crate::ExtensionData;
use crate::ExtensionEventSink;
use crate::ExtensionMetrics;
use crate::McpServerContributor;
use crate::NoopExtensionEventSink;
use crate::SkillInvocationContributor;
use crate::ThreadLifecycleContributor;
use crate::TokenUsageContributor;
use crate::ToolContributor;
use crate::ToolLifecycleContributor;
use crate::TurnInputContributor;
use crate::TurnItemContributor;
use crate::TurnLifecycleContributor;

/// Mutable registry used while hosts register typed runtime contributions.
pub struct ExtensionRegistryBuilder<C: Sync> {
    registry: ExtensionRegistry<C>,
}

impl<C: Sync> Default for ExtensionRegistryBuilder<C> {
    fn default() -> Self {
        Self {
            registry: ExtensionRegistry {
                event_sink: Arc::new(NoopExtensionEventSink),
                thread_lifecycle_contributors: Vec::new(),
                turn_lifecycle_contributors: Vec::new(),
                config_contributors: Vec::new(),
                token_usage_contributors: Vec::new(),
                skill_invocation_contributors: Vec::new(),
                approval_review_contributors: Vec::new(),
                context_contributors: Vec::new(),
                mcp_server_contributors: Vec::new(),
                turn_input_contributors: Vec::new(),
                tool_contributors: Vec::new(),
                tool_lifecycle_contributors: Vec::new(),
                turn_item_contributors: Vec::new(),
            },
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
        let mut builder = Self::default();
        builder.registry.event_sink = event_sink;
        builder
    }

    /// Returns the host event sink to pass into extension constructors.
    pub fn event_sink(&self) -> Arc<dyn ExtensionEventSink> {
        Arc::clone(&self.registry.event_sink)
    }

    /// Registers one approval-review contributor.
    pub fn approval_review_contributor(&mut self, contributor: Arc<dyn ApprovalReviewContributor>) {
        self.registry.approval_review_contributors.push(contributor);
    }

    /// Registers one thread-lifecycle contributor.
    pub fn thread_lifecycle_contributor(
        &mut self,
        contributor: Arc<dyn ThreadLifecycleContributor<C>>,
    ) {
        self.registry
            .thread_lifecycle_contributors
            .push(contributor);
    }

    /// Registers one turn-lifecycle contributor.
    pub fn turn_lifecycle_contributor(&mut self, contributor: Arc<dyn TurnLifecycleContributor>) {
        self.registry.turn_lifecycle_contributors.push(contributor);
    }

    /// Registers one config contributor.
    pub fn config_contributor(&mut self, contributor: Arc<dyn ConfigContributor<C>>) {
        self.registry.config_contributors.push(contributor);
    }

    /// Registers one token-usage contributor.
    pub fn token_usage_contributor(&mut self, contributor: Arc<dyn TokenUsageContributor>) {
        self.registry.token_usage_contributors.push(contributor);
    }

    /// Registers one skill-invocation contributor.
    pub fn skill_invocation_contributor(
        &mut self,
        contributor: Arc<dyn SkillInvocationContributor>,
    ) {
        self.registry
            .skill_invocation_contributors
            .push(contributor);
    }

    /// Registers one prompt contributor.
    pub fn prompt_contributor(&mut self, contributor: Arc<dyn ContextContributor>) {
        self.registry.context_contributors.push(contributor);
    }

    /// Registers one runtime MCP server contributor.
    pub fn mcp_server_contributor(&mut self, contributor: Arc<dyn McpServerContributor<C>>) {
        self.registry.mcp_server_contributors.push(contributor);
    }

    /// Registers one turn-input contributor.
    pub fn turn_input_contributor(&mut self, contributor: Arc<dyn TurnInputContributor>) {
        self.registry.turn_input_contributors.push(contributor);
    }

    /// Registers one native tool contributor.
    pub fn tool_contributor(&mut self, contributor: Arc<dyn ToolContributor>) {
        self.registry.tool_contributors.push(contributor);
    }

    /// Registers one tool-lifecycle contributor.
    pub fn tool_lifecycle_contributor(&mut self, contributor: Arc<dyn ToolLifecycleContributor>) {
        self.registry.tool_lifecycle_contributors.push(contributor);
    }

    /// Registers one ordered turn-item contributor.
    pub fn turn_item_contributor(&mut self, contributor: Arc<dyn TurnItemContributor>) {
        self.registry.turn_item_contributors.push(contributor);
    }

    /// Finishes construction and returns the immutable registry.
    pub fn build(self) -> ExtensionRegistry<C> {
        self.registry
    }
}

/// Immutable typed registry produced after extensions are installed.
pub struct ExtensionRegistry<C: Sync> {
    event_sink: Arc<dyn ExtensionEventSink>,
    thread_lifecycle_contributors: Vec<Arc<dyn ThreadLifecycleContributor<C>>>,
    turn_lifecycle_contributors: Vec<Arc<dyn TurnLifecycleContributor>>,
    config_contributors: Vec<Arc<dyn ConfigContributor<C>>>,
    token_usage_contributors: Vec<Arc<dyn TokenUsageContributor>>,
    skill_invocation_contributors: Vec<Arc<dyn SkillInvocationContributor>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    mcp_server_contributors: Vec<Arc<dyn McpServerContributor<C>>>,
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

    /// Returns the registered skill-invocation contributors.
    pub fn skill_invocation_contributors(&self) -> &[Arc<dyn SkillInvocationContributor>] {
        &self.skill_invocation_contributors
    }

    /// Returns the first full approval assessment claimed by a contributor.
    pub async fn full_approval_review(
        &self,
        input: ApprovalReviewInput<'_>,
    ) -> Option<Result<ApprovalAssessment, ApprovalReviewError>> {
        for contributor in &self.approval_review_contributors {
            if let Some(assessment) = contributor.full_review(&input).await {
                return Some(assessment);
            }
        }

        None
    }

    /// Returns the first fast approval decision claimed by a contributor.
    pub async fn fast_approval_decision(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        prompt: &str,
        extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
    ) -> Option<ReviewDecision> {
        for contributor in &self.approval_review_contributors {
            if let Some(decision) = contributor
                .fast_decision(
                    session_store,
                    thread_store,
                    prompt,
                    extension_metrics.clone(),
                )
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

    /// Returns the registered runtime MCP server contributors.
    pub fn mcp_server_contributors(&self) -> &[Arc<dyn McpServerContributor<C>>] {
        &self.mcp_server_contributors
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
