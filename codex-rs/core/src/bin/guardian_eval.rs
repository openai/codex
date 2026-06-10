use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use clap::ValueEnum;
use codex_core::guardian_eval::GuardianEvalCaseStatus;
use codex_core::guardian_eval::GuardianEvalOptions;
use codex_core::guardian_eval::GuardianEvalReport;
use codex_core::guardian_eval::run_guardian_eval_suite;

#[derive(Debug, Parser)]
#[command(name = "codex-guardian-eval")]
#[command(about = "Run live Guardian approval-review eval fixtures")]
struct Args {
    #[arg(long)]
    cases: Option<PathBuf>,

    #[arg(long = "case")]
    case_ids: Vec<String>,

    #[arg(long)]
    model: Option<String>,

    #[arg(long, default_value_t = 1)]
    concurrency: usize,

    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    output: OutputFormat,

    #[arg(long)]
    dump_prompts: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let cases = args.cases.unwrap_or_else(default_cases_dir);
    let result = run_guardian_eval_suite(
        cases,
        GuardianEvalOptions {
            case_ids: args.case_ids,
            model: args.model,
            concurrency: args.concurrency,
            dump_prompts: args.dump_prompts,
            ..GuardianEvalOptions::default()
        },
    )
    .await;

    let report = match result {
        Ok(report) => report,
        Err(err) => {
            eprintln!("guardian eval failed to start: {err:#}");
            return ExitCode::from(1);
        }
    };

    match args.output {
        OutputFormat::Human => {
            if let Err(err) = write_human_report(&report, &mut std::io::stdout()) {
                eprintln!("failed to write guardian eval report: {err}");
                return ExitCode::from(1);
            }
        }
        OutputFormat::Json => match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                eprintln!("failed to serialize guardian eval report: {err}");
                return ExitCode::from(1);
            }
        },
    }

    if report.all_passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn default_cases_dir() -> PathBuf {
    let repo_root_relative = PathBuf::from("codex-rs/core/evals/guardian/cases");
    if repo_root_relative.exists() {
        repo_root_relative
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evals/guardian/cases")
    }
}

fn write_human_report(report: &GuardianEvalReport, writer: &mut impl Write) -> std::io::Result<()> {
    writeln!(
        writer,
        "Guardian evals: {}/{} passed ({:.1}%)",
        report.passed,
        report.total,
        report.pass_rate * 100.0
    )?;
    if let Some(model) = &report.selected_model {
        writeln!(writer, "Model: {model}")?;
    }
    if !report.per_tag.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "Tags:")?;
        for (tag, summary) in &report.per_tag {
            writeln!(
                writer,
                "  {tag}: {}/{} passed ({:.1}%)",
                summary.passed,
                summary.total,
                summary.pass_rate * 100.0
            )?;
        }
    }
    writeln!(writer)?;
    writeln!(writer, "Cases:")?;
    for case in &report.cases {
        let status = match case.status {
            GuardianEvalCaseStatus::Passed => "PASS",
            GuardianEvalCaseStatus::Mismatch => "FAIL",
            GuardianEvalCaseStatus::Error => "ERROR",
        };
        writeln!(writer, "  [{status}] {}", case.id)?;
        if let Some(model) = &case.selected_model {
            writeln!(writer, "    model: {model}")?;
        }
        if let Some(actual) = &case.actual {
            writeln!(
                writer,
                "    actual: outcome={}, risk_level={}, user_authorization={}",
                serde_json::to_string(&actual.outcome).unwrap_or_else(|_| "\"?\"".to_string()),
                serde_json::to_string(&actual.risk_level).unwrap_or_else(|_| "\"?\"".to_string()),
                serde_json::to_string(&actual.user_authorization)
                    .unwrap_or_else(|_| "\"?\"".to_string())
            )?;
            if !actual.rationale.trim().is_empty() {
                writeln!(writer, "    rationale: {}", actual.rationale.trim())?;
            }
        }
        if let Some(reason) = &case.mismatch_reason {
            writeln!(writer, "    mismatch: {reason}")?;
        }
        if let Some(error) = &case.error {
            writeln!(writer, "    error: {error}")?;
        }
    }
    Ok(())
}
