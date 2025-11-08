//! Parallel agent delegation command

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_common::CliConfigOverrides;
use codex_core::AuthManager;
use codex_core::agents::AgentRuntime;
use codex_core::agents::AgentStatus;
use codex_core::auth::CODEX_API_KEY_ENV_VAR;
use codex_core::auth::OPENAI_API_KEY_ENV_VAR;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::terminal;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::resolve_runtime_budget;

const DEFAULT_SUBAGENT_RUNTIME_BUDGET: i64 = 200_000;

/// Run the parallel delegate command
pub async fn run_parallel_delegate_command(
    agents: Vec<String>,
    goals: Vec<String>,
    scopes: Vec<Option<PathBuf>>,
    budgets: Vec<Option<usize>>,
    deadline: Option<u64>,
    out: Option<PathBuf>,
    config_overrides: CliConfigOverrides,
) -> Result<()> {
    // エージェント数の検証
    if agents.is_empty() {
        bail!("No agents specified");
    }

    if !goals.is_empty() && goals.len() != agents.len() {
        bail!(
            "Number of goals ({}) must match number of agents ({})",
            goals.len(),
            agents.len()
        );
    }

    // 設定読み込み
    let cli_overrides = config_overrides
        .parse_overrides()
        .map_err(|err| anyhow!("failed to parse -c overrides: {err}"))?;

    let config = Config::load_with_cli_overrides(cli_overrides, ConfigOverrides::default())
        .await
        .context("failed to load configuration")?;
    let config = Arc::new(config);

    let workspace_dir = config.cwd.clone();

    // 認証確認
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        true,
        config.cli_auth_credentials_store_mode,
    );
    let auth_snapshot = auth_manager.auth();

    if config.model_provider.requires_openai_auth
        && auth_snapshot.is_none()
        && std::env::var(OPENAI_API_KEY_ENV_VAR).is_err()
        && std::env::var(CODEX_API_KEY_ENV_VAR).is_err()
    {
        bail!(
            "No authentication credentials found. Run `codex login` or set the {OPENAI_API_KEY_ENV_VAR} environment variable."
        );
    }

    // Runtime初期化
    let conversation_id = ConversationId::default();
    let otel_manager = OtelEventManager::new(
        conversation_id,
        config.model.as_str(),
        config.model_family.slug.as_str(),
        auth_snapshot
            .as_ref()
            .and_then(|auth| auth.get_account_id()),
        auth_snapshot
            .as_ref()
            .and_then(|auth| auth.get_account_email()),
        auth_snapshot.as_ref().map(|auth| auth.mode),
        config.otel.log_user_prompt,
        terminal::user_agent(),
    );

    let runtime_budget = resolve_runtime_budget(&config, DEFAULT_SUBAGENT_RUNTIME_BUDGET);

    let reasoning_effort = config.model_reasoning_effort.unwrap_or_default();
    let reasoning_summary = config.model_reasoning_summary.unwrap_or_default();
    let verbosity = config.model_verbosity.unwrap_or_default();

    let runtime = AgentRuntime::new(
        workspace_dir.clone(),
        runtime_budget,
        Arc::clone(&config),
        Some(Arc::clone(&auth_manager)),
        otel_manager,
        config.model_provider.clone(),
        conversation_id,
        reasoning_effort,
        reasoning_summary,
        verbosity,
    );

    println!("🚀 Starting parallel delegation...");
    println!("   Agents: {:?}", agents);
    if let Some(minutes) = deadline {
        println!("   Deadline: {} minutes", minutes);
    }
    println!();

    // エージェント設定の準備
    let mut agent_configs = Vec::new();
    for (i, agent_name) in agents.iter().enumerate() {
        let goal = goals
            .get(i)
            .cloned()
            .unwrap_or_else(|| "Complete task".to_string());

        let resolved_scope = scopes.get(i).and_then(|opt_scope| {
            opt_scope.as_ref().map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    workspace_dir.join(path)
                }
            })
        });

        let budget = budgets.get(i).and_then(|opt_budget| *opt_budget);

        let mut inputs = HashMap::new();
        inputs.insert("goal".to_string(), goal.clone());
        inputs.insert("workspace".to_string(), workspace_dir.display().to_string());
        if let Some(ref path) = resolved_scope {
            inputs.insert("scope".to_string(), path.display().to_string());
        }

        println!("📋 Agent {}/{}: {}", i + 1, agents.len(), agent_name);
        println!("   Goal: {}", goal);
        if let Some(ref path) = resolved_scope {
            println!("   Scope: {}", path.display());
        }
        if let Some(b) = budget {
            println!("   Budget: {} tokens", b);
        }
        println!();

        agent_configs.push((agent_name.clone(), goal, inputs, budget));
    }

    println!("⏳ Executing {} agents in parallel...", agents.len());
    println!();

    // 並列実行
    let results = runtime
        .delegate_parallel(agent_configs, deadline)
        .await
        .context("parallel agent execution failed")?;

    // 結果表示
    println!("\n📊 Execution Results:");
    let mut success_count = 0;
    for (i, result) in results.iter().enumerate() {
        println!("\n  Agent {}/{}: {}", i + 1, results.len(), agents[i]);
        println!("    Status: {:?}", result.status);
        println!("    Tokens used: {}", result.tokens_used);
        println!("    Duration: {:.2}s", result.duration_secs);

        if result.status == AgentStatus::Completed {
            success_count += 1;
        }

        if !result.artifacts.is_empty() {
            println!("    Artifacts:");
            for artifact in &result.artifacts {
                println!("      - {artifact}");
            }
        }

        if let Some(ref error) = result.error {
            eprintln!("    ⚠️  Error: {error}");
        }
    }

    println!("\n✅ Parallel delegation completed!");
    println!("   Success: {}/{}", success_count, agents.len());

    if let Some(out_file) = out {
        let report = serde_json::json!({
            "agents": agents,
            "results": results,
            "success_count": success_count,
            "total_count": agents.len(),
        });
        std::fs::write(&out_file, serde_json::to_string_pretty(&report)?)
            .context("failed to write results")?;
        println!("\n📄 Results saved to: {}", out_file.display());
    }

    Ok(())
}
