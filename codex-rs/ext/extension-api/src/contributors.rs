//! Contributor traits and inputs that let extensions hook into host-owned workflows.

use std::future::Future;
use std::sync::Arc;

use codex_protocol::items::TurnItem;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsageInfo;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;

use crate::ExtensionData;

mod prompt;
mod thread_lifecycle;
mod tool_lifecycle;
mod turn_lifecycle;

pub use prompt::PromptFragment;
pub use prompt::PromptSlot;
pub use thread_lifecycle::IdleTurnPolicy;
pub use thread_lifecycle::ThreadIdleInput;
pub use thread_lifecycle::ThreadIdleRequest;
pub use thread_lifecycle::ThreadIdleTurnStartInput;
pub use thread_lifecycle::ThreadResumeInput;
pub use thread_lifecycle::ThreadStartInput;
pub use thread_lifecycle::ThreadStopInput;
pub use tool_lifecycle::ToolCallOutcome;
pub use tool_lifecycle::ToolCallSource;
pub use tool_lifecycle::ToolFinishInput;
pub use tool_lifecycle::ToolLifecycleFuture;
pub use tool_lifecycle::ToolStartInput;
pub use turn_lifecycle::TurnAbortInput;
pub use turn_lifecycle::TurnErrorInput;
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
#[async_trait::async_trait]
pub trait ThreadLifecycleContributor<C: Sync>: Send + Sync {
    /// Called after thread-scoped extension stores are created, before later
    /// contributors can read from them.
    async fn on_thread_start(&self, _input: ThreadStartInput<'_, C>) {}

    /// Called after the host constructs a runtime from persisted history.
    async fn on_thread_resume(&self, _input: ThreadResumeInput<'_>) {}

    /// Called after the host has drained immediately pending thread work.
    ///
    /// Implementations may use host capabilities captured by the extension to
    /// submit follow-up input. The host remains responsible for deciding
    /// whether that input starts a turn, is queued, or is ignored.
    async fn on_thread_idle(&self, _input: ThreadIdleInput<'_>) {}

    /// Called before the host drops the thread runtime and thread-scoped store.
    async fn on_thread_stop(&self, _input: ThreadStopInput<'_>) {}
}

/// Future returned while requesting an extension-owned idle turn.
pub type ThreadIdleTurnRequestFuture<'a> =
    std::pin::Pin<Box<dyn Future<Output = Option<ThreadIdleRequest>> + Send + 'a>>;

/// Future returned while validating an extension-owned idle turn.
pub type ThreadIdleTurnStartFuture<'a> = std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/// Contributor that can request a host-owned turn while a thread is idle.
///
/// Implementations should normally return hidden context. Raw items are
/// available for extensions that intentionally need unwrapped input. The host
/// owns hidden-context wrapping, enforces the declared idle-turn policy before
/// calling into the contributor, and validates returned requests immediately
/// before starting a turn.
pub trait ThreadIdleTurnContributor: Send + Sync {
    /// Returns the host scheduling policy for this contributor's idle turns.
    fn idle_turn_policy(&self) -> IdleTurnPolicy {
        IdleTurnPolicy::default()
    }

    /// Returns input to start an idle turn, if the extension still has work
    /// that should run without user input.
    ///
    /// The host calls this at most once per idle scheduling attempt. A returned
    /// request can still be discarded if user work wins or validation fails, so
    /// implementations should not treat this as a start notification.
    fn request_thread_idle_turn<'a>(
        &'a self,
        _input: ThreadIdleInput<'a>,
    ) -> ThreadIdleTurnRequestFuture<'a> {
        Box::pin(std::future::ready(None))
    }

    /// Confirms that the idle-turn request is still current immediately before
    /// the host starts the turn.
    fn should_start_thread_idle_turn<'a>(
        &'a self,
        _input: ThreadIdleTurnStartInput<'a>,
    ) -> ThreadIdleTurnStartFuture<'a> {
        Box::pin(std::future::ready(true))
    }
}

/// Contributor for host-owned turn lifecycle gates.
///
/// Implementations should use these callbacks to seed, observe, or clear
/// extension-private turn state. The host exposes stable identifiers and
/// extension stores instead of core runtime objects.
#[async_trait::async_trait]
pub trait TurnLifecycleContributor: Send + Sync {
    /// Called after turn-scoped extension stores are created, before the task
    /// for the turn starts running.
    async fn on_turn_start(&self, _input: TurnStartInput<'_>) {}

    /// Called before the host drops the completed turn runtime and turn store.
    async fn on_turn_stop(&self, _input: TurnStopInput<'_>) {}

    /// Called after the host aborts a running turn.
    async fn on_turn_abort(&self, _input: TurnAbortInput<'_>) {}

    /// Called when a running turn encounters a host-classified error before it
    /// can complete normally.
    async fn on_turn_error(&self, _input: TurnErrorInput<'_>) {}
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
#[async_trait::async_trait]
pub trait TokenUsageContributor: Send + Sync {
    /// Called each time the host records token usage from a model response.
    async fn on_token_usage(
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

    /// Returns the native tools visible for the current turn, when the host has
    /// turn context available.
    fn tools_for_turn(
        &self,
        input: ToolContributionInput<'_>,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        self.tools(input.session_store, input.thread_store)
    }
}

/// Context supplied while collecting extension-owned tools for a turn.
pub struct ToolContributionInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Source for the current turn's session.
    pub session_source: &'a SessionSource,
    /// Whether the current thread has persistent state available.
    pub persistent_thread: bool,
}

/// Contributor for host-owned tool lifecycle gates.
///
/// Implementations should use these callbacks to observe tool execution without
/// inspecting or rewriting tool input/output. Use `ToolContributor` for owning a
/// tool implementation and hooks for policy that needs tool payloads.
pub trait ToolLifecycleContributor: Send + Sync {
    /// Called once the host has accepted a tool call for execution.
    fn on_tool_start<'a>(&'a self, _input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(std::future::ready(()))
    }

    /// Called after the tool call returns, is blocked, fails, or is cancelled.
    fn on_tool_finish<'a>(&'a self, _input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(std::future::ready(()))
    }
}

/// Extension contribution that can claim rendered approval-review prompts.
#[async_trait::async_trait]
pub trait ApprovalReviewContributor: Send + Sync {
    async fn contribute(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        prompt: &str,
    ) -> Option<ReviewDecision>;
}

/// Ordered post-processing contribution for one parsed turn item.
///
/// Implementations may mutate the item before it is emitted and may use the
/// explicitly exposed thread- and turn-lifetime stores when they need durable
/// extension-private state.
#[async_trait::async_trait]
pub trait TurnItemContributor: Send + Sync {
    async fn contribute(
        &self,
        thread_store: &ExtensionData,
        turn_store: &ExtensionData,
        item: &mut TurnItem,
    ) -> Result<(), String>;
}
