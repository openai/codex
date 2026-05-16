use std::future::Future;
use std::sync::Arc;

use codex_protocol::items::TurnItem;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TokenUsageInfo;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;

use crate::ExtensionData;

mod prompt;
mod thread_lifecycle;
mod turn_lifecycle;

pub use prompt::PromptFragment;
pub use prompt::PromptSlot;
pub use thread_lifecycle::ThreadResumeInput;
pub use thread_lifecycle::ThreadStartInput;
pub use thread_lifecycle::ThreadStopInput;
pub use turn_lifecycle::TurnAbortInput;
pub use turn_lifecycle::TurnStartInput;
pub use turn_lifecycle::TurnStopInput;

/// Extension contribution that adds prompt fragments during prompt assembly.
pub trait ContextContributor: Send + Sync {
    fn contribute<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> std::pin::Pin<Box<dyn Future<Output = Vec<PromptFragment>> + Send + 'a>>;
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

/// Contributor for host-owned turn lifecycle gates.
///
/// Implementations should use these callbacks to seed, observe, or clear
/// extension-private turn state. The host exposes stable identifiers and
/// extension stores instead of core runtime objects.
pub trait TurnLifecycleContributor: Send + Sync {
    /// Called after turn-scoped extension stores are created, before the task
    /// for the turn starts running.
    fn on_turn_start(&self, _input: TurnStartInput<'_>) {}

    /// Called before the host drops the completed turn runtime and turn store.
    fn on_turn_stop(&self, _input: TurnStopInput<'_>) {}

    /// Called after the host aborts a running turn.
    fn on_turn_abort(&self, _input: TurnAbortInput<'_>) {}
}

/// Contributor for host-owned configuration changes.
///
/// Implementations should treat the supplied values as immutable before/after
/// snapshots of the effective thread configuration.
pub trait ConfigContributor<C>: Send + Sync {
    /// Called after the host commits a changed thread configuration.
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
        _previous_config: &C,
        _new_config: &C,
    ) {
    }
}

/// Contributor for token usage checkpoints reported by the model provider.
///
/// Implementations should keep this callback cheap. The host calls it after
/// updating cached token usage and before emitting the corresponding client
/// token-count notification.
pub trait TokenUsageContributor: Send + Sync {
    /// Called each time the host records token usage from a model response.
    fn on_token_usage(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
        _turn_store: &ExtensionData,
        _token_usage: &TokenUsageInfo,
    ) {
    }
}

/// Extension contribution that exposes native tools owned by a feature.
pub trait ToolContributor: Send + Sync {
    /// Returns the native tools visible for the supplied extension stores.
    fn tools(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>>;
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
