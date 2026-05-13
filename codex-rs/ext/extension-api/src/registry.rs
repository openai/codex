use std::sync::Arc;

use crate::ApprovalReviewContributor;
use crate::ApprovalReviewFuture;
use crate::ConfigContributor;
use crate::ContextContributor;
use crate::ExtensionData;
use crate::ThreadLifecycleContributor;
use crate::TokenUsageContributor;
use crate::ToolContributor;
use crate::TurnItemContributor;
use crate::TurnLifecycleContributor;

/// Mutable registry used while hosts register typed runtime contributions.
pub struct ExtensionRegistryBuilder<C> {
    thread_lifecycle_contributors: Vec<Arc<dyn ThreadLifecycleContributor<C>>>,
    turn_lifecycle_contributors: Vec<Arc<dyn TurnLifecycleContributor>>,
    config_contributors: Vec<Arc<dyn ConfigContributor<C>>>,
    token_usage_contributors: Vec<Arc<dyn TokenUsageContributor>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C> Default for ExtensionRegistryBuilder<C> {
    fn default() -> Self {
        Self {
            thread_lifecycle_contributors: Vec::new(),
            turn_lifecycle_contributors: Vec::new(),
            config_contributors: Vec::new(),
            token_usage_contributors: Vec::new(),
            approval_review_contributors: Vec::new(),
            context_contributors: Vec::new(),
            tool_contributors: Vec::new(),
            turn_item_contributors: Vec::new(),
        }
    }
}

impl<C> ExtensionRegistryBuilder<C> {
    /// Creates an empty registry builder.
    pub fn new() -> Self {
        Self::default()
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

    /// Registers one native tool contributor.
    pub fn tool_contributor(&mut self, contributor: Arc<dyn ToolContributor>) {
        self.tool_contributors.push(contributor);
    }

    /// Registers one ordered turn-item contributor.
    pub fn turn_item_contributor(&mut self, contributor: Arc<dyn TurnItemContributor>) {
        self.turn_item_contributors.push(contributor);
    }

    /// Finishes construction and returns the immutable registry.
    pub fn build(self) -> ExtensionRegistry<C> {
        ExtensionRegistry {
            thread_lifecycle_contributors: self.thread_lifecycle_contributors,
            turn_lifecycle_contributors: self.turn_lifecycle_contributors,
            config_contributors: self.config_contributors,
            token_usage_contributors: self.token_usage_contributors,
            approval_review_contributors: self.approval_review_contributors,
            context_contributors: self.context_contributors,
            tool_contributors: self.tool_contributors,
            turn_item_contributors: self.turn_item_contributors,
        }
    }
}

/// Immutable typed registry produced after extensions are installed.
pub struct ExtensionRegistry<C> {
    thread_lifecycle_contributors: Vec<Arc<dyn ThreadLifecycleContributor<C>>>,
    turn_lifecycle_contributors: Vec<Arc<dyn TurnLifecycleContributor>>,
    config_contributors: Vec<Arc<dyn ConfigContributor<C>>>,
    token_usage_contributors: Vec<Arc<dyn TokenUsageContributor>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C> ExtensionRegistry<C> {
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
    pub fn approval_review<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        prompt: &'a str,
    ) -> Option<ApprovalReviewFuture<'a>> {
        self.approval_review_contributors
            .iter()
            .find_map(|contributor| contributor.contribute(session_store, thread_store, prompt))
    }

    /// Returns the registered prompt contributors.
    pub fn context_contributors(&self) -> &[Arc<dyn ContextContributor>] {
        &self.context_contributors
    }

    /// Returns the registered native tool contributors.
    pub fn tool_contributors(&self) -> &[Arc<dyn ToolContributor>] {
        &self.tool_contributors
    }

    /// Returns the registered ordered turn-item contributors.
    pub fn turn_item_contributors(&self) -> &[Arc<dyn TurnItemContributor>] {
        &self.turn_item_contributors
    }
}

/// Creates an empty shared registry for hosts that do not register contributions.
pub fn empty_extension_registry<C>() -> Arc<ExtensionRegistry<C>> {
    Arc::new(ExtensionRegistryBuilder::new().build())
}
