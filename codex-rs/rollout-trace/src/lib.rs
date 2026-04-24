//! Trace bundle format, writer, and reducer for Codex rollouts.
//!
//! This crate owns the trace schema. Hot-path Codex code should depend on the
//! small writer API here; semantic replay and viewer projections stay outside
//! `codex-core`.
//!
//! See `README.md` for the system diagram and reducer model.

mod bundle;
mod code_cell;
mod compaction;
mod inference;
mod model;
mod payload;
mod protocol_event;
mod raw_event;
mod reducer;
mod thread;
mod tool_dispatch;
mod writer;

/// Conventional reduced-state cache name written next to a raw trace bundle.
pub use bundle::REDUCED_STATE_FILE_NAME;
/// No-op-capable handle for recording one code-mode runtime cell.
pub use code_cell::CodeCellTraceContext;
/// Raw checkpoint payload for a remote compaction install event.
pub use compaction::CompactionCheckpointTracePayload;
/// No-op-capable handle for recording remote-compaction requests.
pub use compaction::CompactionTraceAttempt;
/// Shared recorder context for a compaction checkpoint.
pub use compaction::CompactionTraceContext;
/// No-op-capable handle for recording one upstream inference attempt.
pub use inference::InferenceTraceAttempt;
/// Shared recorder context for inference attempts within one Codex turn.
pub use inference::InferenceTraceContext;
/// Public reduced trace model returned by replay.
pub use model::*;
/// Stable identifier for one raw payload inside a rollout bundle.
pub use payload::RawPayloadId;
/// Coarse role labels for raw payload files.
pub use payload::RawPayloadKind;
/// Reference to a raw request/response/log payload stored in the bundle.
pub use payload::RawPayloadRef;
/// Monotonic sequence number assigned by the raw trace writer.
pub use raw_event::RawEventSeq;
/// Runtime requester observed before semantic reduction.
pub use raw_event::RawToolCallRequester;
/// One append-only raw trace event from `trace.jsonl`.
pub use raw_event::RawTraceEvent;
/// Event-envelope context supplied by hot-path trace producers.
pub use raw_event::RawTraceEventContext;
/// Typed payload for one raw trace event.
pub use raw_event::RawTraceEventPayload;
/// Replay a raw trace bundle and write/read its reduced `RolloutTrace`.
pub use reducer::replay_bundle;
/// Raw payload captured when a child agent reports completion to its parent.
pub use thread::AgentResultTracePayload;
/// Environment variable that enables local trace-bundle recording.
pub use thread::CODEX_ROLLOUT_TRACE_ROOT_ENV;
/// Raw metadata captured when a thread starts.
pub use thread::ThreadStartedTraceMetadata;
/// No-op-capable handle for recording one thread in a rollout bundle.
pub use thread::ThreadTraceContext;
/// Request data for the canonical Codex tool boundary.
pub use tool_dispatch::ToolDispatchInvocation;
/// Tool input observed at the registry boundary.
pub use tool_dispatch::ToolDispatchPayload;
/// Runtime source that caused a dispatch-level tool call.
pub use tool_dispatch::ToolDispatchRequester;
/// Result data returned from a dispatch-level tool call.
pub use tool_dispatch::ToolDispatchResult;
/// No-op-capable handle for recording one resolved tool dispatch.
pub use tool_dispatch::ToolDispatchTraceContext;
/// Append-only writer used by hot-path Codex instrumentation.
pub use writer::TraceWriter;
