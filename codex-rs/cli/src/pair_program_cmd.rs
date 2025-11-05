use anyhow::{Context, Result, anyhow};
use codex_common::CliConfigOverrides;
use codex_core::config::{Config, ConfigOverrides};
use codex_supervisor::{
    CoordinationStrategy, MergeStrategy, MultiAgentEvaluationConfig, MultiAgentEvaluationReport,
    MultiAgentEvaluator, SimpleEvaluationStrategy, SupervisorConfig,
};
use serde_json::to_string_pretty;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Run the multi-agent pair programming loop using natural-language goal input.
#[allow(clippy::too_many_arguments)]
pub async fn run_pair_program_command(
    config_overrides: CliConfigOverrides,
    goal: String,
    agents: Vec<String>,
    rounds: usize,
    top_k: usize,
    improvement_threshold: Option<f64>,
    max_risk: Option<f64>,
    out: Option<PathBuf>,
) -> Result<()> {
    let cli_overrides = config_overrides
        .parse_overrides()
        .map_err(|err| anyhow!("failed to parse -c overrides: {err}"))?;

    // We currently load the config mainly to honour overrides/env selection; the supervisor
    // pipeline is self-contained but future hooks may pull additional defaults from here.
    let _config = Config::load_with_cli_overrides(cli_overrides, ConfigOverrides::default())
        .await
        .context("failed to load configuration")?;

    let mut agent_pool = if agents.is_empty() {
        vec![
            "main-security-backend".to_string(),
            "sub-frontend-integration".to_string(),
        ]
    } else {
        agents
    };

    dedupe_preserving_order(&mut agent_pool);
    if agent_pool.len() < 2 {
        agent_pool.push("sub-frontend-integration".to_string());
        dedupe_preserving_order(&mut agent_pool);
    }

    let mut evaluation_config = MultiAgentEvaluationConfig::default();
    evaluation_config.max_rounds = rounds.max(1);
    evaluation_config.top_k = top_k.max(1).min(agent_pool.len().max(1));
    if let Some(threshold) = improvement_threshold {
        evaluation_config.improvement_threshold = Some(threshold);
    }
    if let Some(risk) = max_risk {
        evaluation_config.max_risk_score = Some(risk);
    }

    let supervisor_config = SupervisorConfig {
        strategy: CoordinationStrategy::Parallel,
        merge_strategy: MergeStrategy::Voting,
        max_parallel_agents: agent_pool.len().max(2),
        ..SupervisorConfig::default()
    };

    let evaluator = MultiAgentEvaluator::new(
        supervisor_config,
        SimpleEvaluationStrategy::default(),
        evaluation_config,
    );

    println!("🤝 Pair programming session");
    println!("   Goal: {goal}");
    println!(
        "   Agents: {}",
        agent_pool
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("   Max rounds: {}", rounds.max(1));
    println!();

    let report = evaluator
        .run(&goal, agent_pool.clone())
        .await
        .context("failed to execute pair programming loop")?;

    display_report(&report);

    if let Some(path) = out {
        persist_report(&report, path)?;
    }

    Ok(())
}

fn dedupe_preserving_order(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn display_report(report: &MultiAgentEvaluationReport) {
    for round in &report.rounds {
        println!("Round {}:", round.round_index + 1);
        println!("  Summary: {}", round.aggregated.summary);
        if round.scores.is_empty() {
            println!("  Scores: <none>");
        } else {
            println!("  Scores:");
            for score in &round.scores {
                let risk = score
                    .risk
                    .map(|value| format!("{value:.2}"))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "    - {} → score {:.2}, risk {}",
                    score.agent_name, score.score, risk
                );
            }
        }
        if round.next_active_agents.is_empty() {
            println!("  Next agents: <none>");
        } else {
            println!(
                "  Next agents: {}",
                round
                    .next_active_agents
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        println!();
    }

    if report.final_agents.is_empty() {
        println!("Final agent set: <none>");
    } else {
        println!(
            "Final agent set: {}",
            report
                .final_agents
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if report.terminated_early {
        println!("ℹ️  Loop ended early based on evaluation thresholds.");
    }
}

fn persist_report(report: &MultiAgentEvaluationReport, path: PathBuf) -> Result<()> {
    let json = to_string_pretty(report).context("failed to serialize pair programming report")?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    println!("📄 Report saved to {}", path.display());
    Ok(())
}
