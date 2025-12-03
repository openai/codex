use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::command_safety::is_dangerous_command::requires_initial_appoval;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use thiserror::Error;
use tokio::fs;

use crate::bash::parse_shell_lc_plain_commands;
use crate::features::Feature;
use crate::features::Features;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ApprovalRequirement;

const FORBIDDEN_REASON: &str = "execpolicy forbids this command";
const PROMPT_REASON: &str = "execpolicy requires approval for this command";
const POLICY_DIR_NAME: &str = "policy";
const POLICY_EXTENSION: &str = "codexpolicy";

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
        source: codex_execpolicy::Error,
    },
}

pub(crate) async fn exec_policy_for(
    features: &Features,
    codex_home: &Path,
) -> Result<Arc<Policy>, ExecPolicyError> {
    if !features.enabled(Feature::ExecPolicy) {
        return Ok(Arc::new(Policy::empty()));
    }

    let policy_dir = codex_home.join(POLICY_DIR_NAME);
    let policy_paths = collect_policy_files(&policy_dir).await?;

    let mut parser = PolicyParser::new();
    for policy_path in &policy_paths {
        let contents =
            fs::read_to_string(policy_path)
                .await
                .map_err(|source| ExecPolicyError::ReadFile {
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
        "loaded execpolicy from {} files in {}",
        policy_paths.len(),
        policy_dir.display()
    );

    Ok(policy)
}

fn evaluate_with_policy(
    policy: &Policy,
    command: &[String],
    approval_policy: AskForApproval,
) -> Option<ApprovalRequirement> {
    let commands = parse_shell_lc_plain_commands(command).unwrap_or_else(|| vec![command.to_vec()]);
    let evaluation = policy.check_multiple(commands.iter());

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
            Decision::Allow => Some(ApprovalRequirement::Skip {
                bypass_sandbox: true,
            }),
        },
        Evaluation::NoMatch { .. } => None,
    }
}

pub(crate) async fn create_approval_requirement_for_command(
    policy: &Policy,
    command: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
) -> ApprovalRequirement {
    if let Some(requirement) = evaluate_with_policy(policy, command, approval_policy) {
        return requirement;
    }

    if requires_initial_appoval(
        approval_policy,
        sandbox_policy,
        command,
        sandbox_permissions,
    ) {
        ApprovalRequirement::NeedsApproval { reason: None }
    } else {
        ApprovalRequirement::Skip {
            bypass_sandbox: false,
        }
    }
}

async fn collect_policy_files(dir: &Path) -> Result<Vec<PathBuf>, ExecPolicyError> {
    let mut read_dir = match fs::read_dir(dir).await {
        Ok(read_dir) => read_dir,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ExecPolicyError::ReadDir {
                dir: dir.to_path_buf(),
                source,
            });
        }
    };

    let mut policy_paths = Vec::new();
    while let Some(entry) =
        read_dir
            .next_entry()
            .await
            .map_err(|source| ExecPolicyError::ReadDir {
                dir: dir.to_path_buf(),
                source,
            })?
    {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .await
            .map_err(|source| ExecPolicyError::ReadDir {
                dir: dir.to_path_buf(),
                source,
            })?;

        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == POLICY_EXTENSION)
            && file_type.is_file()
        {
            policy_paths.push(path);
        }
    }

    policy_paths.sort();

    Ok(policy_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::Feature;
    use crate::features::Features;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn returns_empty_policy_when_feature_disabled() {
        let mut features = Features::with_defaults();
        features.disable(Feature::ExecPolicy);
        let temp_dir = tempdir().expect("create temp dir");

        let policy = exec_policy_for(&features, temp_dir.path())
            .await
            .expect("policy result");

        let commands = [vec!["rm".to_string()]];
        assert!(matches!(
            policy.check_multiple(commands.iter()),
            Evaluation::NoMatch { .. }
        ));
        assert!(!temp_dir.path().join(POLICY_DIR_NAME).exists());
    }

    #[tokio::test]
    async fn collect_policy_files_returns_empty_when_dir_missing() {
        let temp_dir = tempdir().expect("create temp dir");

        let policy_dir = temp_dir.path().join(POLICY_DIR_NAME);
        let files = collect_policy_files(&policy_dir)
            .await
            .expect("collect policy files");

        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn loads_policies_from_policy_subdirectory() {
        let temp_dir = tempdir().expect("create temp dir");
        let policy_dir = temp_dir.path().join(POLICY_DIR_NAME);
        fs::create_dir_all(&policy_dir).expect("create policy dir");
        fs::write(
            policy_dir.join("deny.codexpolicy"),
            r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
        )
        .expect("write policy file");

        let policy = exec_policy_for(&Features::with_defaults(), temp_dir.path())
            .await
            .expect("policy result");
        let command = [vec!["rm".to_string()]];
        assert!(matches!(
            policy.check_multiple(command.iter()),
            Evaluation::Match { .. }
        ));
    }

    #[tokio::test]
    async fn ignores_policies_outside_policy_dir() {
        let temp_dir = tempdir().expect("create temp dir");
        fs::write(
            temp_dir.path().join("root.codexpolicy"),
            r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
        )
        .expect("write policy file");

        let policy = exec_policy_for(&Features::with_defaults(), temp_dir.path())
            .await
            .expect("policy result");
        let command = [vec!["ls".to_string()]];
        assert!(matches!(
            policy.check_multiple(command.iter()),
            Evaluation::NoMatch { .. }
        ));
    }

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

    #[tokio::test]
    async fn approval_requirement_prefers_execpolicy_match() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = parser.build();
        let command = vec!["rm".to_string()];

        let requirement = create_approval_requirement_for_command(
            &policy,
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ApprovalRequirement::NeedsApproval {
                reason: Some(PROMPT_REASON.to_string())
            }
        );
    }

    #[tokio::test]
    async fn approval_requirement_respects_approval_policy() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = parser.build();
        let command = vec!["rm".to_string()];

        let requirement = create_approval_requirement_for_command(
            &policy,
            &command,
            AskForApproval::Never,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ApprovalRequirement::Forbidden {
                reason: PROMPT_REASON.to_string()
            }
        );
    }

    #[tokio::test]
    async fn approval_requirement_falls_back_to_heuristics() {
        let command = vec!["python".to_string()];

        let empty_policy = Policy::empty();
        let requirement = create_approval_requirement_for_command(
            &empty_policy,
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ApprovalRequirement::NeedsApproval { reason: None }
        );
    }
}
