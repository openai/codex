use std::sync::Arc;

use crate::ApprovalInterceptorContributor;
use crate::ContextContributor;
use crate::ThreadStartContributor;
use crate::ToolContributor;
use crate::TurnItemContributor;

/// Mutable registry used while hosts register typed runtime contributions.
pub struct ExtensionRegistryBuilder<C> {
    thread_start_contributors: Vec<Arc<dyn ThreadStartContributor<C>>>,
    context_contributors: Vec<Arc<dyn ContextContributor>>,
    tool_contributors: Vec<Arc<dyn ToolContributor>>,
    turn_item_contributors: Vec<Arc<dyn TurnItemContributor>>,
    approval_interceptor_contributors: Vec<Arc<dyn ApprovalInterceptorContributor>>,
}

impl<C> Default for ExtensionRegistryBuilder<C> {
    fn default() -> Self {
        Self {
            thread_start_contributors: Vec::new(),
            approval_interceptor_contributors: Vec::new(),
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

    /// Registers one approval interceptor contributor.
    pub fn approval_interceptor_contributor(
        &mut self,
        contributor: Arc<dyn ApprovalInterceptorContributor>,
    ) {
        self.approval_interceptor_contributors.push(contributor);
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
            approval_interceptor_contributors: self.approval_interceptor_contributors,
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
    approval_interceptor_contributors: Vec<Arc<dyn ApprovalInterceptorContributor>>,
}

impl<C> ExtensionRegistry<C> {
    /// Returns the registered thread-start contributors.
    pub fn thread_start_contributors(&self) -> &[Arc<dyn ThreadStartContributor<C>>] {
        &self.thread_start_contributors
    }

    /// Returns the registered approval interceptor contributors.
    pub fn approval_interceptor_contributors(&self) -> &[Arc<dyn ApprovalInterceptorContributor>] {
        &self.approval_interceptor_contributors
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
