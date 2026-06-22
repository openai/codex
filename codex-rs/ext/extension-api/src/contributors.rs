use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_context_fragments::ContextualUserFragment;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TokenUsageInfo;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;

use crate::ExtensionData;

mod context;
mod mcp;
mod prompt;
mod thread_lifecycle;
mod tool_lifecycle;
mod turn_input;
mod turn_lifecycle;

pub use context::TurnContextContributionInput;
pub use mcp::McpServerContribution;
pub use mcp::McpServerContributionContext;
pub use prompt::PromptFragment;
pub use prompt::PromptSlot;
pub use thread_lifecycle::ThreadIdleInput;
pub use thread_lifecycle::ThreadResumeInput;
pub use thread_lifecycle::ThreadStartInput;
pub use thread_lifecycle::ThreadStopInput;
pub use tool_lifecycle::ToolCallOutcome;
pub use tool_lifecycle::ToolCallSource;
pub use tool_lifecycle::ToolFinishInput;
pub use tool_lifecycle::ToolLifecycleFuture;
pub use tool_lifecycle::ToolStartInput;
pub use turn_input::TurnInputContext;
pub use turn_input::TurnInputEnvironment;
pub use turn_lifecycle::TurnAbortInput;
pub use turn_lifecycle::TurnErrorInput;
pub use turn_lifecycle::TurnStartInput;
pub use turn_lifecycle::TurnStopInput;

/// Boxed, sendable future returned by asynchronous extension contributors.
pub type ExtensionFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Extension contribution that resolves runtime MCP servers from host config.
///
/// Contributors run in registration order. Later contributions for the same
/// name replace earlier ones. Implementations must contribute only names they
/// own and must apply any source-specific policy before returning a server.
/// Thread-scoped resolution exposes the host-seeded thread inputs; global
/// resolution exposes none and must not imply a local fallback. Thread inputs
/// are frozen for the runtime and do not include lifecycle-contributor state.
/// Auto-discovered plugin servers are resolved by the plugin manager. A
/// thread-selected plugin contribution must carry its own package provenance.
pub trait McpServerContributor<C: Sync>: Send + Sync {
    /// Stable identity used for registration provenance and conflict diagnostics.
    fn id(&self) -> &'static str;

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, C>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>>;
}

/// Extension contribution that adds prompt fragments during prompt assembly.
///
/// Implementations should use the method matching the scope needed by the
/// fragment: thread/session context for stable inputs, and turn context for
/// fragments that depend on turn-local host state.
pub trait ContextContributor: Send + Sync {
    fn contribute_thread_context<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(async move {
            let _self = self;
            let _session_store = session_store;
            let _thread_store = thread_store;
            Vec::new()
        })
    }

    fn contribute_turn_context<'a>(
        &'a self,
        input: TurnContextContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
            Vec::new()
        })
    }
}

/// Contributor for host-owned thread lifecycle gates.
///
/// Implementations should use these callbacks to seed, rehydrate, or flush
/// extension-private thread state. Heavy dependencies belong on the extension
/// value created by the host, not in these inputs.
pub trait ThreadLifecycleContributor<C: Sync>: Send + Sync {
    /// Called after host startup has initialized the thread-scoped store.
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }

    /// Called after the host constructs a runtime from persisted history.
    fn on_thread_resume<'a>(&'a self, input: ThreadResumeInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }

    /// Called after the host has drained immediately pending thread work.
    ///
    /// Implementations may use host capabilities captured by the extension to
    /// submit follow-up input. The host remains responsible for deciding
    /// whether that input starts a turn, is queued, or is ignored.
    fn on_thread_idle<'a>(&'a self, input: ThreadIdleInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }

    /// Called before the host drops the thread runtime and thread-scoped store.
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }
}

/// Contributor for host-owned turn lifecycle gates.
///
/// Implementations should use these callbacks to seed, observe, or clear
/// extension-private turn state. The host exposes stable identifiers and
/// extension stores instead of core runtime objects.
pub trait TurnLifecycleContributor: Send + Sync {
    /// Called after turn-scoped extension stores are created, before the task
    /// for the turn starts running.
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }

    /// Called before the host drops the completed turn runtime and turn store.
    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }

    /// Called after the host aborts a running turn.
    fn on_turn_abort<'a>(&'a self, input: TurnAbortInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }

    /// Called when the host observes an error for a running turn.
    fn on_turn_error<'a>(&'a self, input: TurnErrorInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
        })
    }
}

/// Extension contribution that can add turn-local model input.
///
/// Implementations should resolve only the model-visible input they own and
/// must preserve authority boundaries for external resources. Expensive or
/// host-specific dependencies belong on the extension value installed by the
/// host, not in this input.
pub trait TurnInputContributor: Send + Sync {
    /// Returns additional contextual fragments for one submitted turn.
    fn contribute<'a>(
        &'a self,
        input: TurnInputContext,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>>;
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
    fn on_token_usage<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        _token_usage: &'a TokenUsageInfo,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _self = self;
            let _inputs = (_session_store, _thread_store, _turn_store, _token_usage);
        })
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
pub trait ApprovalReviewContributor: Send + Sync {
    fn contribute<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        prompt: &'a str,
    ) -> ExtensionFuture<'a, Option<ReviewDecision>>;
}

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
    ) -> ExtensionFuture<'a, Result<(), String>>;
}
