use super::CodexClient;
use super::loopback_responses_server::LoopbackResponsesServer;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use serde_json::Value;
use serde_json::json;
use std::collections::VecDeque;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

const MOCK_MODEL_SLUG: &str = "unified-exec-benchmark";
const MOCK_PROVIDER_ID: &str = "unified_exec_benchmark";
const OUTPUT_MARKER: &str = "unified_exec_benchmark_marker";
const MAX_COMMAND_DURATION_MS: u64 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationPath {
    Direct,
    CodeMode,
}

impl fmt::Display for InvocationPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => f.write_str("direct"),
            Self::CodeMode => f.write_str("code_mode"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PlannedSample {
    path: InvocationPath,
    warmup: bool,
}

#[derive(Clone, Copy, Debug)]
struct Observation {
    path: InvocationPath,
    elapsed: Duration,
}

struct BenchmarkResponder {
    samples: VecDeque<PlannedSample>,
    active_sample: Option<(PlannedSample, Instant)>,
    observations: Arc<Mutex<Vec<Observation>>>,
    next_response_id: usize,
    command: String,
    yield_time_ms: u64,
}

struct TemporaryCodexHome {
    path: PathBuf,
}

impl TemporaryCodexHome {
    fn create() -> Result<Self> {
        let path = env::temp_dir().join(format!("codex-unified-exec-benchmark-{}", Uuid::new_v4()));
        fs::create_dir(&path)
            .with_context(|| format!("create temporary Codex home {}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryCodexHome {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "failed to remove temporary Codex home {}: {err}",
                self.path.display()
            );
        }
    }
}

impl BenchmarkResponder {
    fn respond(&mut self, _request: &[u8]) -> io::Result<String> {
        if let Some((sample, started_at)) = self.active_sample.take() {
            if !sample.warmup {
                self.observations
                    .lock()
                    .map_err(|_| io::Error::other("benchmark observations lock poisoned"))?
                    .push(Observation {
                        path: sample.path,
                        elapsed: started_at.elapsed(),
                    });
            }
            return Ok(self.completed_response());
        }

        let sample = self
            .samples
            .pop_front()
            .ok_or_else(|| io::Error::other("unexpected extra Responses API request"))?;
        self.active_sample = Some((sample, Instant::now()));
        Ok(self.tool_call_response(sample.path))
    }

    fn tool_call_response(&mut self, path: InvocationPath) -> String {
        let response_id = self.response_id();
        let call_id = format!("call-{response_id}");
        let arguments = json!({
            "cmd": &self.command,
            "yield_time_ms": self.yield_time_ms,
        })
        .to_string();
        let tool_call = match path {
            InvocationPath::Direct => json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": call_id,
                    "name": "exec_command",
                    "arguments": arguments,
                }
            }),
            InvocationPath::CodeMode => {
                let code = format!(
                    "const result = await tools.exec_command({arguments});\ntext(result.output);"
                );
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "custom_tool_call",
                        "call_id": call_id,
                        "name": "exec",
                        "input": code,
                    }
                })
            }
        };
        sse([
            json!({
                "type": "response.created",
                "response": { "id": &response_id },
            }),
            tool_call,
            completed_event(&response_id),
        ])
    }

    fn completed_response(&mut self) -> String {
        let response_id = self.response_id();
        sse([
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "id": format!("message-{response_id}"),
                    "content": [{ "type": "output_text", "text": "done" }],
                }
            }),
            completed_event(&response_id),
        ])
    }

    fn response_id(&mut self) -> String {
        let response_id = format!("benchmark-{}", self.next_response_id);
        self.next_response_id += 1;
        response_id
    }
}

