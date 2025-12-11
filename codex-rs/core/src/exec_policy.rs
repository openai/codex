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
use codex_execpolicy::RuleMatch;
use codex_execpolicy::blocking_append_allow_prefix_rule;
use codex_protocol::approvals::ExecPolicyAmendment;
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
const PROMPT_CONFLICT_REASON: &str =
    "execpolicy requires approval for this command, but AskForApproval is set to Never";
const PROMPT_REASON: &str = "execpolicy requires approval for this command";
const RULES_DIR_NAME: &str = "rules";
const RULE_EXTENSION: &str = "rules";
const DEFAULT_POLICY_FILE: &str = "default.rules";

fn is_policy_match(rule_match: &RuleMatch) -> bool {
    match rule_match {
        RuleMatch::PrefixRuleMatch { .. } => true,
        RuleMatch::HeuristicsRuleMatch { .. } => false,
    }
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

pub(crate) async fn load_exec_policy_for_features(
    features: &Features,
    codex_home: &Path,
) -> Result<Policy, ExecPolicyError> {
    if !features.enabled(Feature::ExecPolicy) {
        Ok(Policy::empty())
    } else {
        load_exec_policy(codex_home).await
    }
}

pub async fn load_exec_policy(codex_home: &Path) -> Result<Policy, ExecPolicyError> {
    let policy_dir = codex_home.join(RULES_DIR_NAME);
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

    let policy = parser.build();
    tracing::debug!(
        "loaded execpolicy from {} files in {}",
        policy_paths.len(),
        policy_dir.display()
    );

    Ok(policy)
}

pub(crate) fn default_policy_path(codex_home: &Path) -> PathBuf {
    codex_home.join(RULES_DIR_NAME).join(DEFAULT_POLICY_FILE)
}

pub(crate) async fn append_execpolicy_amendment_and_update(
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

/// Derive a proposed execpolicy amendment when a command requires user approval
/// - If any execpolicy rule prompts, return None, because an amendment would not skip that policy requirement.
/// - Otherwise return the first heuristics Prompt.
/// - Examples:
/// - execpolicy: empty. Command: `["python"]`. Heuristics prompt -> `Some(vec!["python"])`.
/// - execpolicy: empty. Command: `["bash", "-c", "cd /some/folder && prog1 --option1 arg1 && prog2 --option2 arg2"]`.
///   Parsed commands include `cd /some/folder`, `prog1 --option1 arg1`, and `prog2 --option2 arg2`. If heuristics allow `cd` but prompt
///   on `prog1`, we return `Some(vec!["prog1", "--option1", "arg1"])`.
/// - execpolicy: contains a `prompt for prefix ["prog2"]` rule. For the same command as above,
///   we return `None` because an execpolicy prompt still applies even if we amend execpolicy to allow ["prog1", "--option1", "arg1"].
fn try_derive_execpolicy_amendment_for_prompt_rules(
    matched_rules: &[RuleMatch],
) -> Option<ExecPolicyAmendment> {
    if matched_rules
        .iter()
        .any(|rule_match| is_policy_match(rule_match) && rule_match.decision() == Decision::Prompt)
    {
        return None;
    }

    matched_rules
        .iter()
        .find_map(|rule_match| match rule_match {
            RuleMatch::HeuristicsRuleMatch {
                command,
                decision: Decision::Prompt,
            } => Some(ExecPolicyAmendment::from(command.clone())),
            _ => None,
        })
}

/// - Note: we only use this amendment when the command fails to run in sandbox and codex prompts the user to run outside the sandbox
/// - The purpose of this amendment is to bypass sandbox for similar commands in the future
/// - If any execpolicy rule matches, return None, because we would already be running command outside the sandbox
fn try_derive_execpolicy_amendment_for_allow_rules(
    matched_rules: &[RuleMatch],
) -> Option<ExecPolicyAmendment> {
    if matched_rules.iter().any(is_policy_match) {
        return None;
    }

    matched_rules
        .iter()
        .find_map(|rule_match| match rule_match {
            RuleMatch::HeuristicsRuleMatch {
                command,
                decision: Decision::Allow,
            } => Some(ExecPolicyAmendment::from(command.clone())),
            _ => None,
        })
}

/// Only return PROMPT_REASON when an execpolicy rule drove the prompt decision.
fn derive_prompt_reason(evaluation: &Evaluation) -> Option<String> {
    evaluation.matched_rules.iter().find_map(|rule_match| {
        if is_policy_match(rule_match) && rule_match.decision() == Decision::Prompt {
            Some(PROMPT_REASON.to_string())
        } else {
            None
        }
    })
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
    let heuristics_fallback = |cmd: &[String]| {
        if requires_initial_appoval(approval_policy, sandbox_policy, cmd, sandbox_permissions) {
            Decision::Prompt
        } else {
            Decision::Allow
        }
    };
    let policy = exec_policy.read().await;
    let evaluation = policy.check_multiple(commands.iter(), &heuristics_fallback);

    match evaluation.decision {
        Decision::Forbidden => ExecApprovalRequirement::Forbidden {
            reason: FORBIDDEN_REASON.to_string(),
        },
        Decision::Prompt => {
            if matches!(approval_policy, AskForApproval::Never) {
                ExecApprovalRequirement::Forbidden {
                    reason: PROMPT_CONFLICT_REASON.to_string(),
                }
            } else {
                ExecApprovalRequirement::NeedsApproval {
                    reason: derive_prompt_reason(&evaluation),
                    proposed_execpolicy_amendment: if features.enabled(Feature::ExecPolicy) {
                        try_derive_execpolicy_amendment_for_prompt_rules(&evaluation.matched_rules)
                    } else {
                        None
                    },
                }
            }
        }
        Decision::Allow => ExecApprovalRequirement::Skip {
            // Bypass sandbox if execpolicy allows the command
            bypass_sandbox: evaluation.matched_rules.iter().any(|rule_match| {
                is_policy_match(rule_match) && rule_match.decision() == Decision::Allow
            }),
            proposed_execpolicy_amendment: if features.enabled(Feature::ExecPolicy) {
                try_derive_execpolicy_amendment_for_allow_rules(&evaluation.matched_rules)
            } else {
                None
            },
        },
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
            .is_some_and(|ext| ext == RULE_EXTENSION)
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

        let policy = load_exec_policy_for_features(&features, temp_dir.path())
            .await
            .expect("policy result");

        let commands = [vec!["rm".to_string()]];
        assert_eq!(
            Evaluation {
                decision: Decision::Allow,
                matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                    command: vec!["rm".to_string()],
                    decision: Decision::Allow
                }],
            },
            policy.check_multiple(commands.iter(), &|_| Decision::Allow)
        );
        assert!(!temp_dir.path().join(RULES_DIR_NAME).exists());
    }

    #[tokio::test]
    async fn collect_policy_files_returns_empty_when_dir_missing() {
        let temp_dir = tempdir().expect("create temp dir");

        let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
        let files = collect_policy_files(&policy_dir)
            .await
            .expect("collect policy files");

        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn loads_policies_from_policy_subdirectory() {
        let temp_dir = tempdir().expect("create temp dir");
        let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
        fs::create_dir_all(&policy_dir).expect("create policy dir");
        fs::write(
            policy_dir.join("deny.rules"),
            r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
        )
        .expect("write policy file");

        let policy = load_exec_policy(temp_dir.path())
            .await
            .expect("policy result");
        let command = [vec!["rm".to_string()]];
        assert_eq!(
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["rm".to_string()],
                    decision: Decision::Forbidden
                }],
            },
            policy.check_multiple(command.iter(), &|_| Decision::Allow)
        );
    }

    #[tokio::test]
    async fn ignores_policies_outside_policy_dir() {
        let temp_dir = tempdir().expect("create temp dir");
        fs::write(
            temp_dir.path().join("root.rules"),
            r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
        )
        .expect("write policy file");

        let policy = load_exec_policy(temp_dir.path())
            .await
            .expect("policy result");
        let command = [vec!["ls".to_string()]];
        assert_eq!(
            Evaluation {
                decision: Decision::Allow,
                matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                    command: vec!["ls".to_string()],
                    decision: Decision::Allow
                }],
            },
            policy.check_multiple(command.iter(), &|_| Decision::Allow)
        );
    }

    #[tokio::test]
    async fn evaluates_bash_lc_inner_commands() {
        let policy_src = r#"
prefix_rule(pattern=["rm"], decision="forbidden")
"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
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
            .parse("test.rules", policy_src)
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
                proposed_execpolicy_amendment: None,
            }
        );
    }

    #[tokio::test]
    async fn exec_approval_requirement_respects_approval_policy() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
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
                reason: PROMPT_CONFLICT_REASON.to_string()
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
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command))
            }
        );
    }

    #[tokio::test]
    async fn heuristics_apply_when_other_commands_match_policy() {
        let policy_src = r#"prefix_rule(pattern=["apple"], decision="allow")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "apple | orange".to_string(),
        ];

        assert_eq!(
            create_exec_approval_requirement_for_command(
                &policy,
                &Features::with_defaults(),
                &command,
                AskForApproval::UnlessTrusted,
                &SandboxPolicy::DangerFullAccess,
                SandboxPermissions::UseDefault,
            )
            .await,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                    "orange".to_string()
                ]))
            }
        );
    }

    #[tokio::test]
    async fn append_execpolicy_amendment_updates_policy_and_file() {
        let codex_home = tempdir().expect("create temp dir");
        let current_policy = Arc::new(RwLock::new(Policy::empty()));
        let prefix = vec!["echo".to_string(), "hello".to_string()];

        append_execpolicy_amendment_and_update(codex_home.path(), &current_policy, &prefix)
            .await
            .expect("update policy");

        let evaluation = current_policy.read().await.check(
            &["echo".to_string(), "hello".to_string(), "world".to_string()],
            &|_| Decision::Allow,
        );
        assert!(matches!(
            evaluation,
            Evaluation {
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
    async fn append_execpolicy_amendment_rejects_empty_prefix() {
        let codex_home = tempdir().expect("create temp dir");
        let current_policy = Arc::new(RwLock::new(Policy::empty()));

        let result =
            append_execpolicy_amendment_and_update(codex_home.path(), &current_policy, &[]).await;

        assert!(matches!(
            result,
            Err(ExecPolicyUpdateError::AppendRule {
                source: AmendError::EmptyPrefix,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_is_present_for_single_command_without_policy_match() {
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
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command))
            }
        );
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_is_disabled_when_execpolicy_feature_disabled() {
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
                proposed_execpolicy_amendment: None,
            }
        );
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_is_omitted_when_policy_prompts() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
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
                proposed_execpolicy_amendment: None,
            }
        );
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_is_present_for_multi_command_scripts() {
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
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                    "cargo".to_string(),
                    "build".to_string()
                ])),
            }
        );
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_uses_first_no_match_in_multi_command_scripts() {
        let policy_src = r#"prefix_rule(pattern=["cat"], decision="allow")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));

        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat && apple".to_string(),
        ];

        assert_eq!(
            create_exec_approval_requirement_for_command(
                &policy,
                &Features::with_defaults(),
                &command,
                AskForApproval::UnlessTrusted,
                &SandboxPolicy::ReadOnly,
                SandboxPermissions::UseDefault,
            )
            .await,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                    "apple".to_string()
                ])),
            }
        );
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_is_present_when_heuristics_allow() {
        let command = vec!["echo".to_string(), "safe".to_string()];

        let requirement = create_exec_approval_requirement_for_command(
            &Arc::new(RwLock::new(Policy::empty())),
            &Features::with_defaults(),
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
            }
        );
    }

    #[tokio::test]
    async fn proposed_execpolicy_amendment_is_suppressed_when_policy_matches_allow() {
        let policy_src = r#"prefix_rule(pattern=["echo"], decision="allow")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
            .expect("parse policy");
        let policy = Arc::new(RwLock::new(parser.build()));
        let command = vec!["echo".to_string(), "safe".to_string()];

        let requirement = create_exec_approval_requirement_for_command(
            &policy,
            &Features::with_defaults(),
            &command,
            AskForApproval::OnRequest,
            &SandboxPolicy::ReadOnly,
            SandboxPermissions::UseDefault,
        )
        .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            }
        );
    }
}
