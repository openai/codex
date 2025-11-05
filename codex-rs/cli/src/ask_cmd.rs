use anyhow::Context;
use anyhow::Result;
use codex_common::CliConfigOverrides;
use codex_core::agent_interpreter::AgentAction;
use codex_core::agent_interpreter::AgentInterpreter;
use codex_core::agent_interpreter::WebhookServiceKind;
use codex_core::agents::AgentAliases;
use std::path::PathBuf;

pub async fn run_ask_command(
    config_overrides: CliConfigOverrides,
    prompt: String,
    scope: Option<PathBuf>,
    budget: Option<usize>,
    out: Option<PathBuf>,
) -> Result<()> {
    // Load aliases
    let aliases = AgentAliases::load().unwrap_or_default();

    // Check if prompt starts with @mention
    let (agent_name, task) = if AgentAliases::has_mention(&prompt) {
        let (agent, rest) =
            AgentAliases::extract_mention(&prompt).context("Failed to parse @mention")?;
        let resolved = aliases.resolve(agent);
        (resolved, rest.to_string())
    } else {
        // Default to researcher if no @mention
        ("researcher".to_string(), prompt.clone())
    };

    println!("🤖 Using agent: {agent_name}");
    println!("📝 Task: {task}\n");

    // Use the existing delegate logic
    crate::delegate_cmd::run_delegate_command(
        config_overrides,
        agent_name,
        Some(task),
        scope,
        budget,
        None, // deadline
        out,
    )
    .await
}

/// Natural language agent command with AI-powered agent selection
pub async fn run_natural_language_agent(
    config_overrides: CliConfigOverrides,
    prompt: String,
    scope: Option<PathBuf>,
    budget: Option<usize>,
    out: Option<PathBuf>,
) -> Result<()> {
    let interpreter = AgentInterpreter::new();
    let invocation = interpreter
        .parse(&prompt)
        .context("Failed to interpret natural language command")?;

    println!("🧠 Interpreted command:");
    match &invocation.action {
        AgentAction::Delegate { agent } => {
            println!("   Action: delegate to agent '{agent}'");
        }
        AgentAction::AutoOrchestrate => {
            println!("   Action: auto orchestrate multi-agent session");
        }
        AgentAction::DeepResearch {
            use_gemini,
            use_mcp,
        } => {
            let gemini = if *use_gemini { " (Gemini)" } else { "" };
            let mcp = if *use_mcp { " via MCP" } else { "" };
            println!("   Action: deep research{gemini}{mcp}");
        }
        AgentAction::TriggerWebhook { service } => {
            println!("   Action: trigger {:?} webhook", service);
        }
        AgentAction::ListMcpTools => {
            println!("   Action: list configured MCP tools");
        }
    }
    println!("   Confidence: {:.0}%", invocation.confidence * 100.0);
    if !invocation.parameters.is_empty() {
        println!("   Parameters:");
        for (key, value) in &invocation.parameters {
            println!("     {}: {}", key, value);
        }
    }
    println!("   Task: {}\n", invocation.goal);

    let action = invocation.action.clone();
    match action {
        AgentAction::Delegate { agent } => {
            crate::delegate_cmd::run_delegate_command(
                config_overrides,
                agent,
                Some(invocation.goal),
                scope,
                budget,
                None,
                out,
            )
            .await
        }
        AgentAction::AutoOrchestrate => {
            let agents = parse_agents(invocation.parameters.get("agents"));
            let rounds = parse_usize_param(&invocation.parameters, "rounds").unwrap_or(3);
            let top_k = parse_usize_param(&invocation.parameters, "top_k").unwrap_or(2);
            let improvement_threshold = parse_f64_param(&invocation.parameters, "threshold");
            let max_risk = parse_f64_param(&invocation.parameters, "max_risk");

            crate::pair_program_cmd::run_pair_program_command(
                config_overrides,
                invocation.goal,
                agents,
                rounds,
                top_k,
                improvement_threshold,
                max_risk,
                out,
            )
            .await
        }
        AgentAction::DeepResearch {
            use_gemini,
            use_mcp,
        } => {
            let topic = invocation
                .parameters
                .get("topic")
                .cloned()
                .unwrap_or_else(|| invocation.goal.clone());
            let depth = parse_u8_param(&invocation.parameters, "depth").unwrap_or(3);
            let breadth = parse_u8_param(&invocation.parameters, "breadth").unwrap_or(3);
            let budget_tokens =
                parse_usize_param(&invocation.parameters, "budget").unwrap_or(80_000);
            let mcp_url = invocation.parameters.get("mcp_url").cloned();

            crate::research_cmd::run_research_command(
                topic,
                depth,
                breadth,
                budget_tokens,
                true,
                mcp_url,
                false,
                out,
                use_gemini,
                use_mcp,
            )
            .await
        }
        AgentAction::TriggerWebhook { service } => match service {
            WebhookServiceKind::Slack => {
                use crate::webhook_cmd::SlackArgs;
                use crate::webhook_cmd::WebhookCli;
                use crate::webhook_cmd::WebhookSubcommand;

                let text = invocation
                    .parameters
                    .get("message")
                    .cloned()
                    .unwrap_or_else(|| invocation.goal.clone());
                let channel = invocation
                    .parameters
                    .get("channel")
                    .and_then(|value| normalize_channel(value));

                let slack_args = SlackArgs {
                    text,
                    channel,
                    data: None,
                };

                crate::webhook_cmd::run(WebhookCli {
                    config_overrides,
                    subcommand: WebhookSubcommand::Slack(slack_args),
                })
                .await
            }
            WebhookServiceKind::Github => {
                println!("ℹ️ GitHub webhooks require an endpoint and JSON payload. Use `codex webhook github --endpoint <path> --data '<json>'` for full control.");
                Ok(())
            }
            WebhookServiceKind::Custom => {
                println!("ℹ️ Custom webhooks require a URL and payload. Use `codex webhook custom --url <URL> --data '<json>'` to send a request.");
                Ok(())
            }
        },
        AgentAction::ListMcpTools => {
            use crate::mcp_cmd::ListArgs;
            use crate::mcp_cmd::McpCli;
            use crate::mcp_cmd::McpSubcommand;

            McpCli {
                config_overrides,
                subcommand: McpSubcommand::List(ListArgs { json: false }),
            }
            .run()
            .await
        }
    }
}

