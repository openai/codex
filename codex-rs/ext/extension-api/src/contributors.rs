use std::future::Future;

use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::ReviewDecision;
use codex_tool_api::ToolBundle;

use crate::ExtensionData;

mod prompt;

pub use prompt::PromptFragment;
pub use prompt::PromptSlot;

/// Contributor that receives the live thread id and host-owned thread-start
/// input before later contributors read from extension stores.
pub trait ThreadStartContributor<C>: Send + Sync {
    fn contribute(
        &self,
        thread_id: ThreadId,
        input: &C,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    );
}

/// Extension contribution that adds prompt fragments during prompt assembly.
pub trait ContextContributor: Send + Sync {
    fn contribute(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<PromptFragment>;
}

/// Extension contribution that exposes native tools owned by a feature.
pub trait ToolContributor: Send + Sync {
    /// Returns the native tools visible for the supplied extension stores.
    fn tools(&self, session_store: &ExtensionData, thread_store: &ExtensionData)
    -> Vec<ToolBundle>;
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
