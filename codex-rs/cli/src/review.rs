use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_tui::Cli as TuiCli;
use std::path::PathBuf;
use tokio::process::Command;

enum InputKind {
    Branches {
        source: String,
        target: String,
    },
    Pr {
        repo: Option<String>,
        number: String,
    },
}

#[derive(Debug, Parser)]
pub struct ReviewCommand {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Optional subject: either `source->target`, a target branch, or a GitHub PR URL/number
    #[arg(value_name = "SUBJECT")]
    pub subject: Option<String>,

    /// Source branch (defaults to current branch when omitted)
    #[arg(long = "source", short = 's', value_name = "BRANCH")]
    pub source: Option<String>,

    /// Target branch to review merging into
    #[arg(
        long = "target",
        short = 't',
        value_name = "BRANCH",
        visible_alias = "into",
        alias = "to"
    )]
    pub target: Option<String>,
}

pub async fn run_review_command(
    cli: ReviewCommand,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let ReviewCommand {
        mut config_overrides,
        subject,
        source,
        target,
    } = cli;

    let input = if let Some(s) = subject {
        if looks_like_pr(&s) {
            parse_pr_spec(&s)?
        } else if s.contains("->") {
            let (src, dst) = parse_merge_spec(&s)?;
            InputKind::Branches {
                source: src,
                target: dst,
            }
        } else {
            // Fallback: treat as target with default/current source
            let src = source.unwrap_or(current_branch().await?);
            InputKind::Branches {
                source: src,
                target: s,
            }
        }
    } else {
        match (source, target) {
            (_, None) => {
                anyhow::bail!(
                    "the following required arguments were not provided:\n  --target <BRANCH>\n\nUsage: codex review --target <BRANCH> <SUBJECT>"
                )
            }
            (maybe_s, Some(t)) => {
                let src = maybe_s.unwrap_or(current_branch().await?);
                InputKind::Branches {
                    source: src,
                    target: t,
                }
            }
        }
    };

    let (context, patch) = match input {
        InputKind::Branches { source, target } => {
            let patch = git_diff_patch(&target, &source).await?;
            if patch.trim().is_empty() {
                eprintln!("No differences found between {source} and {target}.");
                return Ok(());
            }
            (
                format!("proposed merge of '{source}' into '{target}'"),
                patch,
            )
        }
        InputKind::Pr { repo, number } => {
            let patch = gh_pr_diff(repo.as_deref(), &number).await?;
            if patch.trim().is_empty() {
                eprintln!("No differences found for PR #{number}.");
                return Ok(());
            }
            let context = match repo {
                Some(r) => format!("GitHub PR {r}#{number}"),
                None => format!("GitHub PR #{number}"),
            };
            (context, patch)
        }
    };

    let prompt = build_review_prompt(&context, &patch);

    // Launch the interactive TUI with the constructed prompt, like normal codex.
    let tui_cli = TuiCli {
        prompt: Some(prompt),
        images: vec![],
        model: None,
        oss: false,
        config_profile: None,
        sandbox_mode: None,
        approval_policy: None,
        full_auto: false,
        dangerously_bypass_approvals_and_sandbox: false,
        cwd: None,
        config_overrides: std::mem::take(&mut config_overrides),
    };

    let usage = codex_tui::run_main(tui_cli, codex_linux_sandbox_exe).await?;
    if !usage.is_zero() {
        println!("{}", codex_core::protocol::FinalOutput::from(usage));
    }
    Ok(())
}

fn build_review_prompt(context: &str, patch: &str) -> String {
    let header = format!("You are reviewing a {context}.");
    let instructions = "Please analyze the following git diff patch in detail. Provide: \n\
        - 4 specific areas of improvement (actionable suggestions with rationale).\n\
        - 4 thoughtful questions a senior software engineer would ask.\n\
        Consider correctness, performance, security, readability, testing, and documentation.\n\
        Structure the output with two sections: 'Areas of Improvement' (numbered 1-4) and 'Questions' (numbered 1-4).";
    // Truncate the patch to ~80k tokens (approximate via 4 chars per token)
    let (truncated_patch, was_truncated) = truncate_patch_to_tokens(patch, 80_000);
    let diff_block = if was_truncated {
        format!("```patch\n{truncated_patch}\n...\n```\n")
    } else {
        format!("```patch\n{truncated_patch}\n```\n")
    };
    format!("{header}\n\n{instructions}\n\n{diff_block}")
}

fn truncate_patch_to_tokens(patch: &str, max_tokens: usize) -> (String, bool) {
    // Heuristic: ~4 characters per token
    let max_chars = max_tokens.saturating_mul(4);
    if patch.chars().count() <= max_chars {
        return (patch.to_string(), false);
    }
    let mut out = String::with_capacity(max_chars);
    for (i, ch) in patch.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    (out, true)
}

async fn git_diff_patch(target: &str, source: &str) -> anyhow::Result<String> {
    // git diff target...source shows changes on source since the merge base with target.
    let output = Command::new("git")
        .arg("-c")
        .arg("color.ui=never")
        .arg("diff")
        .arg("--no-ext-diff")
        .arg("--patch")
        .arg("--binary")
        .arg("-M")
        .arg("-C")
        .arg(format!("{target}...{source}"))
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to compute git diff: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn gh_pr_diff(repo: Option<&str>, pr_number: &str) -> anyhow::Result<String> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("diff")
        .arg(pr_number)
        .arg("--patch")
        .arg("--color=never");
    if let Some(r) = repo {
        cmd.arg("--repo").arg(r);
    }
    let output = cmd.output().await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to run 'gh pr diff': {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn looks_like_pr(s: &str) -> bool {
    s.contains("://github.com/") && s.contains("/pull/") || s.chars().all(|c| c.is_ascii_digit())
}

fn parse_pr_spec(spec: &str) -> anyhow::Result<InputKind> {
    if spec.chars().all(|c| c.is_ascii_digit()) {
        return Ok(InputKind::Pr {
            repo: None,
            number: spec.to_string(),
        });
    }
    if let Some(idx) = spec.find("github.com/") {
        let tail = &spec[idx + "github.com/".len()..];
        let parts: Vec<&str> = tail.split('/').collect();
        if parts.len() >= 4 && parts[2] == "pull" {
            let owner = parts[0];
            let repo = parts[1];
            let number = parts[3];
            if !number.chars().all(|c| c.is_ascii_digit()) {
                anyhow::bail!("Invalid PR number in URL: {number}");
            }
            return Ok(InputKind::Pr {
                repo: Some(format!("{owner}/{repo}")),
                number: number.to_string(),
            });
        }
    }
    anyhow::bail!("Unrecognized PR spec: {spec}")
}

fn parse_merge_spec(spec: &str) -> anyhow::Result<(String, String)> {
    let parts: Vec<&str> = spec.split("->").collect();
    if parts.len() != 2 || parts[0].trim().is_empty() || parts[1].trim().is_empty() {
        anyhow::bail!("Invalid merge spec: {spec}. Expected format: source->target");
    }
    Ok((parts[0].trim().to_string(), parts[1].trim().to_string()))
}

async fn current_branch() -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to determine current branch: {stderr}");
    }
    let mut name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() || name == "HEAD" {
        name = "HEAD".to_string();
    }
    Ok(name)
}
