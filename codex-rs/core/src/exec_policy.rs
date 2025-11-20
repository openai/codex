use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::command_safety::is_dangerous_command::requires_initial_appoval;
use codex_execpolicy2::Decision;
use codex_execpolicy2::Evaluation;
use codex_execpolicy2::Policy;
use codex_execpolicy2::PolicyParser;
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
const DEFAULT_POLICY_FILE_NAME: &str = "default.codexpolicy";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandApproval {
    pub requirement: ApprovalRequirement,
    pub allow_prefix: Option<Vec<String>>,
}

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

#[derive(Debug, Error)]
pub enum ExecPolicyPersistError {
    #[error("failed to persist allow rule in {path}: {source}")]
    WritePolicy {
        path: PathBuf,
        #[source]
        source: codex_execpolicy2::WritePolicyError,
    },

    #[error("failed to persist allow rule: {source}")]
    Join {
        #[from]
        source: tokio::task::JoinError,
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
    commands: &[Vec<String>],
    approval_policy: AskForApproval,
) -> (Evaluation, Option<ApprovalRequirement>) {
    let evaluation = policy.check_multiple(commands.iter());

    let requirement = match evaluation {
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
    };

    (evaluation, requirement)
}

fn matches_additional_allow_prefix(
    commands: &[Vec<String>],
    additional_allow_prefixes: &[Vec<String>],
) -> bool {
    commands.iter().any(|command| {
        additional_allow_prefixes
            .iter()
            .any(|prefix| command.starts_with(prefix))
    })
}

pub(crate) fn create_approval_requirement_for_command(
    policy: &Policy,
    additional_allow_prefixes: &[Vec<String>],
    command: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
) -> CommandApproval {
    let commands = parse_shell_lc_plain_commands(command).unwrap_or_else(|| vec![command.to_vec()]);
    if matches_additional_allow_prefix(&commands, additional_allow_prefixes) {
        return CommandApproval {
            requirement: ApprovalRequirement::Skip,
            allow_prefix: None,
        };
    }

    let (evaluation, requirement) = evaluate_with_policy(policy, &commands, approval_policy);

    if let Some(requirement) = requirement {
        return CommandApproval {
            requirement,
            allow_prefix: None,
        };
    }

    let requirement = if requires_initial_appoval(
        approval_policy,
        sandbox_policy,
        command,
        sandbox_permissions,
    ) {
        ApprovalRequirement::NeedsApproval { reason: None }
    } else {
        ApprovalRequirement::Skip
    };

    let allow_prefix = commands.first().and_then(|first| {
        (commands.len() == 1
            && matches!(&evaluation, Evaluation::NoMatch)
            && matches!(requirement, ApprovalRequirement::NeedsApproval { .. }))
        .then_some(first.clone())
    });

    CommandApproval {
        requirement,
        allow_prefix,
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

pub(crate) async fn persist_allow_rule(
    codex_home: &Path,
    prefix: &[String],
) -> Result<(), ExecPolicyPersistError> {
    let policy_path = codex_home
        .join(POLICY_DIR_NAME)
        .join(DEFAULT_POLICY_FILE_NAME);
    let prefix = prefix.to_vec();
    tokio::task::spawn_blocking(move || {
        codex_execpolicy2::append_prefix_rule(&policy_path, &prefix, Decision::Allow).map_err(
            |source| ExecPolicyPersistError::WritePolicy {
                path: policy_path,
                source,
            },
        )
    })
    .await?
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
            Evaluation::NoMatch
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
            Evaluation::NoMatch
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

        let requirement = create_approval_requirement_for_command(
            &policy,
            &[],
            &forbidden_script,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .requirement;

        assert_eq!(
            requirement,
            ApprovalRequirement::Forbidden {
                reason: FORBIDDEN_REASON.to_string()
            }
        );
    }

    #[test]
    fn approval_requirement_prefers_execpolicy_match() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = parser.build();
        let command = vec!["rm".to_string()];

        let requirement = create_approval_requirement_for_command(
            &policy,
            &[],
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .requirement;

        assert_eq!(
            requirement,
            ApprovalRequirement::NeedsApproval {
                reason: Some(PROMPT_REASON.to_string())
            }
        );
    }

    #[test]
    fn approval_requirement_respects_approval_policy() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = parser.build();
        let command = vec!["rm".to_string()];

        let requirement = create_approval_requirement_for_command(
            &policy,
            &[],
            &command,
            AskForApproval::Never,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .requirement;

        assert_eq!(
            requirement,
            ApprovalRequirement::Forbidden {
                reason: PROMPT_REASON.to_string()
            }
        );
    }

    #[test]
    fn approval_requirement_falls_back_to_heuristics() {
        let command = vec!["python".to_string()];

        let empty_policy = Policy::empty();
        let approval = create_approval_requirement_for_command(
            &empty_policy,
            &[],
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        );

        assert_eq!(
            approval.requirement,
            ApprovalRequirement::NeedsApproval { reason: None }
        );
    }

    #[test]
    fn allow_prefix_is_exposed_for_single_command_without_policy_match() {
        let command = vec!["python".to_string(), "script.py".to_string()];

        let empty_policy = Policy::empty();
        let approval = create_approval_requirement_for_command(
            &empty_policy,
            &[],
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        );

        assert_eq!(approval.allow_prefix, Some(command));
    }

    #[test]
    fn allow_prefix_is_not_exposed_when_policy_prompts() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = parser.build();
        let command = vec!["rm".to_string()];

        let approval = create_approval_requirement_for_command(
            &policy,
            &[],
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        );

        assert!(approval.allow_prefix.is_none());
    }

    #[test]
    fn allow_prefix_is_not_exposed_for_multi_command_scripts() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "rm -rf /tmp && echo done".to_string(),
        ];

        let approval = create_approval_requirement_for_command(
            &Policy::empty(),
            &[],
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        );

        assert!(approval.allow_prefix.is_none());
    }

    #[test]
    fn additional_allow_prefixes_skip_approval() {
        let command = vec!["rg".to_string(), "--files".to_string()];
        let approval = create_approval_requirement_for_command(
            &Policy::empty(),
            std::slice::from_ref(&command),
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        );

        assert_eq!(approval.requirement, ApprovalRequirement::Skip);
    }
}
