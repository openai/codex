use crate::ARCHIVED_SESSIONS_SUBDIR;
use crate::SESSIONS_SUBDIR;
use crate::compression;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionSource;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

mod merge;
mod parsing;

pub use merge::merge_slow_command_collections;

use merge::analyze_observations;
use merge::merge_optional_duration;
use merge::merge_optional_string;
use parsing::parse_direct_command;
use parsing::parse_running_session_id;
use parsing::parse_session_id;
use parsing::parse_wall_time;

/// Coverage and accounting data for a slow-command analysis.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlowCommandSummary {
    pub rollout_files_seen: usize,
    pub rollout_files_analyzed: usize,
    pub rollout_files_skipped: usize,
    pub internal_rollout_files_skipped: usize,
    pub rollout_lines_skipped: usize,
    pub direct_shell_calls_seen: usize,
    pub direct_shell_calls_analyzed: usize,
    pub direct_shell_calls_skipped: usize,
    pub continuation_calls_seen: usize,
    pub continuation_calls_attributed: usize,
    pub continuation_calls_skipped: usize,
    pub duplicate_call_ids: usize,
    pub open_processes_at_eof: usize,
    pub code_mode_cells_ignored: usize,
    pub total_wait: Duration,
}

/// Aggregate wait-time statistics for one exact command or executable family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlowCommandAggregate {
    pub key: String,
    pub invocation_count: usize,
    pub continuation_wait_count: usize,
    pub total_wait: Duration,
    pub max_wait: Duration,
}

impl SlowCommandAggregate {
    pub fn average_wait(&self) -> Duration {
        if self.invocation_count == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.total_wait.as_secs_f64() / self.invocation_count as f64)
    }
}

/// Slow-command analysis over all eligible rollout files in a Codex home.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlowCommandAnalysis {
    pub summary: SlowCommandSummary,
    pub exact_commands: Vec<SlowCommandAggregate>,
    pub command_families: Vec<SlowCommandAggregate>,
}

/// Mergeable low-level observations collected from one Codex home.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlowCommandCollection {
    pub summary: SlowCommandSummary,
    pub direct_calls: Vec<SlowCommandDirectCall>,
    pub continuations: Vec<SlowCommandContinuation>,
}

/// One observed direct `shell_command` or `exec_command` call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlowCommandDirectCall {
    pub call_id: String,
    pub command: Option<String>,
    pub wait: Option<Duration>,
    pub observed_running: bool,
    pub observed_terminal: bool,
}

/// One observed `write_stdin` call and its optional originating command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlowCommandContinuation {
    pub call_id: String,
    pub invocation_call_id: Option<String>,
    pub wait: Option<Duration>,
    pub observed_running: bool,
    pub observed_terminal: bool,
}

enum PendingCall {
    Direct,
    Continuation { session_id: Option<i64> },
}

#[derive(Default)]
struct FileAnalysisState {
    pending_calls: HashMap<String, PendingCall>,
    active_processes: HashMap<i64, String>,
}

#[derive(Default)]
struct AnalysisState {
    summary: SlowCommandSummary,
    direct_calls: HashMap<String, SlowCommandDirectCall>,
    continuations: HashMap<String, SlowCommandContinuation>,
    relevant_call_ids: HashSet<String>,
    code_mode_call_ids: HashSet<String>,
}

/// Analyzes direct agent shell calls in active and archived rollout history.
pub async fn analyze_slow_commands(codex_home: &Path) -> SlowCommandAnalysis {
    merge_slow_command_collections([collect_slow_commands(codex_home).await])
}

/// Collects mergeable direct-call and continuation observations from a Codex home.
pub async fn collect_slow_commands(codex_home: &Path) -> SlowCommandCollection {
    let mut state = AnalysisState::default();
    let paths = discover_rollout_paths(codex_home, &mut state.summary).await;
    state.summary.rollout_files_seen = paths.len();

    for path in paths {
        analyze_rollout(&path, &mut state).await;
    }

    finish_collection(state)
}

async fn discover_rollout_paths(
    codex_home: &Path,
    summary: &mut SlowCommandSummary,
) -> Vec<PathBuf> {
    let mut dirs = vec![
        codex_home.join(SESSIONS_SUBDIR),
        codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
    ];
    let mut files = BTreeMap::<PathBuf, PathBuf>::new();

    while let Some(dir) = dirs.pop() {
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                summary.rollout_files_skipped = summary.rollout_files_skipped.saturating_add(1);
                continue;
            }
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(_) => {
                    summary.rollout_files_skipped = summary.rollout_files_skipped.saturating_add(1);
                    break;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(file_type) => file_type,
                Err(_) => {
                    summary.rollout_files_skipped = summary.rollout_files_skipped.saturating_add(1);
                    continue;
                }
            };
            if file_type.is_dir() {
                dirs.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(rollout_file) = compression::RolloutFile::from_path(path) else {
                continue;
            };
            let canonical_path = compression::plain_rollout_path(rollout_file.path());
            files.insert(canonical_path, rollout_file.into_path());
        }
    }

    files.into_values().collect()
}

