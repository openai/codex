use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::is_dangerous_command::command_might_be_dangerous;
use crate::is_safe_command::is_known_safe_command;
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
use tokio::task::spawn_blocking;

use crate::bash::parse_shell_lc_plain_commands;
use crate::bash::parse_shell_lc_single_command_prefix;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;
use shlex::try_join as shlex_try_join;

const PROMPT_CONFLICT_REASON: &str =
    "approval required by policy, but AskForApproval is set to Never";
const RULES_DIR_NAME: &str = "rules";
const RULE_EXTENSION: &str = "rules";
const DEFAULT_POLICY_FILE: &str = "default.rules";
static BANNED_PREFIX_SUGGESTIONS: &[&[&str]] = &[
    &["python3"],
    &["python3", "-"],
    &["python3", "-c"],
    &["python"],
    &["python", "-"],
    &["python", "-c"],
    &["py"],
    &["py", "-3"],
    &["pythonw"],
    &["pyw"],
    &["pypy"],
    &["pypy3"],
    &["git"],
    &["bash"],
    &["bash", "-lc"],
    &["sh"],
    &["sh", "-c"],
    &["sh", "-lc"],
    &["zsh"],
    &["zsh", "-lc"],
    &["/bin/zsh"],
    &["/bin/zsh", "-lc"],
    &["/bin/bash"],
    &["/bin/bash", "-lc"],
    &["pwsh"],
    &["pwsh", "-Command"],
    &["pwsh", "-c"],
    &["powershell"],
    &["powershell", "-Command"],
    &["powershell", "-c"],
    &["powershell.exe"],
    &["powershell.exe", "-Command"],
    &["powershell.exe", "-c"],
    &["env"],
    &["sudo"],
    &["node"],
    &["node", "-e"],
    &["perl"],
    &["perl", "-e"],
    &["ruby"],
    &["ruby", "-e"],
    &["php"],
    &["php", "-r"],
    &["lua"],
    &["lua", "-e"],
    &["osascript"],
];

fn is_policy_match(rule_match: &RuleMatch) -> bool {
    match rule_match {
        RuleMatch::PrefixRuleMatch { .. } => true,
        RuleMatch::HeuristicsRuleMatch { .. } => false,
    }
}

#[derive(Debug, Error)]
pub enum ExecPolicyError {
    #[error("failed to read rules files from {dir}: {source}")]
    ReadDir {
        dir: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read rules file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse rules file {path}: {source}")]
    ParsePolicy {
        path: String,
        source: codex_execpolicy::Error,
    },
}

#[derive(Debug, Error)]
pub enum ExecPolicyUpdateError {
    #[error("failed to update rules file {path}: {source}")]
    AppendRule { path: PathBuf, source: AmendError },

    #[error("failed to join blocking rules update task: {source}")]
    JoinBlockingTask { source: tokio::task::JoinError },

    #[error("failed to update in-memory rules: {source}")]
    AddRule {
        #[from]
        source: ExecPolicyRuleError,
    },
}

pub(crate) struct ExecPolicyManager {
    policy: ArcSwap<Policy>,
}

pub(crate) struct ExecApprovalRequest<'a> {
    pub(crate) command: &'a [String],
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: &'a SandboxPolicy,
    pub(crate) sandbox_permissions: SandboxPermissions,
    pub(crate) prefix_rule: Option<Vec<String>>,
}

impl ExecPolicyManager {
    pub(crate) fn new(policy: Arc<Policy>) -> Self {
        Self {
            policy: ArcSwap::from(policy),
        }
    }

    pub(crate) async fn load(config_stack: &ConfigLayerStack) -> Result<Self, ExecPolicyError> {
        let (policy, warning) = load_exec_policy_with_warning(config_stack).await?;
        if let Some(err) = warning.as_ref() {
            tracing::warn!("failed to parse rules: {err}");
        }
        Ok(Self::new(Arc::new(policy)))
    }

    pub(crate) fn current(&self) -> Arc<Policy> {
        self.policy.load_full()
    }

