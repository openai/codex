use std::collections::HashMap;
use std::env;
use std::hint::black_box;
use std::process::Command;
use std::process::ExitCode;
use std::time::Instant;

use codex_code_mode::CodeModeService;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::ToolDefinition;
use serde::Deserialize;
use serde::Serialize;

const DEFAULT_SAMPLES: usize = 8;
const DEFAULT_WARM_ITERATIONS: usize = 25;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_TOOL_COUNTS: &[usize] = &[0, 32, 128];
const BENCH_SOURCE: &str = r#"text("bench");"#;

fn main() -> ExitCode {
    match try_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn try_main() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1))?;
    if let Some(worker) = args.worker {
        let report = run_worker(worker)?;
        println!(
            "{}",
            serde_json::to_string(&report)
                .map_err(|err| format!("failed to serialize benchmark report: {err}"))?
        );
        return Ok(());
    }

    let config = args.parent.unwrap_or_default();
    run_parent(config)
}

#[derive(Clone, Debug)]
struct ParentArgs {
    samples: usize,
    warm_iterations: usize,
    tool_counts: Vec<usize>,
}

impl Default for ParentArgs {
    fn default() -> Self {
        Self {
            samples: DEFAULT_SAMPLES,
            warm_iterations: DEFAULT_WARM_ITERATIONS,
            tool_counts: DEFAULT_TOOL_COUNTS.to_vec(),
        }
    }
}

#[derive(Clone, Debug)]
struct WorkerArgs {
    scenario: Scenario,
    tool_count: usize,
    iterations: usize,
    warmups: usize,
}

#[derive(Clone, Debug)]
struct Args {
    parent: Option<ParentArgs>,
    worker: Option<WorkerArgs>,
}

impl Args {
    fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut parent = ParentArgs::default();
        let mut worker_mode = false;
        let mut scenario = None;
        let mut tool_count = None;
        let mut iterations = None;
        let mut warmups = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bench" => {}
                "--worker" => worker_mode = true,
                "--samples" => parent.samples = parse_usize_flag("--samples", args.next())?,
                "--warm-iterations" => {
                    parent.warm_iterations = parse_usize_flag("--warm-iterations", args.next())?;
                }
                "--tool-counts" => {
                    parent.tool_counts = parse_tool_counts(args.next())?;
                }
                "--scenario" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing value for --scenario".to_string())?;
                    scenario = Some(Scenario::parse(&value)?);
                }
                "--tool-count" => {
                    tool_count = Some(parse_usize_flag("--tool-count", args.next())?);
                }
                "--iterations" => {
                    iterations = Some(parse_usize_flag("--iterations", args.next())?);
                }
                "--warmups" => {
                    warmups = Some(parse_usize_flag("--warmups", args.next())?);
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown argument: {arg}")),
            }
        }

        if worker_mode {
            let scenario =
                scenario.ok_or_else(|| "missing --scenario in worker mode".to_string())?;
            let tool_count =
                tool_count.ok_or_else(|| "missing --tool-count in worker mode".to_string())?;
            let iterations =
                iterations.ok_or_else(|| "missing --iterations in worker mode".to_string())?;
            let warmups = warmups.ok_or_else(|| "missing --warmups in worker mode".to_string())?;
            return Ok(Self {
                parent: None,
                worker: Some(WorkerArgs {
                    scenario,
                    tool_count,
                    iterations,
                    warmups,
                }),
            });
        }

        if parent.samples == 0 {
            return Err("--samples must be greater than 0".to_string());
        }
        if parent.warm_iterations == 0 {
            return Err("--warm-iterations must be greater than 0".to_string());
        }
        if parent.tool_counts.is_empty() {
            return Err("--tool-counts must include at least one count".to_string());
        }

        Ok(Self {
            parent: Some(parent),
            worker: None,
        })
    }
}

fn print_help() {
    println!(
        "exec_overhead benchmark\n\
         \n\
         Usage:\n\
           cargo bench -p codex-code-mode --bench exec_overhead -- [--samples N] [--warm-iterations N] [--tool-counts 0,32,128]\n\
         \n\
         The benchmark runs two scenarios for each tool count:\n\
         - cold_exec: one fresh exec in a fresh process\n\
         - warm_exec: repeated execs after one warmup exec in a fresh process\n\
         \n\
         Memory is reported as a fresh-process max RSS delta for each scenario.\n"
    );
}

fn parse_usize_flag(flag: &str, value: Option<String>) -> Result<usize, String> {
    let value = value.ok_or_else(|| format!("missing value for {flag}"))?;
    value
        .parse::<usize>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
}

fn parse_tool_counts(value: Option<String>) -> Result<Vec<usize>, String> {
    let value = value.ok_or_else(|| "missing value for --tool-counts".to_string())?;
    let mut counts = Vec::new();
    for item in value.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        counts.push(
            trimmed
                .parse::<usize>()
                .map_err(|err| format!("invalid tool count `{trimmed}`: {err}"))?,
        );
    }
    if counts.is_empty() {
        return Err("--tool-counts must include at least one count".to_string());
    }
    Ok(counts)
}

