//! Coordination of one agent tree, independent of where its threads run.
//!
//! The trait and its requests use shared agent types and captured settings. Live threads
//! and turn contexts stay in the runtime. Implementations own membership, loading,
//! delivery and shared resources. These Rust contracts do not define a wire protocol.

use crate::agent::types::AgentExecutionGuard;
use crate::agent::types::AgentMessage;
use crate::agent::types::AgentMetadata;
use crate::agent::types::LiveAgent;
use crate::agent::types::MessageDeliveryMode;
use crate::agent::types::SpawnAgentOptions;
use crate::codex_thread::GuardianRootSnapshot;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::config::Config;
use crate::rollout_budget::RolloutBudgetReminder;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use std::num::NonZeroU32;

/// Initial observation followed by changes, with no gap between the two. Observation failures
/// are errors, not terminal agent states. Backends own reconnection and reconciliation.
pub type StatusSubscription = BoxStream<'static, Result<AgentInfo>>;

// Keep dynamic dispatch a compile-time property of the contract.
const _: Option<&dyn AgentControl> = None;

/// Coordinates one agent tree through a local or host backend.
///
/// Every operation validates tree membership and caller authority, including calls made
/// through stale controller handles. Unknown IDs return `ThreadNotFound`; invalid or
/// unresolved references return `UnsupportedOperation`, preserving existing resolution
/// errors. Backend failures must not be reported as missing targets or successful operations.
/// Mutation success acknowledges acceptance, not completion of the requested agent work.
///
/// Implementations preserve MAv2 wake modes and keep loading, capacity checks and delivery
/// behind complete operations. Hosts own transport, durable acceptance, retry identities
/// and recovery. Those mechanisms and channel operations are separate implementation work.
/// Boxed Send futures allow callers to use `Arc<dyn AgentControl>`.
pub trait AgentControl: Send + Sync {
    fn identity(&self) -> ControlIdentity;

    /// Allocate, register and start a child, then accept its initial input. Return effective
    /// settings so callers do not need a second configuration lookup.
    fn spawn(&self, request: SpawnRequest) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Explicitly reopen an agent under the caller's authority and captured settings.
    /// Reloading required for message delivery belongs inside `send`.
    fn resume(
        &self,
        caller: ThreadId,
        target: AgentTarget,
        config: Config,
    ) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Resolve, authorize, reload if needed and accept input with its original attribution
    /// and wake mode. Queue-only messages cannot start an idle agent; follow-ups cannot
    /// target the root. Acceptance does not mean the model has read the input.
    fn send(&self, request: SendRequest) -> BoxFuture<'_, Result<DeliveryReceipt>>;

    /// Stop current work without closing the agent. Return its pre-interrupt snapshot.
    fn interrupt(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Close the agent and its descendants. Return its pre-close snapshot.
    fn close(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Read identity, runtime state and effective settings without loading a dormant agent.
    fn inspect(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Return a bounded membership page. Callers own model and UI formatting.
    fn list(&self, caller: ThreadId, query: AgentQuery) -> BoxFuture<'_, Result<AgentPage>>;

    /// Subscribe without loading a dormant agent. Unavailability must remain visible.
    fn watch(
        &self,
        caller: ThreadId,
        target: AgentTarget,
    ) -> BoxFuture<'_, Result<StatusSubscription>>;

    /// Atomically reserve tree-wide execution capacity. Unlimited turns return no guard.
    /// Cancellation during acquisition must release any partial reservation. This contract
    /// does not require changing the local policy from rejecting at capacity to waiting.
    fn admit_turn(
        &self,
        thread_id: ThreadId,
        turn_id: String,
        version: MultiAgentVersion,
        source: SessionSource,
    ) -> BoxFuture<'_, Result<Option<AgentExecutionGuard>>>;

    /// Account for an inference response, including compaction. Retried reports for the
    /// same thread, turn and response must not charge the budget twice. As in the existing
    /// rollout-budget path, `SessionBudgetExceeded` means the usage was recorded and the
    /// budget is exhausted; retries must preserve that result without charging again.
    fn record_usage(
        &self,
        thread_id: ThreadId,
        turn_id: String,
        response_id: String,
        usage: TokenUsage,
    ) -> BoxFuture<'_, Result<()>>;

    /// Accept the terminal result and own completion delivery to the parent and task
    /// initiator. Retrying the same thread/turn outcome must not duplicate delivery.
    fn turn_finished(&self, outcome: AgentTurnOutcome) -> BoxFuture<'_, Result<()>>;

    /// Publish a root-owned update for live agents and later starts/resumes. Only the root
    /// may change shared settings; runtimes read their own effective configuration.
    fn propagate_config_update(
        &self,
        caller: ThreadId,
        update: AgentConfigUpdate,
    ) -> BoxFuture<'_, Result<()>>;

    /// Read authoritative root evidence, including source, completeness and revision.
    /// Fetch failures are errors. Approval consumers must revalidate the evidence revision
    /// before accepting an approval; this read alone does not grant authorization.
    fn get_guardian_package(&self, agent: ThreadId) -> BoxFuture<'_, Result<GuardianRootSnapshot>>;

    fn pending_budget_reminder<'a>(
        &'a self,
        agent: ThreadId,
        window: &'a str,
    ) -> BoxFuture<'a, Result<Option<RolloutBudgetReminder>>>;

    /// Acknowledge only after inserting the reminder into the agent's history.
    fn mark_budget_reminder_delivered<'a>(
        &'a self,
        agent: ThreadId,
        window: &'a str,
        reminder: RolloutBudgetReminder,
    ) -> BoxFuture<'a, Result<()>>;
}

/// Persistent tree identity and its current owner generation. Clones and reconnects keep
/// the generation; a replacement owner changes it and fences the previous owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlIdentity {
    pub session_id: SessionId,
    pub generation: uuid::Uuid,
}