    pub(crate) async fn create_exec_approval_requirement_for_command(
        &self,
        req: ExecApprovalRequest<'_>,
    ) -> ExecApprovalRequirement {
        let ExecApprovalRequest {
            command,
            approval_policy,
            sandbox_policy,
            sandbox_permissions,
            prefix_rule,
        } = req;
        let exec_policy = self.current();
        let (commands, used_complex_parsing) = commands_for_exec_policy(command);
        // Keep heredoc prefix parsing for rule evaluation so existing
        // allow/prompt/forbidden rules still apply, but avoid auto-derived
        // amendments when only the heredoc fallback parser matched.
        let auto_amendment_allowed = !used_complex_parsing;
        let exec_policy_fallback = |cmd: &[String]| {
            render_decision_for_unmatched_command(
                approval_policy,
                sandbox_policy,
                cmd,
                sandbox_permissions,
                used_complex_parsing,
            )
        };
        let evaluation = exec_policy.check_multiple(commands.iter(), &exec_policy_fallback);

        let requested_amendment = derive_requested_execpolicy_amendment_from_prefix_rule(
            prefix_rule.as_ref(),
            &evaluation.matched_rules,
        );

        match evaluation.decision {
            Decision::Forbidden => ExecApprovalRequirement::Forbidden {
                reason: derive_forbidden_reason(command, &evaluation),
            },
            Decision::Prompt => {
                if matches!(approval_policy, AskForApproval::Never) {
                    ExecApprovalRequirement::Forbidden {
                        reason: PROMPT_CONFLICT_REASON.to_string(),
                    }
                } else {
                    ExecApprovalRequirement::NeedsApproval {
                        reason: derive_prompt_reason(command, &evaluation),
                        proposed_execpolicy_amendment: requested_amendment.or_else(|| {
                            if auto_amendment_allowed {
                                try_derive_execpolicy_amendment_for_prompt_rules(
                                    &evaluation.matched_rules,
                                )
                            } else {
                                None
                            }
                        }),
                    }
                }
            }
            Decision::Allow => ExecApprovalRequirement::Skip {
                // Bypass sandbox if execpolicy allows the command
                bypass_sandbox: evaluation.matched_rules.iter().any(|rule_match| {
                    is_policy_match(rule_match) && rule_match.decision() == Decision::Allow
                }),
                proposed_execpolicy_amendment: if auto_amendment_allowed {
                    try_derive_execpolicy_amendment_for_allow_rules(&evaluation.matched_rules)
                } else {
                    None
                },
            },
        }
    }

    pub(crate) async fn append_amendment_and_update(
        &self,
        codex_home: &Path,
        amendment: &ExecPolicyAmendment,
    ) -> Result<(), ExecPolicyUpdateError> {
        let policy_path = default_policy_path(codex_home);
        let prefix = amendment.command.clone();
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

        let mut updated_policy = self.current().as_ref().clone();
        updated_policy.add_prefix_rule(&prefix, Decision::Allow)?;
        self.policy.store(Arc::new(updated_policy));
        Ok(())
    }
}

impl Default for ExecPolicyManager {
    fn default() -> Self {
        Self::new(Arc::new(Policy::empty()))
    }
}

pub async fn check_execpolicy_for_warnings(
    config_stack: &ConfigLayerStack,
) -> Result<Option<ExecPolicyError>, ExecPolicyError> {
    let (_, warning) = load_exec_policy_with_warning(config_stack).await?;
    Ok(warning)
}

