use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_execpolicy2::Decision;
use codex_execpolicy2::Evaluation;
use codex_execpolicy2::Policy;
use codex_execpolicy2::PolicyParser;
use codex_protocol::protocol::AskForApproval;
use thiserror::Error;

use crate::bash::parse_shell_lc_plain_commands;
use crate::features::Feature;
use crate::features::Features;
use crate::tools::sandboxing::ApprovalRequirement;

const FORBIDDEN_REASON: &str = "execpolicy forbids this command";
const PROMPT_REASON: &str = "execpolicy requires approval for this command";

#[derive(Debug, Error)]
pub enum ExecPolicyError {
    #[error("failed to read execpolicy files from {dir}: {source}")]
    ReadDir {
        dir: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read execpolicy file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse execpolicy file {path}: {source}")]
    ParsePolicy {
        path: String,
        source: codex_execpolicy2::Error,
    },
}

pub(crate) fn exec_policy_for(
    features: &Features,
    cwd: &Path,
) -> Result<Option<Arc<Policy>>, ExecPolicyError> {
    if !features.enabled(Feature::ExecPolicyV2) {
        return Ok(None);
    }

    load_policy(cwd)
}

pub(crate) fn commands_for_policy(command: &[String]) -> Vec<Vec<String>> {
    if let Some(commands) = parse_shell_lc_plain_commands(command)
        && !commands.is_empty()
    {
        return commands;
    }

    vec![command.to_vec()]
}

pub(crate) fn evaluate_with_policy(
    policy: &Policy,
    command: &[String],
    approval_policy: AskForApproval,
) -> Option<ApprovalRequirement> {
    let commands = commands_for_policy(command);
    let evaluation = if let [single] = commands.as_slice() {
        policy.check(single)
    } else {
        policy.check_multiple(commands.iter())
    };

    match evaluation {
        Evaluation::Match { decision, .. } => match decision {
            Decision::Forbidden => Some(ApprovalRequirement::Forbidden {
                reason: FORBIDDEN_REASON.to_string(),
            }),
            Decision::Prompt => {
                let reason = PROMPT_REASON.to_string();
                if matches!(approval_policy, AskForApproval::Never) {
                    Some(ApprovalRequirement::Forbidden { reason })
                } else {
                    Some(ApprovalRequirement::NeedsApproval {
                        reason: Some(reason),
                    })
                }
            }
            Decision::Allow => Some(ApprovalRequirement::Skip),
        },
        Evaluation::NoMatch => None,
    }
}

fn load_policy(cwd: &Path) -> Result<Option<Arc<Policy>>, ExecPolicyError> {
    let codex_dir = cwd.join(".codex");
    let entries = match fs::read_dir(&codex_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ExecPolicyError::ReadDir {
                dir: codex_dir,
                source,
            });
        }
    };

    let mut policy_paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ExecPolicyError::ReadDir {
            dir: codex_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "codexpolicy")
            && path.is_file()
        {
            policy_paths.push(path);
        }
    }

    if policy_paths.is_empty() {
        return Ok(None);
    }

    policy_paths.sort();

    let mut parser = PolicyParser::new();
    for policy_path in &policy_paths {
        let contents =
            fs::read_to_string(policy_path).map_err(|source| ExecPolicyError::ReadFile {
                path: policy_path.clone(),
                source,
            })?;
        let identifier = policy_path.to_string_lossy().to_string();
        parser
            .parse(&identifier, &contents)
            .map_err(|source| ExecPolicyError::ParsePolicy {
                path: identifier,
                source,
            })?;
    }

    let policy = Arc::new(parser.build());
    tracing::debug!(
        file_count = policy_paths.len(),
        "loaded execpolicy2 from {}",
        codex_dir.display()
    );

    Ok(Some(policy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;

    #[test]
    fn evaluates_bash_lc_inner_commands() {
        let policy_src = r#"
prefix_rule(pattern=["rm"], decision="forbidden")
"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = parser.build();

        let forbidden_script = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "rm -rf /tmp".to_string(),
        ];

        let requirement =
            evaluate_with_policy(&policy, &forbidden_script, AskForApproval::OnRequest)
                .expect("expected match for forbidden command");

        assert_eq!(
            requirement,
            ApprovalRequirement::Forbidden {
                reason: FORBIDDEN_REASON.to_string()
            }
        );
    }
}