#[derive(Clone, Copy, Debug)]
enum Scenario {
    ColdExec,
    WarmExec,
}

impl Scenario {
    fn all() -> [Self; 2] {
        [Self::ColdExec, Self::WarmExec]
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cold_exec" => Ok(Self::ColdExec),
            "warm_exec" => Ok(Self::WarmExec),
            _ => Err(format!("unknown scenario `{value}`")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ColdExec => "cold_exec",
            Self::WarmExec => "warm_exec",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkerReport {
    scenario: String,
    tool_count: usize,
    iterations: usize,
    warmups: usize,
    mean_exec_nanos: u128,
    min_exec_nanos: u128,
    max_exec_nanos: u128,
    total_exec_nanos: u128,
    max_rss_delta_bytes: u64,
}

#[derive(Debug)]
struct SummaryRow {
    scenario: Scenario,
    tool_count: usize,
    samples: usize,
    iterations: usize,
    warmups: usize,
    mean_exec_nanos: u128,
    p95_exec_nanos: u128,
    median_rss_delta_bytes: u64,
    max_rss_delta_bytes: u64,
}

fn run_parent(config: ParentArgs) -> Result<(), String> {
    let exe = env::current_exe().map_err(|err| format!("failed to locate bench binary: {err}"))?;
    let mut rows = Vec::new();

    for tool_count in &config.tool_counts {
        for scenario in Scenario::all() {
            let (iterations, warmups) = match scenario {
                Scenario::ColdExec => (1, 0),
                Scenario::WarmExec => (config.warm_iterations, DEFAULT_WARMUPS),
            };
            let mut reports = Vec::with_capacity(config.samples);
            for _sample in 0..config.samples {
                reports.push(run_sample(
                    &exe,
                    WorkerArgs {
                        scenario,
                        tool_count: *tool_count,
                        iterations,
                        warmups,
                    },
                )?);
            }
            rows.push(summarize_reports(scenario, *tool_count, reports)?);
        }
    }

    print_summary(&config, &rows);
    Ok(())
}

fn run_sample(exe: &std::path::Path, args: WorkerArgs) -> Result<WorkerReport, String> {
    let output = Command::new(exe)
        .arg("--worker")
        .arg("--scenario")
        .arg(args.scenario.as_str())
        .arg("--tool-count")
        .arg(args.tool_count.to_string())
        .arg("--iterations")
        .arg(args.iterations.to_string())
        .arg("--warmups")
        .arg(args.warmups.to_string())
        .output()
        .map_err(|err| format!("failed to run benchmark worker: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "benchmark worker failed for scenario {} / tools {}: {}",
            args.scenario.as_str(),
            args.tool_count,
            stderr.trim()
        ));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("failed to parse benchmark worker output: {err}"))
}

fn summarize_reports(
    scenario: Scenario,
    tool_count: usize,
    reports: Vec<WorkerReport>,
) -> Result<SummaryRow, String> {
    if reports.is_empty() {
        return Err("no benchmark reports collected".to_string());
    }
    let iterations = reports[0].iterations;
    let warmups = reports[0].warmups;
    let mut exec_times = reports
        .iter()
        .map(|report| report.mean_exec_nanos)
        .collect::<Vec<_>>();
    exec_times.sort_unstable();
    let mut rss_deltas = reports
        .iter()
        .map(|report| report.max_rss_delta_bytes)
        .collect::<Vec<_>>();
    rss_deltas.sort_unstable();

    let mean_exec_nanos = exec_times.iter().sum::<u128>() / u128::from(exec_times.len() as u64);
    let p95_exec_nanos = percentile_u128(&exec_times, 95);
    let median_rss_delta_bytes = percentile(&rss_deltas, 50);
    let max_rss_delta_bytes = rss_deltas.last().copied().unwrap_or(0);

    Ok(SummaryRow {
        scenario,
        tool_count,
        samples: reports.len(),
        iterations,
        warmups,
        mean_exec_nanos,
        p95_exec_nanos,
        median_rss_delta_bytes,
        max_rss_delta_bytes,
    })
}

fn percentile(values: &[u64], percentile: usize) -> u64 {
    debug_assert!(!values.is_empty());
    let last_index = values.len().saturating_sub(1);
    let index = (last_index * percentile) / 100;
    values[index]
}

fn percentile_u128(values: &[u128], percentile: usize) -> u128 {
    debug_assert!(!values.is_empty());
    let last_index = values.len().saturating_sub(1);
    let index = (last_index * percentile) / 100;
    values[index]
}

fn print_summary(config: &ParentArgs, rows: &[SummaryRow]) {
    println!(
        "exec_overhead: samples={}, warm_iterations={}, tool_counts={:?}",
        config.samples, config.warm_iterations, config.tool_counts
    );
    println!(
        "{:<12} {:>7} {:>7} {:>10} {:>10} {:>14} {:>14} {:>14} {:>14}",
        "scenario",
        "tools",
        "samples",
        "warmups",
        "iters",
        "mean/exec",
        "p95/exec",
        "rssΔ p50",
        "rssΔ max"
    );
    for row in rows {
        println!(
            "{:<12} {:>7} {:>7} {:>10} {:>10} {:>14} {:>14} {:>14} {:>14}",
            row.scenario.as_str(),
            row.tool_count,
            row.samples,
            row.warmups,
            row.iterations,
            format_duration_nanos(row.mean_exec_nanos),
            format_duration_nanos(row.p95_exec_nanos),
            format_bytes(row.median_rss_delta_bytes),
            format_bytes(row.max_rss_delta_bytes),
        );
    }
    println!("memory uses a fresh-process max RSS delta for each scenario");
}

fn format_duration_nanos(nanos: u128) -> String {
    if nanos >= 1_000_000_000 {
        return format!("{:.2}s", nanos as f64 / 1_000_000_000.0);
    }
    if nanos >= 1_000_000 {
        return format!("{:.2}ms", nanos as f64 / 1_000_000.0);
    }
    if nanos >= 1_000 {
        return format!("{:.2}us", nanos as f64 / 1_000.0);
    }
    format!("{nanos}ns")
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;

    if bytes >= GIB {
        return format!("{:.2}GiB", bytes as f64 / GIB as f64);
    }
    if bytes >= MIB {
        return format!("{:.2}MiB", bytes as f64 / MIB as f64);
    }
    if bytes >= KIB {
        return format!("{:.2}KiB", bytes as f64 / KIB as f64);
    }
    format!("{bytes}B")
}

fn run_worker(args: WorkerArgs) -> Result<WorkerReport, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to create tokio runtime: {err}"))?;

    runtime.block_on(async move {
        let service = CodeModeService::new();
        let tools = benchmark_tools(args.tool_count);

        for warmup_index in 0..args.warmups {
            execute_benchmark_script(&service, &tools, warmup_index).await?;
        }

        let baseline_rss = current_max_rss_bytes()?;
        let mut total_exec_nanos = 0_u128;
        let mut min_exec_nanos = u128::MAX;
        let mut max_exec_nanos = 0_u128;

        for iteration in 0..args.iterations {
            let started_at = Instant::now();
            let response =
                execute_benchmark_script(&service, &tools, args.warmups + iteration).await?;
            let elapsed_nanos = started_at.elapsed().as_nanos();
            total_exec_nanos += elapsed_nanos;
            min_exec_nanos = min_exec_nanos.min(elapsed_nanos);
            max_exec_nanos = max_exec_nanos.max(elapsed_nanos);
            black_box(response);
        }

        let mean_exec_nanos = total_exec_nanos / u128::from(args.iterations as u64);
        let max_rss_delta_bytes = current_max_rss_bytes()?.saturating_sub(baseline_rss);

        Ok(WorkerReport {
            scenario: args.scenario.as_str().to_string(),
            tool_count: args.tool_count,
            iterations: args.iterations,
            warmups: args.warmups,
            mean_exec_nanos,
            min_exec_nanos,
            max_exec_nanos,
            total_exec_nanos,
            max_rss_delta_bytes,
        })
    })
}

async fn execute_benchmark_script(
    service: &CodeModeService,
    tools: &[ToolDefinition],
    exec_index: usize,
) -> Result<RuntimeResponse, String> {
    let response = service
        .execute(ExecuteRequest {
            tool_call_id: format!("bench_call_{exec_index}"),
            enabled_tools: tools.to_vec(),
            source: BENCH_SOURCE.to_string(),
            stored_values: HashMap::new(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await?;

    match &response {
        RuntimeResponse::Result {
            error_text,
            content_items,
            ..
        } => {
            if error_text.is_some() {
                return Err(format!(
                    "benchmark exec failed unexpectedly: {error_text:?}"
                ));
            }
            if content_items.len() != 1 {
                return Err(format!(
                    "benchmark exec produced unexpected content item count: {}",
                    content_items.len()
                ));
            }
        }
        unexpected => {
            return Err(format!(
                "benchmark exec produced unexpected response: {unexpected:?}"
            ));
        }
    }

    Ok(response)
}

fn benchmark_tools(tool_count: usize) -> Vec<ToolDefinition> {
    (0..tool_count)
        .map(|tool_index| ToolDefinition {
            name: format!("bench_tool_{tool_index:04}"),
            description: format!("Benchmark tool {tool_index}"),
            kind: CodeModeToolKind::Function,
            input_schema: None,
            output_schema: None,
        })
        .collect()
}

fn current_max_rss_bytes() -> Result<u64, String> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if status != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let max_rss = unsafe { usage.assume_init() }.ru_maxrss as u64;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        Ok(max_rss)
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        Ok(max_rss.saturating_mul(1024))
    }
}