fn exec_policy_message_for_display(source: &codex_execpolicy::Error) -> String {
    let message = source.to_string();
    if let Some(line) = message
        .lines()
        .find(|line| line.trim_start().starts_with("error: "))
    {
        return line.to_owned();
    }
    if let Some(first_line) = message.lines().next()
        && let Some((_, detail)) = first_line.rsplit_once(": starlark error: ")
    {
        return detail.trim().to_string();
    }

    message
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_starlark_line_from_message(message: &str) -> Option<(PathBuf, usize)> {
    let first_line = message.lines().next()?.trim();
    let (path_and_position, _) = first_line.rsplit_once(": starlark error:")?;

    let mut parts = path_and_position.rsplitn(3, ':');
    let _column = parts.next()?.parse::<usize>().ok()?;
    let line = parts.next()?.parse::<usize>().ok()?;
    let path = PathBuf::from(parts.next()?);

    if line == 0 {
        return None;
    }

    Some((path, line))
}

pub fn format_exec_policy_error_with_source(error: &ExecPolicyError) -> String {
    match error {
        ExecPolicyError::ParsePolicy { path, source } => {
            let rendered_source = source.to_string();
            let structured_location = source
                .location()
                .map(|location| (PathBuf::from(location.path), location.range.start.line));
            let parsed_location = parse_starlark_line_from_message(&rendered_source);
            let location = match (structured_location, parsed_location) {
                (Some((_, 1)), Some((parsed_path, parsed_line))) if parsed_line > 1 => {
                    Some((parsed_path, parsed_line))
                }
                (Some(structured), _) => Some(structured),
                (None, parsed) => parsed,
            };
            let message = exec_policy_message_for_display(source);
            match location {
                Some((path, line)) => {
                    format!(
                        "{}:{}: {} (problem is on or around line {})",
                        path.display(),
                        line,
                        message,
                        line
                    )
                }
                None => format!("{path}: {message}"),
            }
        }
        _ => error.to_string(),
    }
}

async fn load_exec_policy_with_warning(
    config_stack: &ConfigLayerStack,
) -> Result<(Policy, Option<ExecPolicyError>), ExecPolicyError> {
    match load_exec_policy(config_stack).await {
        Ok(policy) => Ok((policy, None)),
        Err(err @ ExecPolicyError::ParsePolicy { .. }) => Ok((Policy::empty(), Some(err))),
        Err(err) => Err(err),
    }
}

pub async fn load_exec_policy(config_stack: &ConfigLayerStack) -> Result<Policy, ExecPolicyError> {
    // Iterate the layers in increasing order of precedence, adding the *.rules
    // from each layer, so that higher-precedence layers can override
    // rules defined in lower-precedence ones.
    let mut policy_paths = Vec::new();
    for layer in config_stack.get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, false) {
        if let Some(config_folder) = layer.config_folder() {
            #[expect(clippy::expect_used)]
            let policy_dir = config_folder.join(RULES_DIR_NAME).expect("safe join");
            let layer_policy_paths = collect_policy_files(&policy_dir).await?;
            policy_paths.extend(layer_policy_paths);
        }
    }
    tracing::trace!(
        policy_paths = ?policy_paths,
        "loaded exec policies"
    );

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
    tracing::debug!("loaded rules from {} files", policy_paths.len());
    tracing::trace!(rules = ?policy, "exec policy rules loaded");

    let Some(requirements_policy) = config_stack.requirements().exec_policy.as_deref() else {
        return Ok(policy);
    };

    let mut combined_rules = policy.rules().clone();
    for (program, rules) in requirements_policy.as_ref().rules().iter_all() {
        for rule in rules {
            combined_rules.insert(program.clone(), rule.clone());
        }
    }

    Ok(Policy::new(combined_rules))
}

/// If a command is not matched by any execpolicy rule, derive a [`Decision`].
pub fn render_decision_for_unmatched_command(
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    command: &[String],
    sandbox_permissions: SandboxPermissions,
    used_complex_parsing: bool,
) -> Decision {
    if is_known_safe_command(command) && !used_complex_parsing {
        return Decision::Allow;
    }

    // On Windows, ReadOnly sandbox is not a real sandbox, so special-case it
    // here.
    let runtime_sandbox_provides_safety =
        cfg!(windows) && matches!(sandbox_policy, SandboxPolicy::ReadOnly { .. });

    // If the command is flagged as dangerous or we have no sandbox protection,
    // we should never allow it to run without user approval.
    //
    // We prefer to prompt the user rather than outright forbid the command,
    // but if the user has explicitly disabled prompts, we must
    // forbid the command.
    if command_might_be_dangerous(command) || runtime_sandbox_provides_safety {
        return if matches!(approval_policy, AskForApproval::Never) {
            Decision::Forbidden
        } else {
            Decision::Prompt
        };
    }

    match approval_policy {
        AskForApproval::Never | AskForApproval::OnFailure => {
            // We allow the command to run, relying on the sandbox for
            // protection.
            Decision::Allow
        }
        AskForApproval::UnlessTrusted => {
            // We already checked `is_known_safe_command(command)` and it
            // returned false, so we must prompt.
            Decision::Prompt
        }
        AskForApproval::OnRequest => {
            match sandbox_policy {
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                    // The user has indicated we should "just run" commands
                    // in their unrestricted environment, so we do so since the
                    // command has not been flagged as dangerous.
                    Decision::Allow
                }
                SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. } => {
                    // In restricted sandboxes (ReadOnly/WorkspaceWrite), do not prompt for
                    // non‑escalated, non‑dangerous commands — let the sandbox enforce
                    // restrictions (e.g., block network/write) without a user prompt.
                    if sandbox_permissions.requires_escalated_permissions() {
                        Decision::Prompt
                    } else {
                        Decision::Allow
                    }
                }
            }
        }
    }
}

fn default_policy_path(codex_home: &Path) -> PathBuf {
    codex_home.join(RULES_DIR_NAME).join(DEFAULT_POLICY_FILE)
}