/// Shortcut command that automatically selects the appropriate agent
pub async fn run_shortcut_command(
    config_overrides: CliConfigOverrides,
    shortcut: &str,
    prompt: String,
    scope: Option<PathBuf>,
    budget: Option<usize>,
    out: Option<PathBuf>,
) -> Result<()> {
    let aliases = AgentAliases::load().unwrap_or_default();
    let agent_name = aliases.resolve(shortcut);

    println!("🚀 Shortcut: {shortcut} → {agent_name}");
    println!("📝 Task: {prompt}\n");

    crate::delegate_cmd::run_delegate_command(
        config_overrides,
        agent_name,
        Some(prompt),
        scope,
        budget,
        None,
        out,
    )
    .await
}

fn parse_agents(value: Option<&String>) -> Vec<String> {
    value
        .map(|raw| {
            raw.split(|c| matches!(c, ',' | ';' | '|'))
                .flat_map(|segment| segment.split(" and "))
                .map(|agent| agent.trim().to_string())
                .filter(|agent| !agent.is_empty())
                .collect()
        })
        .filter(|agents: &Vec<String>| !agents.is_empty())
        .unwrap_or_default()
}

fn parse_usize_param(
    params: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<usize> {
    params.get(key).and_then(|value| value.parse().ok())
}

fn parse_u8_param(params: &std::collections::HashMap<String, String>, key: &str) -> Option<u8> {
    params.get(key).and_then(|value| value.parse().ok())
}

fn parse_f64_param(params: &std::collections::HashMap<String, String>, key: &str) -> Option<f64> {
    params.get(key).and_then(|value| value.parse().ok())
}

fn normalize_channel(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }
    let mut channel = value.trim().to_string();
    if !channel.starts_with('#') && !channel.starts_with('@') {
        channel.insert(0, '#');
    }
    Some(channel)
}
