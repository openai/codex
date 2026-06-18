use anyhow::Context;
use anyhow::Result;
use clap::Args;
use clap::Subcommand;
use codex_core::config::find_codex_home;
use codex_rollout::SlowCommandAggregate;
use codex_rollout::SlowCommandCollection;
use codex_rollout::SlowCommandContinuation;
use codex_rollout::SlowCommandDirectCall;
use codex_rollout::SlowCommandSummary;
use codex_rollout::analyze_slow_commands;
use codex_rollout::collect_slow_commands;
use codex_rollout::merge_slow_command_collections;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::io::BufRead;
use std::io::BufWriter;
use std::io::Write;
use std::time::Duration;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Args)]
pub(crate) struct SlowCommandsCommand {
    #[command(subcommand)]
    action: Option<SlowCommandsAction>,
}

#[derive(Debug, Subcommand)]
enum SlowCommandsAction {
    /// Emit mergeable low-level command observations as JSONL.
    Collect,
    /// Read concatenated collection JSONL from stdin and emit one analysis.
    Merge,
}

#[derive(Deserialize, Serialize)]
struct SummaryRecord {
    #[serde(rename = "type")]
    record_type: String,
    schema_version: u32,
    rollout_files_seen: usize,
    rollout_files_analyzed: usize,
    rollout_files_skipped: usize,
    internal_rollout_files_skipped: usize,
    rollout_lines_skipped: usize,
    direct_shell_calls_seen: usize,
    direct_shell_calls_analyzed: usize,
    direct_shell_calls_skipped: usize,
    continuation_calls_seen: usize,
    continuation_calls_attributed: usize,
    continuation_calls_skipped: usize,
    duplicate_call_ids: usize,
    open_processes_at_eof: usize,
    code_mode_cells_ignored: usize,
    total_wait_seconds: f64,
}

impl SummaryRecord {
    fn new(record_type: &str, summary: &SlowCommandSummary) -> Self {
        Self {
            record_type: record_type.to_string(),
            schema_version: SCHEMA_VERSION,
            rollout_files_seen: summary.rollout_files_seen,
            rollout_files_analyzed: summary.rollout_files_analyzed,
            rollout_files_skipped: summary.rollout_files_skipped,
            internal_rollout_files_skipped: summary.internal_rollout_files_skipped,
            rollout_lines_skipped: summary.rollout_lines_skipped,
            direct_shell_calls_seen: summary.direct_shell_calls_seen,
            direct_shell_calls_analyzed: summary.direct_shell_calls_analyzed,
            direct_shell_calls_skipped: summary.direct_shell_calls_skipped,
            continuation_calls_seen: summary.continuation_calls_seen,
            continuation_calls_attributed: summary.continuation_calls_attributed,
            continuation_calls_skipped: summary.continuation_calls_skipped,
            duplicate_call_ids: summary.duplicate_call_ids,
            open_processes_at_eof: summary.open_processes_at_eof,
            code_mode_cells_ignored: summary.code_mode_cells_ignored,
            total_wait_seconds: seconds(summary.total_wait),
        }
    }

    fn into_collection_summary(self) -> SlowCommandSummary {
        SlowCommandSummary {
            rollout_files_seen: self.rollout_files_seen,
            rollout_files_analyzed: self.rollout_files_analyzed,
            rollout_files_skipped: self.rollout_files_skipped,
            internal_rollout_files_skipped: self.internal_rollout_files_skipped,
            rollout_lines_skipped: self.rollout_lines_skipped,
            duplicate_call_ids: self.duplicate_call_ids,
            code_mode_cells_ignored: self.code_mode_cells_ignored,
            ..Default::default()
        }
    }
}

#[derive(Serialize)]
struct RankingRecord<'a> {
    #[serde(rename = "type")]
    record_type: &'static str,
    schema_version: u32,
    rank: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    executable: Option<&'a str>,
    invocation_count: usize,
    continuation_wait_count: usize,
    total_wait_seconds: f64,
    average_wait_seconds: f64,
    max_wait_seconds: f64,
}

#[derive(Deserialize, Serialize)]
struct DirectCallRecord {
    #[serde(rename = "type")]
    record_type: String,
    schema_version: u32,
    call_id: String,
    command: Option<String>,
    wait_seconds: Option<f64>,
    observed_running: bool,
    observed_terminal: bool,
}

#[derive(Deserialize, Serialize)]
struct ContinuationRecord {
    #[serde(rename = "type")]
    record_type: String,
    schema_version: u32,
    call_id: String,
    invocation_call_id: Option<String>,
    wait_seconds: Option<f64>,
    observed_running: bool,
    observed_terminal: bool,
}

#[derive(Clone, Copy)]
enum RankingKind {
    ExactCommand,
    CommandFamily,
}

struct FixedPrecisionFormatter;

impl serde_json::ser::Formatter for FixedPrecisionFormatter {
    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        write!(writer, "{value:.4}")
    }
}

pub(crate) async fn run(command: SlowCommandsCommand) -> Result<()> {
    let mut output = BufWriter::new(std::io::stdout().lock());
    match command.action {
        None => {
            let codex_home = find_codex_home()?;
            let analysis = analyze_slow_commands(codex_home.as_path()).await;
            write_analysis(&mut output, &analysis)?;
        }
        Some(SlowCommandsAction::Collect) => {
            let codex_home = find_codex_home()?;
            let collection = collect_slow_commands(codex_home.as_path()).await;
            write_collection(&mut output, &collection)?;
        }
        Some(SlowCommandsAction::Merge) => {
            let collections = read_collections(std::io::stdin().lock())?;
            let analysis = merge_slow_command_collections(collections);
            write_analysis(&mut output, &analysis)?;
        }
    }
    output.flush()?;
    Ok(())
}

