//! Thread lifecycle observation event definitions.

use crate::Observation;
use serde::Serialize;

/// How a thread became active in the runtime.
///
/// This describes the open operation, not the long-term origin of the thread.
/// A resumed thread already existed; it still becomes active again for the
/// runtime or client connection handling the resume.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadInitializationMode {
    New,
    Forked,
    Resumed,
}

/// Subagent work that caused a thread to start.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadSubagentKind<'a> {
    Review,
    Compact,
    ThreadSpawn,
    MemoryConsolidation,
    Other(&'a str),
}

/// Origin of the request that made a thread active.
///
/// Keep this separate from `ThreadInitializationMode`: source answers who or
/// what opened the thread, while initialization mode answers whether the
/// thread was new, forked, or resumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadSource<'a> {
    User,
    AppServer,
    Custom(&'a str),
    Subagent(ThreadSubagentKind<'a>),
    Unknown,
}

/// Observation emitted when Codex starts tracking a thread.
///
/// "Started" means the thread became active for this runtime or client
/// connection. It does not imply the thread was newly created; see
/// `initialization_mode` for new, forked, and resumed activations.
#[derive(Observation)]
#[observation(name = "thread.started", crate = "crate", uses = ["analytics"])]
pub struct ThreadStarted<'a> {
    /// Thread that became active.
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    pub source: ThreadSource<'a>,

    /// Parent thread that created this thread, when the source represents subagent work.
    ///
    /// This stays top-level instead of being nested inside the source enum so
    /// sinks can apply identifier policy directly to the field.
    #[obs(level = "basic", class = "identifier")]
    pub parent_thread_id: Option<&'a str>,

    #[obs(level = "basic", class = "operational")]
    pub initialization_mode: ThreadInitializationMode,

    /// Model associated with the thread at activation time.
    ///
    /// Turn configuration also records a model because turns may later run with
    /// overrides or migrated settings. This field is thread lifecycle metadata.
    #[obs(level = "basic", class = "operational")]
    pub model: &'a str,

    /// Whether the thread is persisted beyond the active runtime session.
    ///
    /// Turn configuration repeats this for legacy analytics compatibility, but
    /// the stable owner of the value is the thread lifecycle.
    #[obs(level = "basic", class = "operational")]
    pub ephemeral: bool,

    /// Unix timestamp in seconds when the thread was originally created.
    ///
    /// For resumed threads this is historical creation time, not resume time.
    #[obs(level = "basic", class = "operational")]
    pub created_at: i64,
}
