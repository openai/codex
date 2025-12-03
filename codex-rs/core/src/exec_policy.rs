use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::command_safety::is_dangerous_command::requires_initial_appoval;
use codex_execpolicy::AmendError;
use codex_execpolicy::Decision;
use codex_execpolicy::Error as ExecPolicyRuleError;
use codex_execpolicy::Evaluation;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use codex_execpolicy::blocking_append_allow_prefix_rule;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use thiserror::Error;
use tokio::fs;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;

use crate::bash::parse_shell_lc_plain_commands;
use crate::features::Feature;
use crate::features::Features;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;

const FORBIDDEN_REASON: &str = "execpolicy forbids this command";
const PROMPT_REASON: &str = "execpolicy requires approval for this command";
const POLICY_DIR_NAME: &str = "policy";
const POLICY_EXTENSION: &str = "codexpolicy";
const DEFAULT_POLICY_FILE: &str = "default.codexpolicy";

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

#[derive(Debug, Error)]
pub enum ExecPolicyUpdateError {
    #[error("failed to update execpolicy file {path}: {source}")]
    AppendRule { path: PathBuf, source: AmendError },

    #[error("failed to join blocking execpolicy update task: {source}")]
    JoinBlockingTask { source: tokio::task::JoinError },

    #[error("failed to update in-memory execpolicy: {source}")]
    AddRule {
        #[from]
        source: ExecPolicyRuleError,
    },

    #[error("cannot append execpolicy rule because execpolicy feature is disabled")]
    FeatureDisabled,
}

pub(crate) async fn exec_policy_for(
    features: &Features,
    codex_home: &Path,
) -> Result<Arc<RwLock<Policy>>, ExecPolicyError> {
    if !features.enabled(Feature::ExecPolicy) {
        return Ok(Arc::new(RwLock::new(Policy::empty())));
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

    let policy = Arc::new(RwLock::new(parser.build()));
    tracing::debug!(
        "loaded execpolicy from {} files in {}",
        policy_paths.len(),
        policy_dir.display()
    );

    Ok(policy)
}

pub(crate) fn default_policy_path(codex_home: &Path) -> PathBuf {
    codex_home.join(POLICY_DIR_NAME).join(DEFAULT_POLICY_FILE)
}

pub(crate) async fn append_allow_prefix_rule_and_update(
    codex_home: &Path,
    current_policy: &Arc<RwLock<Policy>>,
    prefix: &[String],
) -> Result<(), ExecPolicyUpdateError> {
    let policy_path = default_policy_path(codex_home);
    let prefix = prefix.to_vec();
    spawn_blocking({
        let policy_path = policy_path.clone();
        let prefix = prefix.clone();
        move || blocking_append_allow_prefix_rule(&policy_path, &prefix)
    })
    .await
    .map_err(|source| ExecPolicyUpdateError::JoinBlockingTask { source })?
    .map_err(|source| ExecPolicyUpdateError::AppendRule {
        path: policy_path,
        source,
    })?;

    current_policy
        .write()
        .await
        .add_prefix_rule(&prefix, Decision::Allow)?;

    Ok(())
}

fn requirement_from_decision(
    decision: Decision,
    approval_policy: AskForApproval,
) -> ExecApprovalRequirement {
    match decision {
        Decision::Forbidden => ExecApprovalRequirement::Forbidden {
            reason: FORBIDDEN_REASON.to_string(),
        },
        Decision::Prompt => {
            let reason = PROMPT_REASON.to_string();
            if matches!(approval_policy, AskForApproval::Never) {
                ExecApprovalRequirement::Forbidden { reason }
            } else {
                ExecApprovalRequirement::NeedsApproval {
                    reason: Some(reason),
                    allow_prefix: None,
                }
            }
        }
        Decision::Allow => ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
        },
    }
}

/// Return an allow-prefix option when a single plain command needs approval without
/// any matching policy rule. We only surface the prefix opt-in when execpolicy did
/// not already drive the decision (NoMatch) and when the command is a single
/// unrolled command (multi-part scripts shouldn’t be whitelisted via prefix) and
/// when execpolicy feature is enabled.
fn allow_prefix_if_applicable(
    commands: &[Vec<String>],
    features: &Features,
) -> Option<Vec<String>> {
    if features.enabled(Feature::ExecPolicy) && commands.len() == 1 {
        Some(commands[0].clone())
    } else {
        None
    }
}

