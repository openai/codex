use super::SlowCommandAggregate;
use super::SlowCommandAnalysis;
use super::SlowCommandCollection;
use super::SlowCommandContinuation;
use super::SlowCommandDirectCall;
use super::SlowCommandSummary;
use super::parsing::first_executable;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;

struct Invocation {
    command: String,
    executable: String,
    continuation_wait_count: usize,
    total_wait: Duration,
}

/// Merges low-level collections, deduplicates calls globally, and ranks their commands.
pub fn merge_slow_command_collections(
    collections: impl IntoIterator<Item = SlowCommandCollection>,
) -> SlowCommandAnalysis {
    let mut summary = SlowCommandSummary::default();
    let mut direct_calls = HashMap::<String, SlowCommandDirectCall>::new();
    let mut continuations = HashMap::<String, SlowCommandContinuation>::new();

    for collection in collections {
        add_coverage(&mut summary, &collection.summary);
        for direct_call in collection.direct_calls {
            match direct_calls.entry(direct_call.call_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    summary.duplicate_call_ids = summary.duplicate_call_ids.saturating_add(1);
                    merge_direct_call(entry.get_mut(), direct_call);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(direct_call);
                }
            }
        }
        for continuation in collection.continuations {
            match continuations.entry(continuation.call_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    summary.duplicate_call_ids = summary.duplicate_call_ids.saturating_add(1);
                    merge_continuation(entry.get_mut(), continuation);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(continuation);
                }
            }
        }
    }

    let direct_calls = direct_calls.into_values().collect::<Vec<_>>();
    let continuations = continuations.into_values().collect::<Vec<_>>();
    analyze_observations(summary, &direct_calls, &continuations)
}

pub(super) fn analyze_observations(
    mut summary: SlowCommandSummary,
    direct_calls: &[SlowCommandDirectCall],
    continuations: &[SlowCommandContinuation],
) -> SlowCommandAnalysis {
    let mut continuations_by_invocation = HashMap::<&str, Vec<&SlowCommandContinuation>>::new();
    for continuation in continuations {
        if let Some(invocation_call_id) = continuation.invocation_call_id.as_deref() {
            continuations_by_invocation
                .entry(invocation_call_id)
                .or_default()
                .push(continuation);
        }
    }

    let mut invocations = Vec::new();
    let mut attributed_continuation_ids = HashSet::new();
    let mut open_processes = 0usize;
    for direct_call in direct_calls {
        let Some(command) = direct_call.command.as_ref() else {
            continue;
        };
        let mut total_wait = direct_call.wait.unwrap_or(Duration::ZERO);
        let mut wait_samples = usize::from(direct_call.wait.is_some());
        let mut continuation_wait_count = 0usize;
        let mut observed_running = direct_call.observed_running;
        let mut observed_terminal = direct_call.observed_terminal;
        for continuation in continuations_by_invocation
            .get(direct_call.call_id.as_str())
            .into_iter()
            .flatten()
        {
            observed_running |= continuation.observed_running;
            observed_terminal |= continuation.observed_terminal;
            if let Some(wait) = continuation.wait {
                total_wait = total_wait.saturating_add(wait);
                wait_samples = wait_samples.saturating_add(1);
                continuation_wait_count = continuation_wait_count.saturating_add(1);
                attributed_continuation_ids.insert(continuation.call_id.as_str());
            }
        }
        if observed_running && !observed_terminal {
            open_processes = open_processes.saturating_add(1);
        }
        if wait_samples > 0 {
            invocations.push(Invocation {
                executable: first_executable(command),
                command: command.clone(),
                continuation_wait_count,
                total_wait,
            });
        }
    }

    summary.direct_shell_calls_seen = direct_calls.len();
    summary.direct_shell_calls_analyzed = invocations.len();
    summary.direct_shell_calls_skipped = summary
        .direct_shell_calls_seen
        .saturating_sub(summary.direct_shell_calls_analyzed);
    summary.continuation_calls_seen = continuations.len();
    summary.continuation_calls_attributed = attributed_continuation_ids.len();
    summary.continuation_calls_skipped = summary
        .continuation_calls_seen
        .saturating_sub(summary.continuation_calls_attributed);
    summary.open_processes_at_eof = open_processes;
    summary.total_wait = invocations
        .iter()
        .fold(Duration::ZERO, |total, invocation| {
            total.saturating_add(invocation.total_wait)
        });

    let exact_commands =
        aggregate_invocations(&invocations, |invocation| invocation.command.as_str());
    let command_families =
        aggregate_invocations(&invocations, |invocation| invocation.executable.as_str());
    SlowCommandAnalysis {
        summary,
        exact_commands,
        command_families,
    }
}