async fn analyze_rollout(path: &Path, state: &mut AnalysisState) {
    let mut reader = match compression::open_rollout_line_reader(path).await {
        Ok(reader) => reader,
        Err(_) => {
            state.summary.rollout_files_skipped =
                state.summary.rollout_files_skipped.saturating_add(1);
            return;
        }
    };
    let mut file_state = FileAnalysisState::default();
    let mut saw_session_meta = false;

    loop {
        let line = match reader.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(_) => {
                state.summary.rollout_files_skipped =
                    state.summary.rollout_files_skipped.saturating_add(1);
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let rollout_line = match serde_json::from_str::<RolloutLine>(&line) {
            Ok(rollout_line) => rollout_line,
            Err(_) => {
                state.summary.rollout_lines_skipped =
                    state.summary.rollout_lines_skipped.saturating_add(1);
                continue;
            }
        };
        match rollout_line.item {
            RolloutItem::SessionMeta(meta) if !saw_session_meta => {
                saw_session_meta = true;
                if matches!(meta.meta.source, SessionSource::Internal(_)) {
                    state.summary.internal_rollout_files_skipped = state
                        .summary
                        .internal_rollout_files_skipped
                        .saturating_add(1);
                    return;
                }
            }
            RolloutItem::ResponseItem(item) => {
                analyze_response_item(item, &mut file_state, state);
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::InterAgentCommunication(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::EventMsg(_) => {}
        }
    }

    state.summary.rollout_files_analyzed = state.summary.rollout_files_analyzed.saturating_add(1);
}

fn analyze_response_item(
    item: ResponseItem,
    file_state: &mut FileAnalysisState,
    state: &mut AnalysisState,
) {
    match item {
        ResponseItem::FunctionCall {
            name,
            namespace,
            arguments,
            call_id,
            ..
        } if namespace.is_none() => match name.as_str() {
            "shell_command" | "exec_command" => {
                record_relevant_call_id(&call_id, state);
                let command = parse_direct_command(&name, &arguments);
                let direct_call = state
                    .direct_calls
                    .entry(call_id.clone())
                    .or_insert_with(|| SlowCommandDirectCall {
                        call_id: call_id.clone(),
                        command: None,
                        wait: None,
                        observed_running: false,
                        observed_terminal: false,
                    });
                merge_optional_string(&mut direct_call.command, command);
                file_state
                    .pending_calls
                    .insert(call_id, PendingCall::Direct);
            }
            "write_stdin" => {
                record_relevant_call_id(&call_id, state);
                state
                    .continuations
                    .entry(call_id.clone())
                    .or_insert_with(|| SlowCommandContinuation {
                        call_id: call_id.clone(),
                        invocation_call_id: None,
                        wait: None,
                        observed_running: false,
                        observed_terminal: false,
                    });
                file_state.pending_calls.insert(
                    call_id,
                    PendingCall::Continuation {
                        session_id: parse_session_id(&arguments),
                    },
                );
            }
            _ => {}
        },
        ResponseItem::FunctionCallOutput {
            call_id, output, ..
        } => {
            let Some(pending) = file_state.pending_calls.remove(&call_id) else {
                return;
            };
            let text = output.body.to_text().unwrap_or_default();
            match pending {
                PendingCall::Direct => {
                    record_direct_output(&call_id, &text, file_state, state);
                }
                PendingCall::Continuation { session_id } => {
                    record_continuation_output(&call_id, session_id, &text, file_state, state);
                }
            }
        }
        ResponseItem::CustomToolCall { call_id, name, .. } if name == "exec" => {
            state.code_mode_call_ids.insert(call_id);
        }
        ResponseItem::Message { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => {}
    }
}

fn record_relevant_call_id(call_id: &str, state: &mut AnalysisState) {
    if !state.relevant_call_ids.insert(call_id.to_string()) {
        state.summary.duplicate_call_ids = state.summary.duplicate_call_ids.saturating_add(1);
    }
}

fn record_direct_output(
    call_id: &str,
    output: &str,
    file_state: &mut FileAnalysisState,
    state: &mut AnalysisState,
) {
    let Some(direct_call) = state.direct_calls.get_mut(call_id) else {
        return;
    };

    if let Some(duration) = parse_wall_time(output) {
        merge_optional_duration(&mut direct_call.wait, Some(duration));
    }

    if let Some(session_id) = parse_running_session_id(output) {
        direct_call.observed_running = true;
        file_state
            .active_processes
            .insert(session_id, call_id.to_string());
    } else {
        direct_call.observed_terminal = true;
    }
}

fn record_continuation_output(
    call_id: &str,
    session_id: Option<i64>,
    output: &str,
    file_state: &mut FileAnalysisState,
    state: &mut AnalysisState,
) {
    let Some(continuation) = state.continuations.get_mut(call_id) else {
        return;
    };

    if let Some(duration) = parse_wall_time(output) {
        merge_optional_duration(&mut continuation.wait, Some(duration));
    }
    let observed_running = parse_running_session_id(output).is_some();
    if observed_running {
        continuation.observed_running = true;
    } else {
        continuation.observed_terminal = true;
    }

    let Some(session_id) = session_id else {
        return;
    };
    let Some(invocation_id) = file_state.active_processes.get(&session_id).cloned() else {
        return;
    };
    merge_optional_string(&mut continuation.invocation_call_id, Some(invocation_id));
    if !observed_running {
        file_state.active_processes.remove(&session_id);
    }
}

fn finish_collection(mut state: AnalysisState) -> SlowCommandCollection {
    let mut direct_calls = state.direct_calls.into_values().collect::<Vec<_>>();
    direct_calls.sort_by(|left, right| left.call_id.cmp(&right.call_id));
    let mut continuations = state.continuations.into_values().collect::<Vec<_>>();
    continuations.sort_by(|left, right| left.call_id.cmp(&right.call_id));
    state.summary.code_mode_cells_ignored = state.code_mode_call_ids.len();
    state.summary = analyze_observations(state.summary, &direct_calls, &continuations).summary;
    SlowCommandCollection {
        summary: state.summary,
        direct_calls,
        continuations,
    }
}

#[cfg(test)]
#[path = "slow_commands_tests.rs"]
mod tests;