/// References resolve relative to the caller. IDs still require membership checks.
#[derive(Clone, Debug)]
pub enum AgentTarget {
    Id(ThreadId),
    Reference(String),
}

/// Observes existing registry metadata and runtime snapshots without loading an agent.
/// A known identity survives unloading; unloaded does not mean completed. Missing agents
/// are operation errors, not loaded snapshots with `AgentStatus::NotFound`.
#[derive(Clone, Debug)]
pub enum AgentInfo {
    Loaded {
        agent: LiveAgent,
        config: Box<ThreadConfigSnapshot>,
    },
    /// Membership is known, but no runtime is loaded. Metadata identifies the known agent.
    Unloaded(AgentMetadata),
}

/// User input starts or steers a turn; agent messages retain their sender and wake mode.
pub enum AgentInput {
    UserInput(Vec<UserInput>),
    Message {
        message: AgentMessage,
        mode: MessageDeliveryMode,
    },
}

pub struct SpawnRequest {
    pub caller: ThreadId,
    pub config: Config,
    /// Spawning starts work; message input must use `TriggerTurn`.
    pub input: AgentInput,
    pub source: SessionSource,
    pub options: SpawnAgentOptions,
}

pub struct SendRequest {
    pub caller: ThreadId,
    pub target: AgentTarget,
    /// Captured caller settings used if the recipient must be restored.
    pub resume_config: Config,
    pub input: AgentInput,
    pub start_options: TurnStartOptions,
}

pub struct DeliveryReceipt {
    pub thread_id: ThreadId,
    /// Acceptance identifier, not evidence that the recipient processed the input.
    pub submission_id: String,
}

#[derive(Clone, Debug)]
pub enum AgentScope {
    Tree,
    Children(AgentTarget),
    /// Includes the target itself.
    Subtree(AgentTarget),
}

#[derive(Clone, Copy, Debug)]
pub enum AgentVisibility {
    Known,
    Loaded,
}

pub struct AgentQuery {
    pub scope: AgentScope,
    pub visibility: AgentVisibility,
    /// Opaque backend cursor. Pages are observations, not a locked tree snapshot.
    pub cursor: Option<String>,
    /// Requested maximum; backends must also enforce their own hard page-size cap.
    pub limit: NonZeroU32,
}

pub struct AgentPage {
    pub agents: Vec<AgentInfo>,
    pub next_cursor: Option<String>,
}

pub struct AgentTurnOutcome {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub source: SessionSource,
    pub parent_turn_id: Option<String>,
    pub initiating_agent_path: Option<AgentPath>,
    pub status: AgentStatus,
}

/// Settings shared by the tree. A service tier of `None` restores the default tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentConfigUpdate {
    ServiceTier(Option<String>),
}