pub(super) fn merge_optional_string(target: &mut Option<String>, incoming: Option<String>) {
    if let Some(incoming) = incoming
        && target
            .as_ref()
            .is_none_or(|current| incoming.as_str() < current.as_str())
    {
        *target = Some(incoming);
    }
}

pub(super) fn merge_optional_duration(target: &mut Option<Duration>, incoming: Option<Duration>) {
    if let Some(incoming) = incoming
        && target.is_none_or(|current| incoming > current)
    {
        *target = Some(incoming);
    }
}

fn add_coverage(target: &mut SlowCommandSummary, source: &SlowCommandSummary) {
    target.rollout_files_seen = target
        .rollout_files_seen
        .saturating_add(source.rollout_files_seen);
    target.rollout_files_analyzed = target
        .rollout_files_analyzed
        .saturating_add(source.rollout_files_analyzed);
    target.rollout_files_skipped = target
        .rollout_files_skipped
        .saturating_add(source.rollout_files_skipped);
    target.internal_rollout_files_skipped = target
        .internal_rollout_files_skipped
        .saturating_add(source.internal_rollout_files_skipped);
    target.rollout_lines_skipped = target
        .rollout_lines_skipped
        .saturating_add(source.rollout_lines_skipped);
    target.duplicate_call_ids = target
        .duplicate_call_ids
        .saturating_add(source.duplicate_call_ids);
    target.code_mode_cells_ignored = target
        .code_mode_cells_ignored
        .saturating_add(source.code_mode_cells_ignored);
}

fn merge_direct_call(target: &mut SlowCommandDirectCall, incoming: SlowCommandDirectCall) {
    merge_optional_string(&mut target.command, incoming.command);
    merge_optional_duration(&mut target.wait, incoming.wait);
    target.observed_running |= incoming.observed_running;
    target.observed_terminal |= incoming.observed_terminal;
}

fn merge_continuation(target: &mut SlowCommandContinuation, incoming: SlowCommandContinuation) {
    merge_optional_string(&mut target.invocation_call_id, incoming.invocation_call_id);
    merge_optional_duration(&mut target.wait, incoming.wait);
    target.observed_running |= incoming.observed_running;
    target.observed_terminal |= incoming.observed_terminal;
}

fn aggregate_invocations<'a>(
    invocations: &'a [Invocation],
    key: impl Fn(&'a Invocation) -> &'a str,
) -> Vec<SlowCommandAggregate> {
    let mut aggregates = HashMap::<String, SlowCommandAggregate>::new();
    for invocation in invocations {
        let aggregate = aggregates
            .entry(key(invocation).to_string())
            .or_insert_with(|| SlowCommandAggregate {
                key: key(invocation).to_string(),
                invocation_count: 0,
                continuation_wait_count: 0,
                total_wait: Duration::ZERO,
                max_wait: Duration::ZERO,
            });
        aggregate.invocation_count = aggregate.invocation_count.saturating_add(1);
        aggregate.continuation_wait_count = aggregate
            .continuation_wait_count
            .saturating_add(invocation.continuation_wait_count);
        aggregate.total_wait = aggregate.total_wait.saturating_add(invocation.total_wait);
        aggregate.max_wait = aggregate.max_wait.max(invocation.total_wait);
    }
    let mut aggregates = aggregates.into_values().collect::<Vec<_>>();
    aggregates.sort_by(|left, right| {
        right
            .total_wait
            .cmp(&left.total_wait)
            .then_with(|| right.invocation_count.cmp(&left.invocation_count))
            .then_with(|| left.key.cmp(&right.key))
    });
    aggregates
}
