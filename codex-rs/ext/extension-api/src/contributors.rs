use std::future::Future;
use std::sync::Arc;

use codex_protocol::items::TurnItem;
use codex_protocol::protocol::ReviewDecision;

use crate::ExtensionData;

mod prompt;
mod thread_lifecycle;
mod tools;

pub use prompt::PromptFragment;
pub use prompt::PromptSlot;
pub use thread_lifecycle::ThreadResumeInput;
pub use thread_lifecycle::ThreadStartInput;
pub use thread_lifecycle::ThreadStopInput;
pub use tools::ExtensionToolExecutor;
pub use tools::ExtensionToolFuture;
pub use tools::ExtensionToolOutput;

/// Extension contribution that adds prompt fragments during prompt assembly.
pub trait ContextContributor: Send + Sync {
    fn contribute(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<PromptFragment>;
}

/// Contributor for host-owned thread lifecycle gates.
///
/// Implementations should use these callbacks to seed, rehydrate, or flush
/// extension-private thread state. Heavy dependencies belong on the extension
/// value created by the host, not in these inputs.
pub trait ThreadLifecycleContributor<C>: Send + Sync {
    /// Called after thread-scoped extension stores are created, before later
    /// contributors can read from them.
    fn on_thread_start(&self, _input: ThreadStartInput<'_, C>) {}

    /// Called after the host constructs a runtime from persisted history.
    fn on_thread_resume(&self, _input: ThreadResumeInput<'_>) {}

    /// Called before the host drops the thread runtime and thread-scoped store.
    fn on_thread_stop(&self, _input: ThreadStopInput<'_>) {}
}

/// Extension contribution that exposes native tools owned by a feature.
pub trait ToolContributor: Send + Sync {
    /// Returns the native tools visible for the supplied extension stores.
    fn tools(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ExtensionToolExecutor>>;
}

/// Future returned by one claimed approval-review contribution.
pub type ApprovalReviewFuture<'a> =
    std::pin::Pin<Box<dyn Future<Output = ReviewDecision> + Send + 'a>>;

/// Extension contribution that can claim rendered approval-review prompts.
pub trait ApprovalReviewContributor: Send + Sync {
    fn contribute<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        prompt: &'a str,
    ) -> Option<ApprovalReviewFuture<'a>>;
}

/// Future returned by one ordered turn-item contribution.
pub type TurnItemContributionFuture<'a> =
    std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

/// Ordered post-processing contribution for one parsed turn item.
///
/// Implementations may mutate the item before it is emitted and may use the
/// explicitly exposed thread- and turn-lifetime stores when they need durable
/// extension-private state.
pub trait TurnItemContributor: Send + Sync {
    fn contribute<'a>(
        &'a self,
        thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> TurnItemContributionFuture<'a>;
}