fn commands_for_exec_policy(command: &[String]) -> (Vec<Vec<String>>, bool) {
    if let Some(commands) = parse_shell_lc_plain_commands(command)
        && !commands.is_empty()
    {
        return (commands, false);
    }

    if let Some(single_command) = parse_shell_lc_single_command_prefix(command) {
        return (vec![single_command], true);
    }

    (vec![command.to_vec()], false)
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

fn derive_requested_execpolicy_amendment_from_prefix_rule(
    prefix_rule: Option<&Vec<String>>,
    matched_rules: &[RuleMatch],
) -> Option<ExecPolicyAmendment> {
    let prefix_rule = prefix_rule?;
    if prefix_rule.is_empty() {
        return None;
    }
    if BANNED_PREFIX_SUGGESTIONS.iter().any(|banned| {
        prefix_rule.len() == banned.len()
            && prefix_rule
                .iter()
                .map(String::as_str)
                .eq(banned.iter().copied())
    }) {
        return None;
    }

    // if any policy rule already matches, don't suggest an additional rule that might conflict or not apply
    if matched_rules.iter().any(is_policy_match) {
        return None;
    }

    Some(ExecPolicyAmendment::new(prefix_rule.clone()))
}

/// Only return a reason when a policy rule drove the prompt decision.
fn derive_prompt_reason(command_args: &[String], evaluation: &Evaluation) -> Option<String> {
    let command = render_shlex_command(command_args);

    let most_specific_prompt = evaluation
        .matched_rules
        .iter()
        .filter_map(|rule_match| match rule_match {
            RuleMatch::PrefixRuleMatch {
                matched_prefix,
                decision: Decision::Prompt,
                justification,
                ..
            } => Some((matched_prefix.len(), justification.as_deref())),
            _ => None,
        })
        .max_by_key(|(matched_prefix_len, _)| *matched_prefix_len);

    match most_specific_prompt {
        Some((_matched_prefix_len, Some(justification))) => {
            Some(format!("`{command}` requires approval: {justification}"))
        }
        Some((_matched_prefix_len, None)) => {
            Some(format!("`{command}` requires approval by policy"))
        }
        None => None,
    }
}

fn render_shlex_command(args: &[String]) -> String {
    shlex_try_join(args.iter().map(String::as_str)).unwrap_or_else(|_| args.join(" "))
}

/// Derive a string explaining why the command was forbidden. If `justification`
/// is set by the user, this can contain instructions with recommended
/// alternatives, for example.
fn derive_forbidden_reason(command_args: &[String], evaluation: &Evaluation) -> String {
    let command = render_shlex_command(command_args);

    let most_specific_forbidden = evaluation
        .matched_rules
        .iter()
        .filter_map(|rule_match| match rule_match {
            RuleMatch::PrefixRuleMatch {
                matched_prefix,
                decision: Decision::Forbidden,
                justification,
                ..
            } => Some((matched_prefix, justification.as_deref())),
            _ => None,
        })
        .max_by_key(|(matched_prefix, _)| matched_prefix.len());

    match most_specific_forbidden {
        Some((_matched_prefix, Some(justification))) => {
            format!("`{command}` rejected: {justification}")
        }
        Some((matched_prefix, None)) => {
            let prefix = render_shlex_command(matched_prefix);
            format!("`{command}` rejected: policy forbids commands starting with `{prefix}`")
        }
        None => format!("`{command}` rejected: blocked by policy"),
    }
}

async fn collect_policy_files(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, ExecPolicyError> {
    let dir = dir.as_ref();
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

    tracing::debug!(
        "loaded {} .rules files in {}",
        policy_paths.len(),
        dir.display()
    );
    Ok(policy_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_loader::ConfigLayerEntry;
    use crate::config_loader::ConfigLayerStack;
    use crate::config_loader::ConfigRequirements;
    use crate::config_loader::ConfigRequirementsToml;
    use codex_app_server_protocol::ConfigLayerSource;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;
    use toml::Value as TomlValue;

    fn config_stack_for_dot_codex_folder(dot_codex_folder: &Path) -> ConfigLayerStack {
        let dot_codex_folder = AbsolutePathBuf::from_absolute_path(dot_codex_folder)
            .expect("absolute dot_codex_folder");
        let layer = ConfigLayerEntry::new(
            ConfigLayerSource::Project { dot_codex_folder },
            TomlValue::Table(Default::default()),
        );
        ConfigLayerStack::new(
            vec![layer],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("ConfigLayerStack")
    }

    #[tokio::test]
    async fn returns_empty_policy_when_no_policy_files_exist() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());

        let manager = ExecPolicyManager::load(&config_stack)
            .await
            .expect("manager result");
        let policy = manager.current();

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
    async fn format_exec_policy_error_with_source_renders_range() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());
        let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
        fs::create_dir_all(&policy_dir).expect("create policy dir");
        let broken_path = policy_dir.join("broken.rules");
        fs::write(
            &broken_path,
            r#"prefix_rule(
    pattern = ["tmux capture-pane"],
    decision = "allow",
    match = ["tmux capture-pane -p"],
)"#,
        )
        .expect("write broken policy file");

        let err = load_exec_policy(&config_stack)
            .await
            .expect_err("expected parse error");
        let rendered = format_exec_policy_error_with_source(&err);

        assert!(rendered.contains("broken.rules:1:"));
        assert!(rendered.contains("on or around line 1"));
    }

    #[test]
    fn parse_starlark_line_from_message_extracts_path_and_line() {
        let parsed = parse_starlark_line_from_message(
            "/tmp/default.rules:143:1: starlark error: error: Parse error: unexpected new line",
        )
        .expect("parse should succeed");

        assert_eq!(parsed.0, PathBuf::from("/tmp/default.rules"));
        assert_eq!(parsed.1, 143);
    }

    #[test]
    fn parse_starlark_line_from_message_rejects_zero_line() {
        let parsed = parse_starlark_line_from_message(
            "/tmp/default.rules:0:1: starlark error: error: Parse error: unexpected new line",
        );
        assert_eq!(parsed, None);
    }

    #[tokio::test]
    async fn loads_policies_from_policy_subdirectory() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());
        let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
        fs::create_dir_all(&policy_dir).expect("create policy dir");
        fs::write(
            policy_dir.join("deny.rules"),
            r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
        )
        .expect("write policy file");

        let policy = load_exec_policy(&config_stack)
            .await
            .expect("policy result");
        let command = [vec!["rm".to_string()]];
        assert_eq!(
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["rm".to_string()],
                    decision: Decision::Forbidden,
                    justification: None,
                }],
            },
            policy.check_multiple(command.iter(), &|_| Decision::Allow)
        );
    }

    #[tokio::test]
    async fn ignores_policies_outside_policy_dir() {
        let temp_dir = tempdir().expect("create temp dir");
        let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());
        fs::write(
            temp_dir.path().join("root.rules"),
            r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
        )
        .expect("write policy file");

        let policy = load_exec_policy(&config_stack)
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
    async fn ignores_rules_from_untrusted_project_layers() -> anyhow::Result<()> {
        let project_dir = tempdir()?;
        let policy_dir = project_dir.path().join(RULES_DIR_NAME);
        fs::create_dir_all(&policy_dir)?;
        fs::write(
            policy_dir.join("untrusted.rules"),
            r#"prefix_rule(pattern=["ls"], decision="forbidden")"#,
        )?;

        let project_dot_codex_folder = AbsolutePathBuf::from_absolute_path(project_dir.path())?;
        let layers = vec![ConfigLayerEntry::new_disabled(
            ConfigLayerSource::Project {
                dot_codex_folder: project_dot_codex_folder,
            },
            TomlValue::Table(Default::default()),
            "marked untrusted",
        )];
        let config_stack = ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )?;

        let policy = load_exec_policy(&config_stack).await?;

        assert_eq!(
            Evaluation {
                decision: Decision::Allow,
                matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                    command: vec!["ls".to_string()],
                    decision: Decision::Allow,
                }],
            },
            policy.check_multiple([vec!["ls".to_string()]].iter(), &|_| Decision::Allow)
        );
        Ok(())
    }

    #[tokio::test]
    async fn loads_policies_from_multiple_config_layers() -> anyhow::Result<()> {
        let user_dir = tempdir()?;
        let project_dir = tempdir()?;

        let user_policy_dir = user_dir.path().join(RULES_DIR_NAME);
        fs::create_dir_all(&user_policy_dir)?;
        fs::write(
            user_policy_dir.join("user.rules"),
            r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
        )?;

        let project_policy_dir = project_dir.path().join(RULES_DIR_NAME);
        fs::create_dir_all(&project_policy_dir)?;
        fs::write(
            project_policy_dir.join("project.rules"),
            r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
        )?;

        let user_config_toml =
            AbsolutePathBuf::from_absolute_path(user_dir.path().join("config.toml"))?;
        let project_dot_codex_folder = AbsolutePathBuf::from_absolute_path(project_dir.path())?;
        let layers = vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: user_config_toml,
                },
                TomlValue::Table(Default::default()),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::Project {
                    dot_codex_folder: project_dot_codex_folder,
                },
                TomlValue::Table(Default::default()),
            ),
        ];
        let config_stack = ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )?;

        let policy = load_exec_policy(&config_stack).await?;

        assert_eq!(
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["rm".to_string()],
                    decision: Decision::Forbidden,
                    justification: None,
                }],
            },
            policy.check_multiple([vec!["rm".to_string()]].iter(), &|_| Decision::Allow)
        );
        assert_eq!(
            Evaluation {
                decision: Decision::Prompt,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["ls".to_string()],
                    decision: Decision::Prompt,
                    justification: None,
                }],
            },
            policy.check_multiple([vec!["ls".to_string()]].iter(), &|_| Decision::Allow)
        );
        Ok(())
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
        let policy = Arc::new(parser.build());

        let forbidden_script = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "rm -rf /some/important/folder".to_string(),
        ];

        let manager = ExecPolicyManager::new(policy);
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &forbidden_script,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::DangerFullAccess,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Forbidden {
                reason: "`bash -lc 'rm -rf /some/important/folder'` rejected: policy forbids commands starting with `rm`".to_string()
            }
        );
    }

    #[test]
    fn commands_for_exec_policy_falls_back_for_empty_shell_script() {
        let command = vec!["bash".to_string(), "-lc".to_string(), "".to_string()];

        assert_eq!(commands_for_exec_policy(&command), (vec![command], false));
    }

    #[test]
    fn commands_for_exec_policy_falls_back_for_whitespace_shell_script() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "  \n\t  ".to_string(),
        ];

        assert_eq!(commands_for_exec_policy(&command), (vec![command], false));
    }

    #[tokio::test]
    async fn evaluates_heredoc_script_against_prefix_rules() {
        let policy_src = r#"prefix_rule(pattern=["python3"], decision="allow")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
            .expect("parse policy");
        let policy = Arc::new(parser.build());
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 <<'PY'\nprint('hello')\nPY".to_string(),
        ];

        let requirement = ExecPolicyManager::new(policy)
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            }
        );
    }

    #[tokio::test]
    async fn omits_auto_amendment_for_heredoc_fallback_prompts() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 <<'PY'\nprint('hello')\nPY".to_string(),
        ];

        let requirement = ExecPolicyManager::default()
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
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
    async fn keeps_requested_amendment_for_heredoc_fallback_prompts() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 <<'PY'\nprint('hello')\nPY".to_string(),
        ];
        let requested_prefix = vec!["python3".to_string(), "-m".to_string(), "pip".to_string()];

        let requirement = ExecPolicyManager::default()
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: Some(requested_prefix.clone()),
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(requested_prefix)),
            }
        );
    }

    #[tokio::test]
    async fn justification_is_included_in_forbidden_exec_approval_requirement() {
        let policy_src = r#"
prefix_rule(
    pattern=["rm"],
    decision="forbidden",
    justification="destructive command",
)
"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
            .expect("parse policy");
        let policy = Arc::new(parser.build());

        let manager = ExecPolicyManager::new(policy);
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &[
                    "rm".to_string(),
                    "-rf".to_string(),
                    "/some/important/folder".to_string(),
                ],
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::DangerFullAccess,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Forbidden {
                reason: "`rm -rf /some/important/folder` rejected: destructive command".to_string()
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
        let policy = Arc::new(parser.build());
        let command = vec!["rm".to_string()];

        let manager = ExecPolicyManager::new(policy);
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::DangerFullAccess,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: Some("`rm` requires approval by policy".to_string()),
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
        let policy = Arc::new(parser.build());
        let command = vec!["rm".to_string()];

        let manager = ExecPolicyManager::new(policy);
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::Never,
                sandbox_policy: &SandboxPolicy::DangerFullAccess,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
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

        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
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
    async fn empty_bash_lc_script_falls_back_to_original_command() {
        let command = vec!["bash".to_string(), "-lc".to_string(), "".to_string()];

        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
            }
        );
    }

    #[tokio::test]
    async fn whitespace_bash_lc_script_falls_back_to_original_command() {
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "  \n\t  ".to_string(),
        ];

        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
            }
        );
    }

    #[tokio::test]
    async fn request_rule_uses_prefix_rule() {
        let command = vec![
            "cargo".to_string(),
            "install".to_string(),
            "cargo-insta".to_string(),
        ];
        let manager = ExecPolicyManager::default();

        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::RequireEscalated,
                prefix_rule: Some(vec!["cargo".to_string(), "install".to_string()]),
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                    "cargo".to_string(),
                    "install".to_string(),
                ])),
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
        let policy = Arc::new(parser.build());
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "apple | orange".to_string(),
        ];

        assert_eq!(
            ExecPolicyManager::new(policy)
                .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                    command: &command,
                    approval_policy: AskForApproval::UnlessTrusted,
                    sandbox_policy: &SandboxPolicy::DangerFullAccess,
                    sandbox_permissions: SandboxPermissions::UseDefault,
                    prefix_rule: None,
                })
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
        let prefix = vec!["echo".to_string(), "hello".to_string()];
        let manager = ExecPolicyManager::default();

        manager
            .append_amendment_and_update(codex_home.path(), &ExecPolicyAmendment::from(prefix))
            .await
            .expect("update policy");
        let updated_policy = manager.current();

        let evaluation = updated_policy.check(
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
        let manager = ExecPolicyManager::default();

        let result = manager
            .append_amendment_and_update(codex_home.path(), &ExecPolicyAmendment::from(vec![]))
            .await;

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

        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
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
    async fn proposed_execpolicy_amendment_is_omitted_when_policy_prompts() {
        let policy_src = r#"prefix_rule(pattern=["rm"], decision="prompt")"#;
        let mut parser = PolicyParser::new();
        parser
            .parse("test.rules", policy_src)
            .expect("parse policy");
        let policy = Arc::new(parser.build());
        let command = vec!["rm".to_string()];

        let manager = ExecPolicyManager::new(policy);
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::DangerFullAccess,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: Some("`rm` requires approval by policy".to_string()),
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
        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
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
        let policy = Arc::new(parser.build());

        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat && apple".to_string(),
        ];

        assert_eq!(
            ExecPolicyManager::new(policy)
                .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                    command: &command,
                    approval_policy: AskForApproval::UnlessTrusted,
                    sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                    sandbox_permissions: SandboxPermissions::UseDefault,
                    prefix_rule: None,
                })
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

        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
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
        let policy = Arc::new(parser.build());
        let command = vec!["echo".to_string(), "safe".to_string()];

        let manager = ExecPolicyManager::new(policy);
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            }
        );
    }

    #[test]
    fn derive_requested_execpolicy_amendment_returns_none_for_missing_prefix_rule() {
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_from_prefix_rule(None, &[])
        );
    }

    #[test]
    fn derive_requested_execpolicy_amendment_returns_none_for_empty_prefix_rule() {
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_from_prefix_rule(Some(&Vec::new()), &[])
        );
    }

    #[test]
    fn derive_requested_execpolicy_amendment_returns_none_for_exact_banned_prefix_rule() {
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_from_prefix_rule(
                Some(&vec!["python".to_string(), "-c".to_string()]),
                &[],
            )
        );
    }

    #[test]
    fn derive_requested_execpolicy_amendment_returns_none_for_windows_and_pypy_variants() {
        for prefix_rule in [
            vec!["py".to_string()],
            vec!["py".to_string(), "-3".to_string()],
            vec!["pythonw".to_string()],
            vec!["pyw".to_string()],
            vec!["pypy".to_string()],
            vec!["pypy3".to_string()],
        ] {
            assert_eq!(
                None,
                derive_requested_execpolicy_amendment_from_prefix_rule(Some(&prefix_rule), &[])
            );
        }
    }

    #[test]
    fn derive_requested_execpolicy_amendment_returns_none_for_shell_and_powershell_variants() {
        for prefix_rule in [
            vec!["bash".to_string(), "-lc".to_string()],
            vec!["sh".to_string(), "-c".to_string()],
            vec!["sh".to_string(), "-lc".to_string()],
            vec!["zsh".to_string(), "-lc".to_string()],
            vec!["/bin/bash".to_string(), "-lc".to_string()],
            vec!["/bin/zsh".to_string(), "-lc".to_string()],
            vec!["pwsh".to_string()],
            vec!["pwsh".to_string(), "-Command".to_string()],
            vec!["pwsh".to_string(), "-c".to_string()],
            vec!["powershell".to_string()],
            vec!["powershell".to_string(), "-Command".to_string()],
            vec!["powershell".to_string(), "-c".to_string()],
            vec!["powershell.exe".to_string()],
            vec!["powershell.exe".to_string(), "-Command".to_string()],
            vec!["powershell.exe".to_string(), "-c".to_string()],
        ] {
            assert_eq!(
                None,
                derive_requested_execpolicy_amendment_from_prefix_rule(Some(&prefix_rule), &[])
            );
        }
    }

    #[test]
    fn derive_requested_execpolicy_amendment_allows_non_exact_banned_prefix_rule_match() {
        let prefix_rule = vec![
            "python".to_string(),
            "-c".to_string(),
            "print('hi')".to_string(),
        ];

        assert_eq!(
            Some(ExecPolicyAmendment::new(prefix_rule.clone())),
            derive_requested_execpolicy_amendment_from_prefix_rule(Some(&prefix_rule), &[])
        );
    }

    #[test]
    fn derive_requested_execpolicy_amendment_returns_none_when_policy_matches() {
        let prefix_rule = vec!["cargo".to_string(), "build".to_string()];

        let matched_rules_prompt = vec![RuleMatch::PrefixRuleMatch {
            matched_prefix: vec!["cargo".to_string()],
            decision: Decision::Prompt,
            justification: None,
        }];
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_from_prefix_rule(
                Some(&prefix_rule),
                &matched_rules_prompt
            ),
            "should return none when prompt policy matches"
        );
        let matched_rules_allow = vec![RuleMatch::PrefixRuleMatch {
            matched_prefix: vec!["cargo".to_string()],
            decision: Decision::Allow,
            justification: None,
        }];
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_from_prefix_rule(
                Some(&prefix_rule),
                &matched_rules_allow
            ),
            "should return none when prompt policy matches"
        );
        let matched_rules_forbidden = vec![RuleMatch::PrefixRuleMatch {
            matched_prefix: vec!["cargo".to_string()],
            decision: Decision::Forbidden,
            justification: None,
        }];
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_from_prefix_rule(
                Some(&prefix_rule),
                &matched_rules_forbidden
            ),
            "should return none when prompt policy matches"
        );
    }

    #[tokio::test]
    async fn dangerous_rm_rf_requires_approval_in_danger_full_access() {
        let command = vec_str(&["rm", "-rf", "/tmp/nonexistent"]);
        let manager = ExecPolicyManager::default();
        let requirement = manager
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: &SandboxPolicy::DangerFullAccess,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
            }
        );
    }

    fn vec_str(items: &[&str]) -> Vec<String> {
        items.iter().map(std::string::ToString::to_string).collect()
    }

    /// Note this test behaves differently on Windows because it exercises an
    /// `if cfg!(windows)` code path in render_decision_for_unmatched_command().
    #[tokio::test]
    async fn verify_approval_requirement_for_unsafe_powershell_command() {
        // `brew install powershell` to run this test on a Mac!
        // Note `pwsh` is required to parse a PowerShell command to see if it
        // is safe.
        if which::which("pwsh").is_err() {
            return;
        }

        let policy = ExecPolicyManager::new(Arc::new(Policy::empty()));
        let permissions = SandboxPermissions::UseDefault;

        // This command should not be run without user approval unless there is
        // a proper sandbox in place to ensure safety.
        let sneaky_command = vec_str(&["pwsh", "-Command", "echo hi @(calc)"]);
        let expected_amendment = Some(ExecPolicyAmendment::new(vec_str(&[
            "pwsh",
            "-Command",
            "echo hi @(calc)",
        ])));
        let (pwsh_approval_reason, expected_req) = if cfg!(windows) {
            (
                r#"On Windows, SandboxPolicy::ReadOnly should be assumed to mean
                that no sandbox is present, so anything that is not "provably
                safe" should require approval."#,
                ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    proposed_execpolicy_amendment: expected_amendment.clone(),
                },
            )
        } else {
            (
                "On non-Windows, rely on the read-only sandbox to prevent harm.",
                ExecApprovalRequirement::Skip {
                    bypass_sandbox: false,
                    proposed_execpolicy_amendment: expected_amendment.clone(),
                },
            )
        };
        assert_eq!(
            expected_req,
            policy
                .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                    command: &sneaky_command,
                    approval_policy: AskForApproval::OnRequest,
                    sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                    sandbox_permissions: permissions,
                    prefix_rule: None,
                })
                .await,
            "{pwsh_approval_reason}"
        );

        // This is flagged as a dangerous command on all platforms.
        let dangerous_command = vec_str(&["rm", "-rf", "/important/data"]);
        assert_eq!(
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec_str(&[
                    "rm",
                    "-rf",
                    "/important/data",
                ]))),
            },
            policy
                .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                    command: &dangerous_command,
                    approval_policy: AskForApproval::OnRequest,
                    sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                    sandbox_permissions: permissions,
                    prefix_rule: None,
                })
                .await,
            r#"On all platforms, a forbidden command should require approval
            (unless AskForApproval::Never is specified)."#
        );

        // A dangerous command should be forbidden if the user has specified
        // AskForApproval::Never.
        assert_eq!(
            ExecApprovalRequirement::Forbidden {
                reason: "`rm -rf /important/data` rejected: blocked by policy".to_string(),
            },
            policy
                .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                    command: &dangerous_command,
                    approval_policy: AskForApproval::Never,
                    sandbox_policy: &SandboxPolicy::new_read_only_policy(),
                    sandbox_permissions: permissions,
                    prefix_rule: None,
                })
                .await,
            r#"On all platforms, a forbidden command should require approval
            (unless AskForApproval::Never is specified)."#
        );
    }
}
