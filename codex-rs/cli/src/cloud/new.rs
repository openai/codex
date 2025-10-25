use std::io::Read;
use std::io::{self};

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Args;
use uuid::Uuid;

use super::context::CloudContext;
use super::context::EnvironmentSummary;

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Cloud environment ID or label (label resolves to ID).
    #[arg(long, value_name = "ENV_ID_OR_LABEL")]
    pub env: String,

    /// Base git reference the task should target (defaults to `main`).
    #[arg(long = "base", default_value = "main")]
    pub base: String,

    /// Use QA mode for the new task.
    #[arg(long)]
    pub qa_mode: bool,

    /// Number of assistant attempts to request (defaults to 1).
    #[arg(long, default_value_t = 1)]
    pub best_of: usize,

    /// Prompt text for the new task. If omitted the prompt is read from stdin.
    #[arg(long)]
    pub prompt: Option<String>,
}

pub async fn run(ctx: &mut CloudContext, args: &NewArgs) -> Result<()> {
    let prompt = match &args.prompt {
        Some(p) => p.clone(),
        None => read_stdin_prompt()?,
    };

    let env_id = resolve_env_id(ctx, &args.env).await?;
    let backend = ctx.backend();

    let created = backend
        .create_task(&env_id, &prompt, &args.base, args.qa_mode, args.best_of)
        .await
        .context("failed to create task")?;

    println!("Created task {}", created.id.0);
    Ok(())
}

fn read_stdin_prompt() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    if buf.trim().is_empty() {
        bail!("prompt is empty");
    }
    Ok(buf)
}

fn looks_like_env_id(value: &str) -> bool {
    if value.starts_with("env_") {
        return true;
    }
    Uuid::parse_str(value).is_ok()
}

pub(crate) async fn resolve_env_id(ctx: &CloudContext, env_arg: &str) -> Result<String> {
    if looks_like_env_id(env_arg) {
        return Ok(env_arg.to_string());
    }

    let envs = ctx
        .list_environments()
        .await
        .context("Failed to list environments from Cloud")?;

    resolve_env_id_from_list(&envs, env_arg)
}

fn resolve_env_id_from_list(envs: &[EnvironmentSummary], env_arg: &str) -> Result<String> {
    if let Some(env) = envs.iter().find(|env| env.id == env_arg) {
        return Ok(env.id.clone());
    }

    let matches: Vec<_> = envs
        .iter()
        .filter(|env| {
            env.label
                .as_deref()
                .map(|label| label == env_arg || label.rsplit('/').next() == Some(env_arg))
                .unwrap_or(false)
        })
        .collect();

    match matches.len() {
        1 => Ok(matches[0].id.clone()),
        0 => bail!(
            "No environment with label '{env_arg}'. Use the TUI to copy the ID, or ensure the label matches exactly (e.g., 'Org/Name')."
        ),
        _ => {
            let hint = matches
                .iter()
                .map(|env| {
                    let label = env.label.as_deref().unwrap_or(&env.id);
                    format!("{label} ({})", env.id)
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!("Ambiguous environment label '{env_arg}'. Candidates: {hint}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_envs() -> Vec<EnvironmentSummary> {
        vec![
            EnvironmentSummary {
                id: "env_abc123".to_string(),
                label: Some("OrgA/prod".to_string()),
            },
            EnvironmentSummary {
                id: "env_def456".to_string(),
                label: Some("OrgA/qa".to_string()),
            },
            EnvironmentSummary {
                id: "env_xyz789".to_string(),
                label: Some("L1nuxOne/ade".to_string()),
            },
            EnvironmentSummary {
                id: "env-A".to_string(),
                label: Some("OrgA/dev".to_string()),
            },
            EnvironmentSummary {
                id: "env_prod999".to_string(),
                label: Some("OrgB/prod".to_string()),
            },
        ]
    }

    #[test]
    fn resolves_env_label_to_id() {
        let envs = sample_envs();
        let resolved = resolve_env_id_from_list(&envs, "L1nuxOne/ade").expect("label");
        pretty_assertions::assert_eq!(resolved, "env_xyz789");
    }

    #[test]
    fn resolves_env_suffix_when_unique() {
        let envs = sample_envs();
        let resolved = resolve_env_id_from_list(&envs, "qa").expect("suffix");
        pretty_assertions::assert_eq!(resolved, "env_def456");
    }

    #[test]
    fn resolves_exact_env_id_even_without_env_prefix() {
        let envs = sample_envs();
        let resolved = resolve_env_id_from_list(&envs, "env-A").expect("id");
        pretty_assertions::assert_eq!(resolved, "env-A");
    }

    #[test]
    fn rejects_ambiguous_env_label() {
        let envs = sample_envs();
        let err = resolve_env_id_from_list(&envs, "prod").expect_err("ambiguous");
        let msg = err.to_string();
        assert!(msg.contains("Ambiguous environment label 'prod'"));
        assert!(msg.contains("OrgA/prod (env_abc123)"));
        assert!(msg.contains("OrgB/prod (env_prod999)"));
    }

    #[test]
    fn detects_obvious_env_id() {
        assert!(looks_like_env_id("env_abc123"));
        assert!(looks_like_env_id("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(!looks_like_env_id("not-an-env"));
    }
}