pub(super) fn run(
    codex_bin: &Path,
    config_overrides: &[String],
    samples: usize,
    warmups: usize,
    command_duration_ms: u64,
    workspace: &Path,
) -> Result<()> {
    if cfg!(windows) {
        bail!("unified-exec-benchmark currently requires the POSIX sleep utility");
    }
    if samples == 0 {
        bail!("--samples must be greater than zero");
    }
    if command_duration_ms == 0 || command_duration_ms > MAX_COMMAND_DURATION_MS {
        bail!(
            "--command-duration-ms must be between 1 and {MAX_COMMAND_DURATION_MS} for the single-call benchmark"
        );
    }

    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("resolve workspace {}", workspace.display()))?;
    let command_duration = Duration::from_millis(command_duration_ms);
    let command = format!(
        "sleep {:.3}; printf {OUTPUT_MARKER}",
        command_duration.as_secs_f64()
    );
    let schedule = sample_schedule(samples, warmups);
    let observations = Arc::new(Mutex::new(Vec::with_capacity(samples * 2)));
    let mut responder = BenchmarkResponder {
        samples: schedule.clone().into(),
        active_sample: None,
        observations: Arc::clone(&observations),
        next_response_id: 1,
        command: command.clone(),
        yield_time_ms: command_duration_ms + 1_000,
    };
    let responses_server =
        LoopbackResponsesServer::start_with_responder(move |request| responder.respond(request))?;

    let mut overrides = config_overrides.to_vec();
    overrides.extend(benchmark_config_overrides(responses_server.base_url())?);
    let codex_home = TemporaryCodexHome::create()?;
    let environment = [(
        OsString::from("CODEX_HOME"),
        codex_home.path.as_os_str().to_os_string(),
    )];
    let mut client = CodexClient::spawn_stdio_with_env(codex_bin, &overrides, &environment)?;
    client.quiet = true;
    client.initialize()?;
    let thread = client.thread_start(ThreadStartParams {
        model: Some(MOCK_MODEL_SLUG.to_string()),
        model_provider: Some(MOCK_PROVIDER_ID.to_string()),
        base_instructions: Some(String::new()),
        developer_instructions: Some(String::new()),
        ephemeral: Some(true),
        ..Default::default()
    })?;

    for sample in schedule {
        reset_turn_observations(&mut client);
        run_sample(&mut client, &thread.thread.id, &workspace)?;
        validate_sample(&client, sample)?;
    }

    let observations = observations
        .lock()
        .map_err(|_| anyhow::anyhow!("benchmark observations lock poisoned"))?
        .clone();
    if observations.len() != samples * 2 {
        bail!(
            "expected {} measured samples, recorded {}",
            samples * 2,
            observations.len()
        );
    }

    print_results(
        codex_bin,
        &command,
        samples,
        warmups,
        command_duration,
        &observations,
    );
    Ok(())
}

fn run_sample(client: &mut CodexClient, thread_id: &str, workspace: &Path) -> Result<()> {
    let turn = client.turn_start(TurnStartParams {
        thread_id: thread_id.to_string(),
        client_user_message_id: None,
        input: vec![UserInput::Text {
            text: "run the benchmark tool call".to_string(),
            text_elements: Vec::new(),
        }],
        approval_policy: Some(AskForApproval::Never),
        sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
        cwd: Some(workspace.to_path_buf()),
        ..Default::default()
    })?;
    client.stream_turn(thread_id, &turn.turn.id)
}

fn validate_sample(client: &CodexClient, sample: PlannedSample) -> Result<()> {
    if client.last_turn_status != Some(TurnStatus::Completed) {
        bail!(
            "{} sample failed: turn status {:?}, error {:?}",
            sample.path,
            client.last_turn_status,
            client.last_turn_error_message
        );
    }
    if client.command_execution_statuses.last() != Some(&CommandExecutionStatus::Completed) {
        bail!(
            "{} sample failed: command statuses {:?}",
            sample.path,
            client.command_execution_statuses
        );
    }
    if !client
        .command_execution_outputs
        .last()
        .is_some_and(|output| output.contains(OUTPUT_MARKER))
    {
        bail!("{} sample did not emit the expected marker", sample.path);
    }
    Ok(())
}

fn reset_turn_observations(client: &mut CodexClient) {
    client.command_execution_statuses.clear();
    client.command_execution_outputs.clear();
    client.command_output_stream.clear();
    client.command_item_started = false;
    client.helper_done_seen = false;
    client.turn_completed_before_helper_done = false;
    client.unexpected_items_before_helper_done.clear();
    client.last_turn_status = None;
    client.last_turn_error_message = None;
}

fn sample_schedule(samples: usize, warmups: usize) -> Vec<PlannedSample> {
    let mut schedule = Vec::with_capacity((samples + warmups) * 2);
    for round in 0..(samples + warmups) {
        let paths = if round % 2 == 0 {
            [InvocationPath::Direct, InvocationPath::CodeMode]
        } else {
            [InvocationPath::CodeMode, InvocationPath::Direct]
        };
        for path in paths {
            schedule.push(PlannedSample {
                path,
                warmup: round < warmups,
            });
        }
    }
    schedule
}