fn write_analysis(
    output: &mut impl Write,
    analysis: &codex_rollout::SlowCommandAnalysis,
) -> Result<()> {
    write_json_line(output, &SummaryRecord::new("summary", &analysis.summary))?;
    for (index, aggregate) in analysis.exact_commands.iter().enumerate() {
        write_ranking(output, RankingKind::ExactCommand, index, aggregate)?;
    }
    for (index, aggregate) in analysis.command_families.iter().enumerate() {
        write_ranking(output, RankingKind::CommandFamily, index, aggregate)?;
    }
    Ok(())
}

fn write_collection(output: &mut impl Write, collection: &SlowCommandCollection) -> Result<()> {
    write_json_line(
        output,
        &SummaryRecord::new("collection_summary", &collection.summary),
    )?;
    for direct_call in &collection.direct_calls {
        write_json_line(
            output,
            &DirectCallRecord {
                record_type: "direct_call".to_string(),
                schema_version: SCHEMA_VERSION,
                call_id: direct_call.call_id.clone(),
                command: direct_call.command.clone(),
                wait_seconds: direct_call.wait.map(seconds),
                observed_running: direct_call.observed_running,
                observed_terminal: direct_call.observed_terminal,
            },
        )?;
    }
    for continuation in &collection.continuations {
        write_json_line(
            output,
            &ContinuationRecord {
                record_type: "continuation".to_string(),
                schema_version: SCHEMA_VERSION,
                call_id: continuation.call_id.clone(),
                invocation_call_id: continuation.invocation_call_id.clone(),
                wait_seconds: continuation.wait.map(seconds),
                observed_running: continuation.observed_running,
                observed_terminal: continuation.observed_terminal,
            },
        )?;
    }
    Ok(())
}

fn read_collections(reader: impl BufRead) -> Result<Vec<SlowCommandCollection>> {
    let mut collections = Vec::new();
    let mut current: Option<SlowCommandCollection> = None;
    for (index, line) in reader.lines().enumerate() {
        let line_number = index.saturating_add(1);
        let line = line.with_context(|| format!("failed to read collection line {line_number}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON on collection line {line_number}"))?;
        let record_type = value
            .get("type")
            .and_then(Value::as_str)
            .with_context(|| format!("missing record type on collection line {line_number}"))?;
        match record_type {
            "collection_summary" => {
                if let Some(collection) = current.take() {
                    collections.push(collection);
                }
                let record: SummaryRecord = serde_json::from_value(value)
                    .with_context(|| format!("invalid collection summary on line {line_number}"))?;
                validate_schema_version(record.schema_version, line_number)?;
                current = Some(SlowCommandCollection {
                    summary: record.into_collection_summary(),
                    ..Default::default()
                });
            }
            "direct_call" => {
                let record: DirectCallRecord = serde_json::from_value(value)
                    .with_context(|| format!("invalid direct call on line {line_number}"))?;
                validate_schema_version(record.schema_version, line_number)?;
                current
                    .as_mut()
                    .with_context(|| {
                        format!("direct call before collection summary on line {line_number}")
                    })?
                    .direct_calls
                    .push(SlowCommandDirectCall {
                        call_id: record.call_id,
                        command: record.command,
                        wait: parse_duration(record.wait_seconds, line_number)?,
                        observed_running: record.observed_running,
                        observed_terminal: record.observed_terminal,
                    });
            }
            "continuation" => {
                let record: ContinuationRecord = serde_json::from_value(value)
                    .with_context(|| format!("invalid continuation on line {line_number}"))?;
                validate_schema_version(record.schema_version, line_number)?;
                current
                    .as_mut()
                    .with_context(|| {
                        format!("continuation before collection summary on line {line_number}")
                    })?
                    .continuations
                    .push(SlowCommandContinuation {
                        call_id: record.call_id,
                        invocation_call_id: record.invocation_call_id,
                        wait: parse_duration(record.wait_seconds, line_number)?,
                        observed_running: record.observed_running,
                        observed_terminal: record.observed_terminal,
                    });
            }
            other => {
                anyhow::bail!("unsupported record type `{other}` on collection line {line_number}")
            }
        }
    }
    if let Some(collection) = current {
        collections.push(collection);
    }
    Ok(collections)
}

fn validate_schema_version(schema_version: u32, line_number: usize) -> Result<()> {
    if schema_version != SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported schema version {schema_version} on collection line {line_number}"
        );
    }
    Ok(())
}

fn parse_duration(seconds: Option<f64>, line_number: usize) -> Result<Option<Duration>> {
    seconds
        .map(Duration::try_from_secs_f64)
        .transpose()
        .with_context(|| format!("invalid wait duration on collection line {line_number}"))
}

fn write_ranking(
    output: &mut impl Write,
    kind: RankingKind,
    index: usize,
    aggregate: &SlowCommandAggregate,
) -> Result<()> {
    let (record_type, command, executable) = match kind {
        RankingKind::ExactCommand => ("exact_command", Some(aggregate.key.as_str()), None),
        RankingKind::CommandFamily => ("command_family", None, Some(aggregate.key.as_str())),
    };
    let record = RankingRecord {
        record_type,
        schema_version: SCHEMA_VERSION,
        rank: index.saturating_add(1),
        command,
        executable,
        invocation_count: aggregate.invocation_count,
        continuation_wait_count: aggregate.continuation_wait_count,
        total_wait_seconds: seconds(aggregate.total_wait),
        average_wait_seconds: seconds(aggregate.average_wait()),
        max_wait_seconds: seconds(aggregate.max_wait),
    };
    write_json_line(output, &record)
}

fn write_json_line(output: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut *output, FixedPrecisionFormatter);
    value.serialize(&mut serializer)?;
    output.write_all(b"\n")?;
    Ok(())
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}
