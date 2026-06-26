use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ProcessId;
use opentelemetry::Value as OtelValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanData;
use serde_json::Value;
use serde_json::json;
use tracing::Instrument;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const ITERATIONS_ENV: &str = "EXEC_SERVER_LATENCY_ITERATIONS";
const WARMUP_ITERATIONS_ENV: &str = "EXEC_SERVER_LATENCY_WARMUP_ITERATIONS";
const PROCESS_COMPLETION_MODE_ENV: &str = "EXEC_SERVER_LATENCY_PROCESS_COMPLETION";
const RPC_CLIENT_SPAN_NAME: &str = "codex.exec_server.rpc.client_call";
const RPC_DURATION_FIELDS: [&str; 6] = [
    "duration_ms",
    "pending_registration_ms",
    "serialize_ms",
    "enqueue_ms",
    "response_wait_ms",
    "deserialize_ms",
];

#[derive(Clone, Copy)]
enum ProcessCompletionMode {
    Events,
    Read,
}

impl ProcessCompletionMode {
    fn from_env() -> Result<Self> {
        match std::env::var(PROCESS_COMPLETION_MODE_ENV)
            .unwrap_or_else(|_| "events".to_string())
            .as_str()
        {
            "events" => Ok(Self::Events),
            "read" => Ok(Self::Read),
            value => anyhow::bail!(
                "{PROCESS_COMPLETION_MODE_ENV} must be `events` or `read`, got {value:?}"
            ),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::Read => "read",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let span_exporter = InMemorySpanExporter::default();
    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter.clone())
        .build();
    let tracer = tracer_provider.tracer("exec-server-remote-latency");
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,hyper=warn,reqwest=warn")),
        );
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .try_init()
        .context("initialize benchmark tracing")?;
    tracing::callsite::rebuild_interest_cache();

    let iterations = positive_env_usize(ITERATIONS_ENV, 30)?;
    let warmup_iterations = positive_env_usize(WARMUP_ITERATIONS_ENV, 5)?;
    let process_completion_mode = ProcessCompletionMode::from_env()?;
    let benchmark_span = tracing::info_span!(
        "codex.exec_server.remote.benchmark",
        otel.kind = "client",
        otel.name = "codex.exec_server.remote.benchmark",
        iterations,
        warmup_iterations,
        process_completion_mode = process_completion_mode.as_str(),
    );

    let mut report = async move {
        let trace = codex_otel::current_span_w3c_trace_context();
        let connection_started_at = Instant::now();
        let manager = EnvironmentManager::from_env(/*local_runtime_paths*/ None).await?;
        let environment = manager
            .default_environment()
            .context("Noise remote environment is not configured")?;
        environment.wait_until_ready().await?;
        let connection_ms = elapsed_ms(connection_started_at);

        let environment_info = environment.info().await?;
        let cwd = environment_info
            .cwd
            .context("remote environment did not report a working directory")?;
        let filesystem = environment.get_filesystem();
        let exec_backend = environment.get_exec_backend();

        for iteration in 0..warmup_iterations {
            filesystem.get_metadata(&cwd, /*sandbox*/ None).await?;
            let started = exec_backend
                .start(ExecParams {
                    process_id: ProcessId::from(format!("latency-warmup-{iteration}")),
                    argv: vec!["/usr/bin/true".to_string()],
                    cwd: cwd.clone(),
                    env_policy: None,
                    env: HashMap::new(),
                    tty: false,
                    pipe_stdin: false,
                    arg0: None,
                    sandbox: None,
                    enforce_managed_network: false,
                    managed_network: None,
                })
                .await?;
            wait_for_process_completion(started.process, process_completion_mode).await?;
        }

        let mut metadata_ms = Vec::with_capacity(iterations);
        let mut process_start_ms = Vec::with_capacity(iterations);
        let mut process_completion_wait_ms = Vec::with_capacity(iterations);
        let mut process_completion_ms = Vec::with_capacity(iterations);
        for iteration in 0..iterations {
            let sample_span = tracing::info_span!(
                "codex.exec_server.remote.benchmark.sample",
                otel.kind = "client",
                otel.name = "codex.exec_server.remote.benchmark.sample",
                iteration,
            );
            async {
                let started_at = Instant::now();
                filesystem.get_metadata(&cwd, /*sandbox*/ None).await?;
                metadata_ms.push(elapsed_ms(started_at));

                let completion_started_at = Instant::now();
                let started_at = Instant::now();
                let started = exec_backend
                    .start(ExecParams {
                        process_id: ProcessId::from(format!("latency-measured-{iteration}")),
                        argv: vec!["/usr/bin/true".to_string()],
                        cwd: cwd.clone(),
                        env_policy: None,
                        env: HashMap::new(),
                        tty: false,
                        pipe_stdin: false,
                        arg0: None,
                        sandbox: None,
                        enforce_managed_network: false,
                        managed_network: None,
                    })
                    .await?;
                process_start_ms.push(elapsed_ms(started_at));

                let started_at = Instant::now();
                wait_for_process_completion(started.process, process_completion_mode).await?;
                process_completion_wait_ms.push(elapsed_ms(started_at));
                process_completion_ms.push(elapsed_ms(completion_started_at));
                Result::<()>::Ok(())
            }
            .instrument(sample_span)
            .await?;
        }

        Result::<Value>::Ok(json!({
            "environment_id": std::env::var("CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID").ok(),
            "traceparent": trace.and_then(|context| context.traceparent),
            "iterations": iterations,
            "warmup_iterations": warmup_iterations,
            "process_completion_mode": process_completion_mode.as_str(),
            "connection_ms": connection_ms,
            "fs_get_metadata_ms": summarize(&metadata_ms),
            "process_start_ms": summarize(&process_start_ms),
            "process_completion_wait_ms": summarize(&process_completion_wait_ms),
            "process_completion_ms": summarize(&process_completion_ms),
        }))
    }
    .instrument(benchmark_span)
    .await?;

