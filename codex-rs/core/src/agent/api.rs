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
use codex_rollout_trace::ThreadTraceContext;
use futures::future::BoxFuture;
use futures::stream::BoxStream;

/// An initial runtime snapshot followed by coalesced status changes. The local stream
/// ends when that runtime's status channel closes; it does not follow a later reload.
pub type StatusSubscription = BoxStream<'static, Result<AgentInfo>>;

// Keep dynamic dispatch a compile-time property of the contract.
const _: Option<&dyn AgentControl> = None;

/// Coordinates agent operations and shared state through a local or host backend.
///
/// Implementations preserve the existing wake modes and keep loading and delivery behind
/// complete operations. Successful delivery means accepted, not read by the model. The
/// local backend retains its current best-effort reporting and runtime observation policy;
/// remote ownership, retries and recovery are separate backend work.
/// Boxed Send futures allow callers to use `Arc<dyn AgentControl>`.
pub trait AgentControl: Send + Sync {
    fn identity(&self) -> SessionId;

    /// Start a child and accept its initial input, returning its effective settings.
    fn spawn(
        &self,
        request: SpawnRequest,
    ) -> BoxFuture<'_, Result<(LiveAgent, ThreadConfigSnapshot)>>;

    /// Reopen an absent runtime with the captured settings and session source. An already
    /// loaded runtime keeps its current settings. Legacy ID-based resume can reopen a
    /// stored thread that is not in the controller's registry.
    fn resume(
        &self,
        caller: ThreadId,
        target: AgentTarget,
        config: Config,
        source: SessionSource,
    ) -> BoxFuture<'_, Result<(LiveAgent, ThreadConfigSnapshot)>>;

    /// Resolve, reload if needed and accept input. Agent messages retain their attribution
    /// and wake mode: queue-only messages do not start work and follow-ups cannot target
    /// the root. Legacy user input can address loaded threads outside the agent registry.
    fn send(&self, request: SendRequest) -> BoxFuture<'_, Result<DeliveryReceipt>>;

    /// Stop current work and return the pre-interrupt snapshot. V2 rejects root/self
    /// targets and tolerates known unloaded agents; other modes retain direct-ID interruption.
    fn interrupt(
        &self,
        caller: ThreadId,
        target: AgentTarget,
        version: MultiAgentVersion,
    ) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Close the agent and its live descendants, returning its pre-close snapshot.
    fn close(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>>;

    /// Read runtime settings or known unloaded metadata without loading an agent.
    fn inspect(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>>;

    /// List loaded agents using the caller's captured source to resolve a path prefix.
    /// Callers own model and UI formatting.
    fn list<'a>(
        &'a self,
        source: &'a SessionSource,
        path_prefix: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Vec<LiveAgent>>>;

    /// Subscribe to a loaded runtime without restoring it.
    fn watch(
        &self,
        caller: ThreadId,
        target: AgentTarget,
    ) -> BoxFuture<'_, Result<StatusSubscription>>;

    /// Check capacity before accepting work. This advisory check does not reserve a slot.
    fn check_turn_admission(
        &self,
        version: MultiAgentVersion,
        source: &SessionSource,
    ) -> Result<()>;

    /// Track a turn's execution until its guard drops. Local admission keeps the existing
    /// separate capacity check and running count; it does not atomically reserve capacity.
    /// Root and non-V2 turns return no guard.
    fn admit_turn(
        &self,
        version: MultiAgentVersion,
        source: SessionSource,
    ) -> BoxFuture<'_, Result<Option<AgentExecutionGuard>>>;

    /// Account for one inference response, including compaction. Each call records usage;
    /// callers report it once. `SessionBudgetExceeded` means the usage was recorded and
    /// the shared budget is now exhausted.
    fn record_usage(&self, usage: TokenUsage) -> BoxFuture<'_, Result<()>>;

    /// Report the terminal result to the parent and completion activity to the task
    /// initiator. Local delivery remains best effort and uses the reporting runtime's
    /// diagnostic trace; it is not deduplicated.
    fn turn_finished<'a>(
        &'a self,
        outcome: AgentTurnOutcome,
        trace: &'a ThreadTraceContext,
    ) -> BoxFuture<'a, Result<()>>;

    /// Read the latest shared service tier for use at normal runtime config update points.
    fn service_tier(&self) -> Option<String>;

    /// Publish a shared setting when the runtime accepts a root-owned config update.
    fn propagate_config_update(&self, update: AgentConfigUpdate) -> BoxFuture<'_, Result<()>>;

    /// Read the existing bounded root evidence for a worker. The local backend returns
    /// `None` for the root, a non-V2 tree, or an unavailable root runtime.
    fn get_guardian_package(
        &self,
        agent: ThreadId,
    ) -> BoxFuture<'_, Result<Option<GuardianRootSnapshot>>>;

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

/// References resolve relative to the registered caller. IDs retain each operation's
/// existing lookup policy, including legacy access to unregistered loaded threads.
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

impl AgentInfo {
    pub fn metadata(&self) -> &AgentMetadata {
        match self {
            Self::Loaded { agent, .. } => &agent.metadata,
            Self::Unloaded(metadata) => metadata,
        }
    }

    pub fn status(&self) -> Option<&AgentStatus> {
        match self {
            Self::Loaded { agent, .. } => Some(&agent.status),
            Self::Unloaded(_) => None,
        }
    }
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
    /// Recipient identity captured during delivery, for activity and tool output.
    pub metadata: AgentMetadata,
    /// Acceptance identifier, not evidence that the recipient processed the input.
    pub submission_id: String,
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