fn benchmark_config_overrides(responses_base_url: &str) -> Result<Vec<String>> {
    let provider_base_url = quoted(&format!("{responses_base_url}/v1"))?;
    Ok(vec![
        format!("model={}", quoted(MOCK_MODEL_SLUG)?),
        format!("model_provider={}", quoted(MOCK_PROVIDER_ID)?),
        format!(
            "model_providers.{MOCK_PROVIDER_ID}.name={}",
            quoted("Unified exec benchmark mock provider")?
        ),
        format!("model_providers.{MOCK_PROVIDER_ID}.base_url={provider_base_url}"),
        format!(
            "model_providers.{MOCK_PROVIDER_ID}.wire_api={}",
            quoted("responses")?
        ),
        format!("model_providers.{MOCK_PROVIDER_ID}.requires_openai_auth=false"),
        format!("model_providers.{MOCK_PROVIDER_ID}.request_max_retries=0"),
        format!("model_providers.{MOCK_PROVIDER_ID}.stream_max_retries=0"),
        "features.unified_exec=true".to_string(),
        "features.code_mode=true".to_string(),
        "suppress_unstable_features_warning=true".to_string(),
    ])
}

fn quoted(value: &str) -> Result<String> {
    serde_json::to_string(value).context("serialize config string")
}

fn completed_event(response_id: &str) -> Value {
    json!({
        "type": "response.completed",
        "response": {
            "id": response_id,
            "usage": {
                "input_tokens": 0,
                "input_tokens_details": null,
                "output_tokens": 0,
                "output_tokens_details": null,
                "total_tokens": 0,
            }
        }
    })
}

fn sse(events: impl IntoIterator<Item = Value>) -> String {
    let mut body = String::new();
    for event in events {
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        body.push_str("event: ");
        body.push_str(event_type);
        body.push('\n');
        body.push_str("data: ");
        body.push_str(&event.to_string());
        body.push_str("\n\n");
    }
    body
}

#[derive(Debug)]
struct Summary {
    minimum_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    mean_ms: f64,
}

fn summarize(path: InvocationPath, observations: &[Observation]) -> Summary {
    let mut elapsed: Vec<Duration> = observations
        .iter()
        .filter(|observation| observation.path == path)
        .map(|observation| observation.elapsed)
        .collect();
    elapsed.sort_unstable();
    let mean = elapsed.iter().map(Duration::as_secs_f64).sum::<f64>() / elapsed.len() as f64;
    Summary {
        minimum_ms: elapsed[0].as_secs_f64() * 1_000.0,
        p50_ms: percentile(&elapsed, 50).as_secs_f64() * 1_000.0,
        p95_ms: percentile(&elapsed, 95).as_secs_f64() * 1_000.0,
        mean_ms: mean * 1_000.0,
    }
}

fn percentile(sorted: &[Duration], percentile: usize) -> Duration {
    let index = ((sorted.len() - 1) * percentile).div_ceil(100);
    sorted[index]
}

fn print_results(
    codex_bin: &Path,
    command: &str,
    samples: usize,
    warmups: usize,
    command_duration: Duration,
    observations: &[Observation],
) {
    let direct = summarize(InvocationPath::Direct, observations);
    let code_mode = summarize(InvocationPath::CodeMode, observations);
    let configured_ms = command_duration.as_secs_f64() * 1_000.0;
    let delta_ms = code_mode.mean_ms - direct.mean_ms;
    let delta_percent = delta_ms / direct.mean_ms * 100.0;

    println!("unified exec benchmark");
    println!("codex: {}", codex_bin.display());
    println!("command: {command}");
    println!("warmups: {warmups}/path; samples: {samples}/path");
    println!("timing: initial Responses request to follow-up Responses request");
    println!();
    println!(
        "{:<10} {:>10} {:>10} {:>10} {:>10} {:>14}",
        "path", "min", "p50", "p95", "mean", "mean-sleep"
    );
    println!(
        "{:<10} {:>9.3}ms {:>9.3}ms {:>9.3}ms {:>9.3}ms {:>13.3}ms",
        InvocationPath::Direct,
        direct.minimum_ms,
        direct.p50_ms,
        direct.p95_ms,
        direct.mean_ms,
        direct.mean_ms - configured_ms,
    );
    println!(
        "{:<10} {:>9.3}ms {:>9.3}ms {:>9.3}ms {:>9.3}ms {:>13.3}ms",
        InvocationPath::CodeMode,
        code_mode.minimum_ms,
        code_mode.p50_ms,
        code_mode.p95_ms,
        code_mode.mean_ms,
        code_mode.mean_ms - configured_ms,
    );
    println!();
    println!("code_mode - direct mean: {delta_ms:+.3}ms ({delta_percent:+.2}%)");
}