pub(crate) async fn create_exec_approval_requirement_for_command(
    exec_policy: &Arc<RwLock<Policy>>,
    features: &Features,
    command: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
) -> ExecApprovalRequirement {
    let commands = parse_shell_lc_plain_commands(command).unwrap_or_else(|| vec![command.to_vec()]);
    let evaluation = exec_policy.read().await.check_multiple(commands.iter());

    match evaluation {
        Evaluation::Match { decision, .. } => requirement_from_decision(decision, approval_policy),
        Evaluation::NoMatch { .. } => {
            if requires_initial_appoval(
                approval_policy,
                sandbox_policy,
                command,
                sandbox_permissions,
            ) {
                ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    allow_prefix: allow_prefix_if_applicable(&commands, features),
                }
            } else {
                ExecApprovalRequirement::Skip {
                    bypass_sandbox: false,
                }
            }
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
    use std::sync::Arc;
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
            policy.read().await.check_multiple(commands.iter()),
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
            policy.read().await.check_multiple(command.iter()),
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
            policy.read().await.check_multiple(command.iter()),
            Evaluation::NoMatch { .. }
        ));
    }

    #[tokio::test]
    async fn evaluates_bash_lc_inner_commands() {
        let policy_src = r#"
prefix_rule(pattern=["rm"], decision="forbidden")
"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));

        let forbidden_script = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "rm -rf /tmp".to_string(),
        ];

        let requirement = create_exec_approval_requirement_for_command(
            &policy,
            &Features::with_defaults(),
            &forbidden_script,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Forbidden {
                reason: FORBIDDEN_REASON.to_string()
            }
        );
    }

    #[tokio::test]
    async fn exec_approval_requirement_prefers_execpolicy_match() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));
        let command = vec!["rm".to_string()];

        let requirement = create_exec_approval_requirement_for_command(
            &policy,
            &Features::with_defaults(),
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: Some(PROMPT_REASON.to_string()),
                allow_prefix: None,
            }
        );
    }

    #[tokio::test]
    async fn exec_approval_requirement_respects_approval_policy() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));
        let command = vec!["rm".to_string()];

        let requirement = create_exec_approval_requirement_for_command(
            &policy,
            &Features::with_defaults(),
            &command,
            AskForApproval::Never,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Forbidden {
                reason: PROMPT_REASON.to_string()
            }
        );
    }

    #[tokio::test]
    async fn exec_approval_requirement_falls_back_to_heuristics() {
        let command = vec!["cargo".to_string(), "build".to_string()];

        let empty_policy = Arc::new(RwLock::new(Policy::empty()));
        let requirement = create_exec_approval_requirement_for_command(
            &empty_policy,
            &Features::with_defaults(),
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                allow_prefix: Some(command)
            }
        );
    }

    #[tokio::test]
    async fn append_allow_prefix_rule_updates_policy_and_file() {
        let codex_home = tempdir().expect("create temp dir");
        let current_policy = Arc::new(RwLock::new(Policy::empty()));
        let prefix = vec!["echo".to_string(), "hello".to_string()];

        append_allow_prefix_rule_and_update(codex_home.path(), &current_policy, &prefix)
            .await
            .expect("update policy");

        let evaluation = current_policy.read().await.check(&[
            "echo".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ]);
        assert!(matches!(
            evaluation,
            Evaluation::Match {
                decision: Decision::Allow,
                ..
            }
        ));

        let contents = fs::read_to_string(default_policy_path(codex_home.path()))
            .expect("policy file should have been created");
        assert_eq!(
            contents,
            r#"prefix_rule(pattern=["echo", "hello"], decision="allow")
"#
        );
    }

    #[tokio::test]
    async fn append_allow_prefix_rule_rejects_empty_prefix() {
        let codex_home = tempdir().expect("create temp dir");
        let current_policy = Arc::new(RwLock::new(Policy::empty()));

        let result =
            append_allow_prefix_rule_and_update(codex_home.path(), &current_policy, &[]).await;

        assert!(matches!(
            result,
            Err(ExecPolicyUpdateError::AppendRule {
                source: AmendError::EmptyPrefix,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn allow_prefix_is_present_for_single_command_without_policy_match() {
        let command = vec!["cargo".to_string(), "build".to_string()];

        let empty_policy = Arc::new(RwLock::new(Policy::empty()));
        let requirement = create_exec_approval_requirement_for_command(
            &empty_policy,
            &Features::with_defaults(),
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                allow_prefix: Some(command)
            }
        );
    }

    #[tokio::test]
    async fn allow_prefix_is_disabled_when_execpolicy_feature_disabled() {
        let command = vec!["cargo".to_string(), "build".to_string()];

        let mut features = Features::with_defaults();
        features.disable(Feature::ExecPolicy);

        let requirement = create_exec_approval_requirement_for_command(
            &Arc::new(RwLock::new(Policy::empty())),
            &features,
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                allow_prefix: None,
            }
        );
    }

    #[tokio::test]
    async fn allow_prefix_is_omitted_when_policy_prompts() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.codexpolicy", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));
        let command = vec!["rm".to_string()];

        let requirement = create_exec_approval_requirement_for_command(
            &policy,
            &Features::with_defaults(),
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::DangerFullAccess,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: Some(PROMPT_REASON.to_string()),
                allow_prefix: None,
            }
        );
    }

    #[tokio::test]
    async fn allow_prefix_is_omitted_for_multi_command_scripts() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cargo build && echo ok".to_string(),
        ];
        let requirement = create_exec_approval_requirement_for_command(
            &Arc::new(RwLock::new(Policy::empty())),
            &Features::with_defaults(),
            &command,
            AskForApproval::UnlessTrusted,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                allow_prefix: None,
            }
        );
    }
}
