use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use codex_common::CliConfigOverrides;
use codex_core::agents::AgentLoader;
use codex_core::agents::AgentResult;
use codex_core::agents::AgentRuntime;
use codex_core::agents::AgentStatus;
use codex_core::auth::CODEX_API_KEY_ENV_VAR;
use codex_core::auth::OPENAI_API_KEY_ENV_VAR;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::terminal;
use codex_core::AuthManager;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_SUBAGENT_RUNTIME_BUDGET: u64 = 200_000;

pub async fn run_delegate_command(
    config_overrides: CliConfigOverrides,
    agent: String,
    goal: Option<String>,
    scope: Option<PathBuf>,
    budget: Option<usize>,
    deadline: Option<u64>,
    out: Option<PathBuf>,
) -> Result<()> {
    let cli_overrides = config_overrides
        .parse_overrides()
        .map_err(|err| anyhow!("failed to parse -c overrides: {err}"))?;

    let config = Config::load_with_cli_overrides(cli_overrides, ConfigOverrides::default())
        .await
        .context("failed to load configuration")?;
    let config = Arc::new(config);

    let workspace_dir = config.cwd.clone();
    let mut loader = AgentLoader::new(&workspace_dir);
    let agent_definition = match loader.load_by_name(&agent) {
        Ok(def) => def,
        Err(err) => {
            let available = loader.list_available_agents().unwrap_or_default();
            eprintln!("❌ Agent '{agent}' not found");
            if available.is_empty() {
                eprintln!(
                    "   No agents were found under {}",
                    workspace_dir.join(".codex/agents").display()
                );
            } else {
                eprintln!("   Available agents:");
                for name in available {
                    eprintln!("     - {name}");
                }
            }
            return Err(err);
        }
    };

    let auth_manager = AuthManager::shared(config.codex_home.clone(), true);
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

    let conversation_id = ConversationId::default();
    let otel_manager = OtelEventManager::new(
        conversation_id,
        config.model.as_str(),
        config.model_family.slug.as_str(),
        auth_snapshot
            .as_ref()
            .and_then(|auth| auth.get_account_id()),
        auth_snapshot.as_ref().map(|auth| auth.mode),
        config.otel.log_user_prompt,
        terminal::user_agent(),
    );

    let runtime_budget = config
        .model_context_window
        .unwrap_or(DEFAULT_SUBAGENT_RUNTIME_BUDGET)
        .min(usize::MAX as u64) as usize;

    let runtime = AgentRuntime::new(
        workspace_dir.clone(),
        runtime_budget,
        Arc::clone(&config),
        Some(Arc::clone(&auth_manager)),
        otel_manager,
        config.model_provider.clone(),
        conversation_id,
    );

    let resolved_scope = scope.map(|path| {
        if path.is_absolute() {
            path
        } else {
            workspace_dir.join(path)
        }
    });

    let task_goal = goal.unwrap_or_else(|| match &resolved_scope {
        Some(path) => format!("Process files in {}", path.display()),
        None => "Complete delegated task".to_string(),
    });

    let default_agent_budget = agent_definition.policies.context.max_tokens;
    let delegated_budget = budget.unwrap_or(default_agent_budget);

    println!("🤝 Delegating to sub-agent '{agent}'");
    println!("   Agent role: {}", agent_definition.goal);
    println!("   Task goal: {task_goal}");
    if let Some(path) = resolved_scope.as_ref() {
        println!("   Scope: {}", path.display());
    }
    println!("   Token budget: {delegated_budget} (per-agent)");
    if let Some(minutes) = deadline {
        println!("   Deadline: {minutes} minutes");
    }
    if !agent_definition.success_criteria.is_empty() {
        println!("   Success criteria:");
        for criterion in &agent_definition.success_criteria {
            println!("     - {criterion}");
        }
    }

    let mut inputs = HashMap::new();
    inputs.insert("goal".to_string(), task_goal.clone());
    inputs.insert("workspace".to_string(), workspace_dir.display().to_string());
    inputs.insert("budget".to_string(), delegated_budget.to_string());
    if let Some(path) = resolved_scope.as_ref() {
        inputs.insert("scope".to_string(), path.display().to_string());
    }
    if let Some(minutes) = deadline {
        inputs.insert("deadline_minutes".to_string(), minutes.to_string());
    }

    println!("\n🚀 Starting agent execution...");

    let result = runtime
        .delegate(&agent, &task_goal, inputs.clone(), budget, deadline)
        .await
        .context("agent execution failed")?;

    println!("\n📊 Execution summary:");
    println!("   Status: {:?}", result.status);
    println!("   Tokens used: {}", result.tokens_used);
    println!("   Duration: {:.2}s", result.duration_secs);

    if !result.artifacts.is_empty() {
        println!("\n🗂️  Generated artifacts:");
        for artifact in &result.artifacts {
            println!("   - {artifact}");
        }
    }

    if let Some(ref message) = result.error {
        eprintln!("\n⚠️  Agent reported an error: {message}");
    }

    if let Some(report_path) = out.as_ref() {
        let report = DelegateCommandReport {
            agent: agent.clone(),
            goal: task_goal.clone(),
            scope: resolved_scope
                .as_ref()
                .map(|path| path.display().to_string()),
            inputs: inputs.clone(),
            result: result.clone(),
        };
        write_report(report_path, &report)?;
        println!("\n💾 Result saved to: {}", report_path.display());
    }

    match result.status {
        AgentStatus::Completed => Ok(()),
        AgentStatus::Failed => {
            if let Some(err) = result.error {
                bail!("agent '{agent}' failed: {err}");
            }
            bail!("agent '{agent}' failed");
        }
        other => bail!("agent '{agent}' ended with status {other:?}"),
    }
}

fn write_report(path: &Path, report: &DelegateCommandReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create report directory: {}", parent.display())
            })?;
        }
    }

    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, json)
        .with_context(|| format!("failed to write report to {}", path.display()))?;
    Ok(())
}

#[derive(Clone, Serialize)]
struct DelegateCommandReport {
    agent: String,
    goal: String,
    scope: Option<String>,
    inputs: HashMap<String, String>,
    result: AgentResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn report_writer_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let report_path = dir.path().join("reports/agent/report.json");
        let report = DelegateCommandReport {
            agent: "test-agent".to_string(),
            goal: "Test".to_string(),
            scope: None,
            inputs: HashMap::new(),
            result: AgentResult {
                agent_name: "test-agent".to_string(),
                status: AgentStatus::Completed,
                artifacts: vec!["artifacts/output.md".to_string()],
                tokens_used: 42,
                duration_secs: 1.23,
                error: None,
            },
        };

        write_report(&report_path, &report).unwrap();
        let written = fs::read_to_string(report_path).unwrap();
        assert!(written.contains("test-agent"));
    }
}
