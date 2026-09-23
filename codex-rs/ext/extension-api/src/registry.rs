use std::sync::Arc;

use crate::ApprovalReviewContributor;
use crate::ConfigContributor;
use crate::ContextContributor;
use crate::ExtensionEventSink;
use crate::McpServerContributor;
use crate::ModelRequestContributor;
use crate::NoopExtensionEventSink;
use crate::SkillInvocationContributor;
use crate::ThreadLifecycleContributor;
use crate::TokenUsageContributor;
use crate::ToolContributor;
use crate::ToolLifecycleContributor;
use crate::TurnInputContributor;
use crate::TurnItemContributor;
use crate::TurnLifecycleContributor;
use crate::TurnStartAdmission;

/// Mutable registry used while hosts register typed runtime contributions.
pub struct ExtensionRegistryBuilder<C: Sync> {
    registry: ExtensionRegistry<C>,
}

impl<C: Sync> Default for ExtensionRegistryBuilder<C> {
    fn default() -> Self {
        Self {
            registry: ExtensionRegistry {
                event_sink: Arc::new(NoopExtensionEventSink),
                turn_start_admission: None,
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
                model_request_contributors: Vec::new(),
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

    /// Installs the host gate for turn-input submissions that start a new turn.
    pub fn turn_start_admission(&mut self, admission: Arc<dyn TurnStartAdmission>) {
        self.registry.turn_start_admission = Some(admission);
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

    /// Registers a contributor for request-scoped response interception.
    pub fn model_request_contributor(&mut self, contributor: Arc<dyn ModelRequestContributor>) {
        self.registry.model_request_contributors.push(contributor);
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
    turn_start_admission: Option<Arc<dyn TurnStartAdmission>>,
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
    model_request_contributors: Vec<Arc<dyn ModelRequestContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C: Sync> ExtensionRegistry<C> {
    /// Copies the registered contributors into a builder for host-specific additions.
    pub fn to_builder(&self) -> ExtensionRegistryBuilder<C> {
        ExtensionRegistryBuilder {
            registry: Self {
                event_sink: self.event_sink.clone(),
                turn_start_admission: self.turn_start_admission.clone(),
                thread_lifecycle_contributors: self.thread_lifecycle_contributors.clone(),
                turn_lifecycle_contributors: self.turn_lifecycle_contributors.clone(),
                config_contributors: self.config_contributors.clone(),
                token_usage_contributors: self.token_usage_contributors.clone(),
                skill_invocation_contributors: self.skill_invocation_contributors.clone(),
                context_contributors: self.context_contributors.clone(),
                mcp_server_contributors: self.mcp_server_contributors.clone(),
                turn_input_contributors: self.turn_input_contributors.clone(),
                tool_contributors: self.tool_contributors.clone(),
                tool_lifecycle_contributors: self.tool_lifecycle_contributors.clone(),
                model_request_contributors: self.model_request_contributors.clone(),
                turn_item_contributors: self.turn_item_contributors.clone(),
                approval_review_contributors: self.approval_review_contributors.clone(),
            },
        }
    }

    /// Acquires the host's turn-start permit, or an empty permit for ungated hosts.
    /// A missing permit rejects the start before Core consumes pending input.
    pub fn admit_turn_start(&self) -> Option<Box<dyn Send>> {
        match &self.turn_start_admission {
            Some(admission) => admission.admit_turn_start(),
            None => Some(Box::new(())),
        }
    }

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

    /// Whether any installed skill contributor needs a snapshot of host-owned skills.
    ///
    /// Registries without skill contributors retain legacy host discovery behavior.
    pub fn requires_host_skill_discovery(&self) -> bool {
        self.skill_invocation_contributors.is_empty()
            || self
                .skill_invocation_contributors
                .iter()
                .any(|contributor| contributor.requires_host_skill_discovery())
    }

    /// Returns the first claimed decision in registration order.
    pub async fn decide_approval(
        &self,
        input: &crate::ApprovalDecisionInput<'_>,
    ) -> Option<crate::ApprovalDecision> {
        for contributor in &self.approval_review_contributors {
            if let Some(decision) = contributor.decide(input).await {
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

    /// Returns response interceptor contributors in registration order.
    pub fn model_request_contributors(&self) -> &[Arc<dyn ModelRequestContributor>] {
        &self.model_request_contributors
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