    tracer_provider
        .force_flush()
        .context("flush benchmark spans")?;
    let spans = span_exporter
        .get_finished_spans()
        .context("read benchmark spans")?;
    report["rpc_client_ms"] = rpc_phase_summary(&spans);
    report["rendezvous_cluster"] = span_string_attribute(
        &spans,
        "codex.exec_server.remote.harness.registry_connect_bundle",
        "rendezvous.cluster",
    );
    report["rendezvous_route_id"] = span_string_attribute(
        &spans,
        "codex.exec_server.remote.harness.registry_connect_bundle",
        "rendezvous.route_id",
    );
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

async fn wait_for_process_completion(
    process: std::sync::Arc<dyn codex_exec_server::ExecProcess>,
    mode: ProcessCompletionMode,
) -> Result<()> {
    match mode {
        ProcessCompletionMode::Events => {
            let mut events = process.subscribe_events();
            loop {
                match events.recv().await {
                    Ok(ExecProcessEvent::Closed { .. }) => return Ok(()),
                    Ok(ExecProcessEvent::Failed(message)) => anyhow::bail!(message),
                    Ok(ExecProcessEvent::Output(_) | ExecProcessEvent::Exited { .. }) => {}
                    Err(error) => anyhow::bail!("process event stream failed: {error}"),
                }
            }
        }
        ProcessCompletionMode::Read => {
            let mut after_seq = None;
            loop {
                let response = process
                    .read(after_seq, /*max_bytes*/ None, Some(1_000))
                    .await?;
                if let Some(message) = response.failure {
                    anyhow::bail!(message);
                }
                if response.closed {
                    return Ok(());
                }
                after_seq = response.next_seq.checked_sub(1);
            }
        }
    }
}

fn positive_env_usize(name: &str, default: usize) -> Result<usize> {
    let Some(value) = std::env::var(name).ok() else {
        return Ok(default);
    };
    let value = value
        .parse::<usize>()
        .with_context(|| format!("{name} must be a positive integer"))?;
    anyhow::ensure!(value > 0, "{name} must be a positive integer");
    Ok(value)
}

fn elapsed_ms(started_at: Instant) -> f64 {
    started_at.elapsed().as_secs_f64() * 1_000.0
}

fn summarize(samples: &[f64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |percentile: usize| {
        let index = (sorted.len() * percentile).div_ceil(100).saturating_sub(1);
        sorted[index]
    };
    json!({
        "min": sorted[0],
        "p50": percentile(50),
        "p95": percentile(95),
        "max": sorted[sorted.len() - 1],
        "mean": sorted.iter().sum::<f64>() / sorted.len() as f64,
        "samples": samples,
    })
}

fn rpc_phase_summary(spans: &[SpanData]) -> Value {
    let mut samples_by_method = BTreeMap::<String, BTreeMap<&'static str, Vec<f64>>>::new();
    for span in spans
        .iter()
        .filter(|span| span.name.as_ref() == RPC_CLIENT_SPAN_NAME)
    {
        let Some(OtelValue::String(method)) = span_attribute(span, "rpc.method") else {
            continue;
        };
        let fields = samples_by_method.entry(method.to_string()).or_default();
        for field in RPC_DURATION_FIELDS {
            if let Some(OtelValue::F64(duration_ms)) = span_attribute(span, field) {
                fields.entry(field).or_default().push(*duration_ms);
            }
        }
    }

    Value::Object(
        samples_by_method
            .into_iter()
            .map(|(method, fields)| {
                let fields = fields
                    .into_iter()
                    .map(|(field, samples)| (field.to_string(), summarize(&samples)))
                    .collect();
                (method, Value::Object(fields))
            })
            .collect(),
    )
}

fn span_attribute<'a>(span: &'a SpanData, name: &str) -> Option<&'a OtelValue> {
    span.attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == name)
        .map(|attribute| &attribute.value)
}

fn span_string_attribute(spans: &[SpanData], span_name: &str, attribute_name: &str) -> Value {
    spans
        .iter()
        .find(|span| span.name.as_ref() == span_name)
        .and_then(|span| span_attribute(span, attribute_name))
        .and_then(|value| match value {
            OtelValue::String(value) => Some(Value::String(value.to_string())),
            _ => None,
        })
        .unwrap_or(Value::Null)
}
