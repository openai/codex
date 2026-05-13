use std::sync::Arc;

use crate::ApprovalReviewContributor;
use crate::ApprovalReviewFuture;
use crate::ContextContributor;
use crate::ExtensionData;
use crate::ThreadStartContributor;
use crate::ToolContributor;
use crate::TurnItemContributor;

/// Mutable registry used while hosts register typed runtime contributions.
pub struct ExtensionRegistryBuilder<C> {
    thread_start_contributors: Vec<Arc<dyn ThreadStartContributor<C>>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C> Default for ExtensionRegistryBuilder<C> {
    fn default() -> Self {
        Self {
            thread_start_contributors: Vec::new(),
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

    /// Registers one thread-start contributor.
    pub fn thread_start_contributor(&mut self, contributor: Arc<dyn ThreadStartContributor<C>>) {
        self.thread_start_contributors.push(contributor);
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
            thread_start_contributors: self.thread_start_contributors,
            approval_review_contributors: self.approval_review_contributors,
            context_contributors: self.context_contributors,
            tool_contributors: self.tool_contributors,
            turn_item_contributors: self.turn_item_contributors,
        }
    }
}

/// Immutable typed registry produced after extensions are installed.
pub struct ExtensionRegistry<C> {
    thread_start_contributors: Vec<Arc<dyn ThreadStartContributor<C>>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_review_contributors: Vec<Arc<dyn ApprovalReviewContributor>>,
}

impl<C> ExtensionRegistry<C> {
    /// Returns the registered thread-start contributors.
    pub fn thread_start_contributors(&self) -> &[Arc<dyn ThreadStartContributor<C>>] {
        &self.thread_start_contributors
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
