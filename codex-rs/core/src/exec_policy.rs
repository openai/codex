use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;

use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_execpolicy::AmendError;
use codex_execpolicy::Decision;
use codex_execpolicy::Error as ExecPolicyRuleError;
use codex_execpolicy::Evaluation;
use codex_execpolicy::MatchOptions;
use codex_execpolicy::NetworkRuleProtocol;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use codex_execpolicy::RuleMatch;
use codex_execpolicy::blocking_append_allow_prefix_rule;
use codex_execpolicy::blocking_append_network_rule;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::protocol::AskForApproval;
use codex_shell_command::is_dangerous_command::command_might_be_dangerous;
use codex_shell_command::is_safe_command::is_known_safe_command;
use thiserror::Error;
use tokio::fs;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::config::Config;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::unsandboxed_execution_allowed;
use codex_shell_command::bash::extract_bash_command;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::bash::parse_shell_lc_single_command_prefix;
use codex_shell_command::bash::try_parse_shell;
use codex_shell_command::shell_detect::ShellType;
use codex_shell_command::shell_detect::detect_shell_type;
use codex_utils_absolute_path::AbsolutePathBuf;
use shlex::try_join as shlex_try_join;

#[cfg(windows)]
#[path = "exec_policy_powershell.rs"]
mod powershell_policy;

const PROMPT_CONFLICT_REASON: &str =
    "approval required by policy, but AskForApproval is set to Never";
const REJECT_SANDBOX_APPROVAL_REASON: &str =
    "approval required by policy, but AskForApproval::Granular.sandbox_approval is false";
const REJECT_RULES_APPROVAL_REASON: &str =
    "approval required by policy rule, but AskForApproval::Granular.rules is false";
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

/// Describes which unmatched-command heuristics should classify the command
/// words being evaluated by exec-policy.
///
/// The command tokens may be the original argv or a shell-specific lowering of
/// a wrapper such as `bash -lc ...` or `powershell.exe -Command ...`. We only
/// need to distinguish the PowerShell case because its safelist and dangerous
/// heuristics operate on PowerShell-flavored inner command words rather than
/// the generic command classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecPolicyCommandOrigin {
    /// Use the generic unmatched-command heuristics.
    Generic,
    #[cfg(windows)]
    /// The command words came from the `-Command` body of a top-level
    /// PowerShell wrapper, so use PowerShell-specific unmatched-command
    /// heuristics for the lowered words.
    PowerShell,
}

#[derive(Clone, Copy)]
pub(crate) struct UnmatchedCommandContext<'a> {
    pub(crate) approval_policy: AskForApproval,
    pub(crate) permission_profile: &'a PermissionProfile,
    pub(crate) windows_sandbox_level: WindowsSandboxLevel,
    pub(crate) sandbox_permissions: SandboxPermissions,
    pub(crate) used_complex_parsing: bool,
    pub(crate) command_origin: ExecPolicyCommandOrigin,
}

#[derive(Debug, Eq, PartialEq)]
struct ExecPolicyCommands {
    commands: Vec<Vec<String>>,
    used_complex_parsing: bool,
    command_origin: ExecPolicyCommandOrigin,
}

const MAX_POSIX_POLICY_DEPTH: usize = 8;
const MAX_POSIX_POLICY_CANDIDATES: usize = 64;
const MAX_POSIX_POLICY_SCRIPT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellRuntimeSource {
    EnvironmentSelected,
    ModelResolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellSelectionSource {
    Configured,
    ModelSelected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShellApprovalProvenance {
    runtime: ShellRuntimeSource,
    selection: ShellSelectionSource,
}

impl ShellApprovalProvenance {
    pub(crate) const fn configured() -> Self {
        Self {
            runtime: ShellRuntimeSource::EnvironmentSelected,
            selection: ShellSelectionSource::Configured,
        }
    }

    pub(crate) const fn local_model_resolved() -> Self {
        Self {
            runtime: ShellRuntimeSource::ModelResolved,
            selection: ShellSelectionSource::ModelSelected,
        }
    }

    pub(crate) const fn remote_model_hint() -> Self {
        Self {
            runtime: ShellRuntimeSource::EnvironmentSelected,
            selection: ShellSelectionSource::ModelSelected,
        }
    }

    const fn requires_outer_policy(self) -> bool {
        matches!(self.runtime, ShellRuntimeSource::ModelResolved)
    }

    pub(crate) const fn is_local_model_resolved(self) -> bool {
        self.requires_outer_policy()
    }

    const fn selection_is_model_supplied(self) -> bool {
        matches!(self.selection, ShellSelectionSource::ModelSelected)
    }

    const fn allows_generated_amendment(self) -> bool {
        !self.selection_is_model_supplied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PosixAnalysisCompleteness {
    Complete,
    Incomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecPolicyCandidateRole {
    UntrustedWrapper,
    InnerCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecPolicyCandidate {
    argv: Vec<String>,
    role: ExecPolicyCandidateRole,
    command_origin: ExecPolicyCommandOrigin,
}

#[derive(Debug, Eq, PartialEq)]
struct PosixPolicyAnalysis {
    candidates: Vec<ExecPolicyCandidate>,
    completeness: PosixAnalysisCompleteness,
    contains_untrusted_wrapper: bool,
    script_bytes: usize,
}

impl PosixPolicyAnalysis {
    fn new() -> Self {
        Self {
            candidates: Vec::new(),
            completeness: PosixAnalysisCompleteness::Complete,
            contains_untrusted_wrapper: false,
            script_bytes: 0,
        }
    }

    fn mark_incomplete(&mut self) {
        self.completeness = PosixAnalysisCompleteness::Incomplete;
    }

    fn push_candidate(&mut self, candidate: ExecPolicyCandidate) -> bool {
        if self.candidates.len() >= MAX_POSIX_POLICY_CANDIDATES {
            self.mark_incomplete();
            return false;
        }
        self.candidates.push(candidate);
        true
    }

    fn add_script_bytes(&mut self, bytes: usize) -> bool {
        let Some(total) = self.script_bytes.checked_add(bytes) else {
            self.mark_incomplete();
            return false;
        };
        if total > MAX_POSIX_POLICY_SCRIPT_BYTES {
            self.mark_incomplete();
            return false;
        }
        self.script_bytes = total;
        true
    }
}

fn is_posix_shell_executable(program: &str) -> bool {
    matches!(
        detect_shell_type(Path::new(program)),
        Some(ShellType::Bash | ShellType::Sh | ShellType::Zsh)
    )
}

fn executable_basename_lowercase(program: &str) -> String {
    let basename = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    basename
        .strip_suffix(".exe")
        .unwrap_or(&basename)
        .to_string()
}

fn executable_spelling_is_absolute(program: &str) -> bool {
    // Intentionally use controller-native path semantics. A spelling that is
    // absolute only for a foreign remote target conservatively cannot establish
    // authority to bypass this controller's sandbox or approval boundary.
    Path::new(program).is_absolute()
}

fn executable_may_hide_nested_execution(program: &str) -> bool {
    let basename = executable_basename_lowercase(program);
    matches!(
        basename.as_str(),
        "." | "source"
            | "trap"
            | "env"
            | "sudo"
            | "doas"
            | "su"
            | "runuser"
            | "command"
            | "nice"
            | "nohup"
            | "timeout"
            | "time"
            | "watch"
            | "chroot"
            | "setsid"
            | "setpriv"
            | "stdbuf"
            | "ionice"
            | "taskset"
            | "exec"
            | "eval"
            | "builtin"
            | "noglob"
            | "nocorrect"
            | "xargs"
            | "parallel"
            | "busybox"
            | "ash"
            | "csh"
            | "dash"
            | "fish"
            | "ksh"
            | "mksh"
            | "rc"
            | "tcsh"
            | "cmd"
            | "powershell"
            | "pwsh"
    )
}

fn transparent_executor_is_query_only(command: &[String]) -> bool {
    let arguments = command.get(1..).unwrap_or_default();
    match command.first().map(String::as_str) {
        Some("command") => {
            arguments.is_empty()
                || arguments
                    .first()
                    .is_some_and(|flag| matches!(flag.as_str(), "-v" | "-V"))
        }
        Some("trap") => {
            arguments.is_empty()
                || matches!(arguments, [flag] if matches!(flag.as_str(), "-l" | "-p"))
        }
        Some(_) | None => false,
    }
}

fn command_may_hide_nested_execution(command: &[String]) -> bool {
    let Some(program) = command.first() else {
        return true;
    };
    let basename = executable_basename_lowercase(program);
    if executable_may_hide_nested_execution(program) {
        return !transparent_executor_is_query_only(command);
    }

    basename == "find"
        && command
            .iter()
            .skip(1)
            .any(|argument| matches!(argument.as_str(), "-exec" | "-execdir" | "-ok" | "-okdir"))
}

/// Returns true when a configured shell body that the strict argv extractor
/// could not fully reduce still has structural evidence of descendant
/// execution. A single complex command (for example, a heredoc or redirect)
/// remains on the legacy configured-shell path, while parse errors, multiple
/// commands, nested shells, and known delegators fail closed when policy rules
/// are active.
fn incomplete_posix_body_may_hide_descendant_execution(command: &[String]) -> bool {
    let Some((_shell, script)) = extract_bash_command(command) else {
        return true;
    };
    if script.len() > MAX_POSIX_POLICY_SCRIPT_BYTES {
        return true;
    }
    let Some(tree) = try_parse_shell(script) else {
        return true;
    };
    let root = tree.root_node();
    if root.has_error() {
        return true;
    }

    let mut stack = vec![root];
    let mut command_count = 0usize;
    while let Some(node) = stack.pop() {
        if matches!(
            node.kind(),
            "c_style_for_statement"
                | "case_statement"
                | "command_substitution"
                | "compound_statement"
                | "for_statement"
                | "function_definition"
                | "if_statement"
                | "process_substitution"
                | "subshell"
                | "while_statement"
        ) {
            return true;
        }
        if node.kind() == "command" {
            command_count += 1;
            if command_count > 1 {
                return true;
            }

            let mut cursor = node.walk();
            let mut command_argv = Vec::new();
            let mut saw_command_name = false;
            let mut has_dynamic_argument = false;
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "command_name" => {
                        let Some(word) = child.named_child(0) else {
                            return true;
                        };
                        if word.kind() != "word" || word.named_child_count() != 0 {
                            return true;
                        }
                        let Ok(program) = word.utf8_text(script.as_bytes()) else {
                            return true;
                        };
                        command_argv.push(program.to_string());
                        saw_command_name = true;
                    }
                    "word" | "number" | "concatenation" | "expansion" | "simple_expansion"
                        if saw_command_name =>
                    {
                        has_dynamic_argument |= matches!(
                            child.kind(),
                            "concatenation" | "expansion" | "simple_expansion"
                        ) || child.named_child_count() != 0;
                        let Ok(argument) = child.utf8_text(script.as_bytes()) else {
                            return true;
                        };
                        command_argv.push(argument.to_string());
                    }
                    "raw_string" | "string" if saw_command_name => {
                        if child.kind() == "string" {
                            let mut argument_cursor = child.walk();
                            has_dynamic_argument |=
                                child.named_children(&mut argument_cursor).any(|part| {
                                    matches!(
                                        part.kind(),
                                        "arithmetic_expansion"
                                            | "command_substitution"
                                            | "expansion"
                                            | "process_substitution"
                                            | "simple_expansion"
                                    )
                                });
                        }
                        let Ok(argument) = child.utf8_text(script.as_bytes()) else {
                            return true;
                        };
                        let unquoted = argument
                            .strip_prefix('\'')
                            .and_then(|argument| argument.strip_suffix('\''))
                            .or_else(|| {
                                argument
                                    .strip_prefix('"')
                                    .and_then(|argument| argument.strip_suffix('"'))
                            })
                            .unwrap_or(argument);
                        command_argv.push(unquoted.to_string());
                    }
                    "variable_assignment" if !saw_command_name => {}
                    "file_redirect" | "heredoc_redirect" => {}
                    _ => return true,
                }
            }
            let Some(program) = command_argv.first() else {
                return true;
            };
            let basename = executable_basename_lowercase(program);
            if has_dynamic_argument
                && (basename == "find"
                    || (executable_may_hide_nested_execution(program)
                        && !transparent_executor_is_query_only(&command_argv)))
            {
                return true;
            }
            if is_posix_shell_executable(program)
                || command_may_hide_nested_execution(&command_argv)
            {
                return true;
            }
        }

        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    false
}

fn analyze_posix_policy(
    command: &[String],
    provenance: ShellApprovalProvenance,
) -> Option<PosixPolicyAnalysis> {
    let program = command.first()?;
    if !is_posix_shell_executable(program) {
        return None;
    }

    let mut analysis = PosixPolicyAnalysis::new();
    analyze_posix_wrapper(
        command,
        provenance.requires_outer_policy(),
        /*depth*/ 0,
        &mut analysis,
    );
    Some(analysis)
}

fn opaque_untrusted_wrapper_analysis(command: &[String]) -> PosixPolicyAnalysis {
    PosixPolicyAnalysis {
        candidates: vec![ExecPolicyCandidate {
            argv: command.to_vec(),
            role: ExecPolicyCandidateRole::UntrustedWrapper,
            command_origin: ExecPolicyCommandOrigin::Generic,
        }],
        completeness: PosixAnalysisCompleteness::Incomplete,
        contains_untrusted_wrapper: true,
        script_bytes: 0,
    }
}

fn analyze_posix_wrapper(
    command: &[String],
    untrusted_wrapper: bool,
    depth: usize,
    analysis: &mut PosixPolicyAnalysis,
) {
    if untrusted_wrapper {
        analysis.contains_untrusted_wrapper = true;
        if !analysis.push_candidate(ExecPolicyCandidate {
            argv: command.to_vec(),
            role: ExecPolicyCandidateRole::UntrustedWrapper,
            command_origin: ExecPolicyCommandOrigin::Generic,
        }) {
            return;
        }
    }

    if depth >= MAX_POSIX_POLICY_DEPTH {
        analysis.mark_incomplete();
        return;
    }

    let Some((_shell, script)) = extract_bash_command(command) else {
        analysis.mark_incomplete();
        return;
    };
    if script.trim().is_empty() || !analysis.add_script_bytes(script.len()) {
        analysis.mark_incomplete();
        return;
    }

    if let Some(commands) = parse_shell_lc_plain_commands(command)
        && !commands.is_empty()
    {
        for inner in commands {
            analyze_posix_inner_command(inner, depth, analysis);
        }
        return;
    }

    if let Some(inner) = parse_shell_lc_single_command_prefix(command) {
        analyze_posix_inner_command(inner, depth, analysis);
    }
    analysis.mark_incomplete();
}

fn analyze_posix_inner_command(
    command: Vec<String>,
    parent_depth: usize,
    analysis: &mut PosixPolicyAnalysis,
) {
    if command.is_empty() {
        analysis.mark_incomplete();
        return;
    }

    if command
        .first()
        .is_some_and(|program| is_posix_shell_executable(program))
    {
        analyze_posix_wrapper(
            &command,
            /*untrusted_wrapper*/ true,
            parent_depth + 1,
            analysis,
        );
        return;
    }

    let may_hide_nested_execution = command_may_hide_nested_execution(&command);
    if !analysis.push_candidate(ExecPolicyCandidate {
        argv: command,
        role: ExecPolicyCandidateRole::InnerCommand,
        command_origin: ExecPolicyCommandOrigin::Generic,
    }) {
        return;
    }
    if may_hide_nested_execution {
        analysis.contains_untrusted_wrapper = true;
        analysis.mark_incomplete();
    }
}

pub(crate) fn child_uses_parent_exec_policy(parent_config: &Config, child_config: &Config) -> bool {
    fn exec_policy_config_folders(config: &Config) -> Vec<AbsolutePathBuf> {
        config
            .config_layer_stack
            .get_layers(
                ConfigLayerStackOrdering::LowestPrecedenceFirst,
                /*include_disabled*/ false,
            )
            .into_iter()
            .filter_map(codex_config::ConfigLayerEntry::config_folder)
            .collect()
    }

    exec_policy_config_folders(parent_config) == exec_policy_config_folders(child_config)
        && parent_config
            .config_layer_stack
            .ignore_user_and_project_exec_policy_rules()
            == child_config
                .config_layer_stack
                .ignore_user_and_project_exec_policy_rules()
        && parent_config.config_layer_stack.requirements().exec_policy
            == child_config.config_layer_stack.requirements().exec_policy
}

fn is_policy_match(rule_match: &RuleMatch) -> bool {
    match rule_match {
        RuleMatch::PrefixRuleMatch { .. } => true,
        RuleMatch::HeuristicsRuleMatch { .. } => false,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PromptCauses {
    rules: bool,
    sandbox: bool,
}

fn prompt_causes(matched_rules: &[RuleMatch]) -> PromptCauses {
    matched_rules
        .iter()
        .fold(PromptCauses::default(), |mut causes, rule_match| {
            match rule_match {
                RuleMatch::PrefixRuleMatch {
                    decision: Decision::Prompt,
                    ..
                } => causes.rules = true,
                RuleMatch::HeuristicsRuleMatch {
                    decision: Decision::Prompt,
                    ..
                } => causes.sandbox = true,
                RuleMatch::PrefixRuleMatch { .. } | RuleMatch::HeuristicsRuleMatch { .. } => {}
            }
            causes
        })
}

/// Returns a rejection reason when `approval_policy` disallows surfacing the
/// current prompt to the user.
///
/// `causes` retains every represented policy-rule and sandbox/escalation
/// category so granular settings are honored independently. When both are
/// present and disabled, policy-rule prompts take precedence.
fn prompt_is_rejected_by_policy(
    approval_policy: AskForApproval,
    causes: PromptCauses,
) -> Option<&'static str> {
    match approval_policy {
        AskForApproval::Never => Some(PROMPT_CONFLICT_REASON),
        AskForApproval::OnRequest => None,
        AskForApproval::UnlessTrusted => None,
        AskForApproval::Granular(granular_config) => {
            if causes.rules && !granular_config.allows_rules_approval() {
                Some(REJECT_RULES_APPROVAL_REASON)
            } else if causes.sandbox && !granular_config.allows_sandbox_approval() {
                Some(REJECT_SANDBOX_APPROVAL_REASON)
            } else {
                None
            }
        }
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
    update_lock: Semaphore,
}

pub(crate) struct ExecApprovalRequest<'a> {
    pub(crate) command: &'a [String],
    pub(crate) approval_policy: AskForApproval,
    pub(crate) permission_profile: PermissionProfile,
    pub(crate) windows_sandbox_level: WindowsSandboxLevel,
    pub(crate) sandbox_permissions: SandboxPermissions,
    pub(crate) prefix_rule: Option<Vec<String>>,
}

impl ExecPolicyManager {
    pub(crate) fn new(policy: Arc<Policy>) -> Self {
        Self {
            policy: ArcSwap::from(policy),
            update_lock: Semaphore::new(/*permits*/ 1),
        }
    }

    #[instrument(level = "info", skip_all)]
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
        let permission_expansion_was_requested =
            req.sandbox_permissions.requests_sandbox_override();
        self.create_exec_approval_requirement_for_configured_command(
            req,
            permission_expansion_was_requested,
        )
        .await
    }

    pub(crate) async fn create_exec_approval_requirement_for_configured_command(
        &self,
        req: ExecApprovalRequest<'_>,
        permission_expansion_was_requested: bool,
    ) -> ExecApprovalRequirement {
        self.create_exec_approval_requirement_for_command_with_provenance(
            req,
            ShellApprovalProvenance::configured(),
            permission_expansion_was_requested,
        )
        .await
    }

    pub(crate) async fn create_exec_approval_requirement_for_command_with_provenance(
        &self,
        req: ExecApprovalRequest<'_>,
        provenance: ShellApprovalProvenance,
        permission_expansion_was_requested: bool,
    ) -> ExecApprovalRequirement {
        let ExecApprovalRequest {
            command,
            approval_policy,
            permission_profile,
            windows_sandbox_level,
            sandbox_permissions,
            prefix_rule,
        } = req;
        let permission_expansion_was_requested =
            permission_expansion_was_requested || sandbox_permissions.requests_sandbox_override();
        let exec_policy = self.current();
        #[cfg(windows)]
        let (parsed_powershell, powershell_outer_authority) =
            match powershell_policy::prepare(command) {
                Some(powershell_policy::PreparedPowerShell::Terminal(requirement)) => {
                    return requirement;
                }
                Some(powershell_policy::PreparedPowerShell::Parsed(parsed)) => {
                    if let Some(outer_argv) = parsed.untrusted_outer_argv() {
                        return create_untrusted_powershell_approval_requirement(
                            exec_policy.as_ref(),
                            outer_argv,
                            parsed.commands(),
                            UnmatchedCommandContext {
                                approval_policy,
                                permission_profile: &permission_profile,
                                windows_sandbox_level,
                                sandbox_permissions,
                                used_complex_parsing: false,
                                command_origin: ExecPolicyCommandOrigin::PowerShell,
                            },
                        );
                    }
                    (Some(parsed), true)
                }
                Some(powershell_policy::PreparedPowerShell::Unsupported) => (None, true),
                None => (None, false),
            };
        #[cfg(windows)]
        let exec_policy_commands = if let Some(parsed) = parsed_powershell.as_ref() {
            ExecPolicyCommands {
                commands: parsed.commands().to_vec(),
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::PowerShell,
            }
        } else {
            commands_for_exec_policy(command)
        };
        #[cfg(not(windows))]
        let exec_policy_commands = commands_for_exec_policy(command);
        #[cfg(not(windows))]
        let powershell_outer_authority = false;

        let posix_analysis = analyze_posix_policy(command, provenance);
        let command_rules_active = exec_policy.rules().iter_all().next().is_some();
        if let Some(analysis) = posix_analysis.as_ref()
            && (provenance.requires_outer_policy()
                || (command_rules_active
                    && (analysis.contains_untrusted_wrapper
                        || (analysis.completeness == PosixAnalysisCompleteness::Incomplete
                            && incomplete_posix_body_may_hide_descendant_execution(command)))))
        {
            return create_untrusted_wrapper_approval_requirement(
                exec_policy.as_ref(),
                command,
                analysis,
                /*allow_exact_opaque_wrapper_for_environment_runtime*/
                !provenance.requires_outer_policy(),
                UnmatchedCommandContext {
                    approval_policy,
                    permission_profile: &permission_profile,
                    windows_sandbox_level,
                    sandbox_permissions,
                    used_complex_parsing: matches!(
                        analysis.completeness,
                        PosixAnalysisCompleteness::Incomplete
                    ),
                    command_origin: ExecPolicyCommandOrigin::Generic,
                },
            );
        }
        if provenance.requires_outer_policy() {
            let analysis = opaque_untrusted_wrapper_analysis(command);
            return create_untrusted_wrapper_approval_requirement(
                exec_policy.as_ref(),
                command,
                &analysis,
                /*allow_exact_opaque_wrapper_for_environment_runtime*/ false,
                UnmatchedCommandContext {
                    approval_policy,
                    permission_profile: &permission_profile,
                    windows_sandbox_level,
                    sandbox_permissions,
                    used_complex_parsing: true,
                    command_origin: ExecPolicyCommandOrigin::Generic,
                },
            );
        }

        let ExecPolicyCommands {
            commands,
            used_complex_parsing,
            command_origin,
        } = exec_policy_commands;
        // Keep heredoc prefix parsing for rule evaluation so existing
        // allow/prompt/forbidden rules still apply, but avoid auto-derived
        // amendments when only the heredoc fallback parser matched.
        let auto_amendment_allowed = !used_complex_parsing
            && provenance.allows_generated_amendment()
            && !permission_expansion_was_requested;
        let exec_policy_fallback = |cmd: &[String]| {
            render_decision_for_unmatched_command(
                cmd,
                UnmatchedCommandContext {
                    approval_policy,
                    permission_profile: &permission_profile,
                    windows_sandbox_level,
                    sandbox_permissions,
                    used_complex_parsing,
                    command_origin,
                },
            )
        };
        let match_options = MatchOptions {
            resolve_host_executables: true,
        };
        let parsed_powershell_outer = powershell_outer_authority.then_some(command);
        let mut evaluation = exec_policy.check_multiple_with_options(
            commands.iter(),
            &exec_policy_fallback,
            &match_options,
        );
        let outer_matches = parsed_powershell_outer
            .map(|outer| {
                exec_policy.matches_for_command_with_options(
                    outer,
                    /*heuristics_fallback*/ None,
                    &match_options,
                )
            })
            .unwrap_or_default();
        let outer_allow = outer_matches.iter().any(|rule_match| {
            matches!(
                rule_match,
                RuleMatch::PrefixRuleMatch {
                    decision: Decision::Allow,
                    ..
                }
            )
        });
        let exact_outer_allow = parsed_powershell_outer.is_some_and(|outer| {
            outer_matches.iter().any(|rule_match| {
                matches!(
                    rule_match,
                    RuleMatch::PrefixRuleMatch {
                        matched_prefix,
                        decision: Decision::Allow,
                        ..
                    } if matched_prefix.len() == outer.len()
                )
            })
        });
        evaluation.matched_rules.extend(outer_matches);
        evaluation.decision = evaluation
            .matched_rules
            .iter()
            .filter(|rule_match| !outer_allow || is_policy_match(rule_match))
            .map(RuleMatch::decision)
            .max()
            .unwrap_or(Decision::Forbidden);

        let every_command_explicit_allow = !commands.is_empty()
            && commands.iter().all(|command| {
                exec_policy
                    .matches_for_command_with_options(
                        command,
                        /*heuristics_fallback*/ None,
                        &match_options,
                    )
                    .iter()
                    .any(|rule_match| {
                        is_policy_match(rule_match) && rule_match.decision() == Decision::Allow
                    })
            });
        let effective_full_policy_authority = evaluation.decision == Decision::Allow
            && parsed_powershell_outer.map_or(every_command_explicit_allow, |_| exact_outer_allow);
        let permission_or_backend_gate =
            permission_delta_requires_outer_authority(&permission_profile, sandbox_permissions)
                || missing_managed_windows_sandbox_backend(
                    &permission_profile,
                    windows_sandbox_level,
                );
        if evaluation.decision != Decision::Forbidden
            && permission_or_backend_gate
            && !effective_full_policy_authority
        {
            evaluation
                .matched_rules
                .push(RuleMatch::HeuristicsRuleMatch {
                    command: command.to_vec(),
                    decision: Decision::Prompt,
                });
            evaluation.decision = evaluation
                .matched_rules
                .iter()
                .map(RuleMatch::decision)
                .max()
                .unwrap_or(Decision::Forbidden);
        }

        let requested_amendment = if parsed_powershell_outer.is_some() {
            None
        } else if auto_amendment_allowed {
            derive_requested_execpolicy_amendment_from_prefix_rule(
                prefix_rule.as_ref(),
                &evaluation.matched_rules,
                exec_policy.as_ref(),
                &commands,
                &exec_policy_fallback,
                &match_options,
            )
        } else {
            None
        };

        let requirement = match evaluation.decision {
            Decision::Forbidden => ExecApprovalRequirement::Forbidden {
                reason: derive_forbidden_reason(command, &evaluation),
            },
            Decision::Prompt => {
                let causes = prompt_causes(&evaluation.matched_rules);
                match prompt_is_rejected_by_policy(approval_policy, causes) {
                    Some(reason) => ExecApprovalRequirement::Forbidden {
                        reason: reason.to_string(),
                    },
                    None => ExecApprovalRequirement::NeedsApproval {
                        reason: derive_prompt_reason(command, &evaluation),
                        proposed_execpolicy_amendment: requested_amendment.or_else(|| {
                            if !auto_amendment_allowed {
                                return None;
                            }
                            match (
                                parsed_powershell_outer,
                                causes.rules,
                                auto_amendment_allowed,
                            ) {
                                (Some(outer), false, _) => {
                                    Some(ExecPolicyAmendment::new(outer.to_vec()))
                                }
                                (None, _, true) => {
                                    try_derive_execpolicy_amendment_for_prompt_rules(
                                        &evaluation.matched_rules,
                                    )
                                }
                                _ => None,
                            }
                        }),
                    },
                }
            }
            Decision::Allow => ExecApprovalRequirement::Skip {
                // Lowered PowerShell command names do not bind runtime module, profile, or PATH
                // resolution. Only effective aggregate Allow plus exact authored authority may
                // authorize sandbox bypass.
                bypass_sandbox: effective_full_policy_authority,
                proposed_execpolicy_amendment: if !auto_amendment_allowed {
                    None
                } else if let Some(outer) = parsed_powershell_outer {
                    (!exact_outer_allow).then(|| ExecPolicyAmendment::new(outer.to_vec()))
                } else {
                    try_derive_execpolicy_amendment_for_allow_rules(&evaluation.matched_rules)
                },
            },
        };
        narrow_requirement_for_shell_provenance(requirement, provenance)
    }

    pub(crate) async fn append_amendment_and_update(
        &self,
        codex_home: &Path,
        amendment: &ExecPolicyAmendment,
    ) -> Result<(), ExecPolicyUpdateError> {
        let _update_guard =
            self.update_lock
                .acquire()
                .await
                .map_err(|_| ExecPolicyUpdateError::AddRule {
                    source: ExecPolicyRuleError::InvalidRule(
                        "exec policy update semaphore closed".to_string(),
                    ),
                })?;
        let policy_path = default_policy_path(codex_home);
        spawn_blocking({
            let policy_path = policy_path.clone();
            let prefix = amendment.command.clone();
            move || blocking_append_allow_prefix_rule(&policy_path, &prefix)
        })
        .await
        .map_err(|source| ExecPolicyUpdateError::JoinBlockingTask { source })?
        .map_err(|source| ExecPolicyUpdateError::AppendRule {
            path: policy_path,
            source,
        })?;

        let current_policy = self.current();
        let match_options = MatchOptions {
            resolve_host_executables: true,
        };
        let existing_evaluation = current_policy.check_multiple_with_options(
            [&amendment.command],
            &|_| Decision::Forbidden,
            &match_options,
        );
        let already_allowed = existing_evaluation.decision == Decision::Allow
            && existing_evaluation.matched_rules.iter().any(|rule_match| {
                is_policy_match(rule_match) && rule_match.decision() == Decision::Allow
            });
        if already_allowed {
            return Ok(());
        }

        let mut updated_policy = current_policy.as_ref().clone();
        updated_policy.add_prefix_rule(&amendment.command, Decision::Allow)?;
        self.policy.store(Arc::new(updated_policy));
        Ok(())
    }

    pub(crate) async fn append_network_rule_and_update(
        &self,
        codex_home: &Path,
        host: &str,
        protocol: NetworkRuleProtocol,
        decision: Decision,
        justification: Option<String>,
    ) -> Result<(), ExecPolicyUpdateError> {
        let _update_guard =
            self.update_lock
                .acquire()
                .await
                .map_err(|_| ExecPolicyUpdateError::AddRule {
                    source: ExecPolicyRuleError::InvalidRule(
                        "exec policy update semaphore closed".to_string(),
                    ),
                })?;
        let policy_path = default_policy_path(codex_home);
        let host = host.to_string();
        spawn_blocking({
            let policy_path = policy_path.clone();
            let host = host.clone();
            let justification = justification.clone();
            move || {
                blocking_append_network_rule(
                    &policy_path,
                    &host,
                    protocol,
                    decision,
                    justification.as_deref(),
                )
            }
        })
        .await
        .map_err(|source| ExecPolicyUpdateError::JoinBlockingTask { source })?
        .map_err(|source| ExecPolicyUpdateError::AppendRule {
            path: policy_path,
            source,
        })?;

        let mut updated_policy = self.current().as_ref().clone();
        updated_policy.add_network_rule(&host, protocol, decision, justification)?;
        self.policy.store(Arc::new(updated_policy));
        Ok(())
    }
}

fn create_untrusted_wrapper_approval_requirement(
    exec_policy: &Policy,
    display_argv: &[String],
    analysis: &PosixPolicyAnalysis,
    allow_exact_opaque_wrapper_for_environment_runtime: bool,
    context: UnmatchedCommandContext<'_>,
) -> ExecApprovalRequirement {
    let match_options = MatchOptions {
        resolve_host_executables: true,
    };
    let mut matched_rules = Vec::new();
    let mut every_candidate_explicit_allow = !analysis.candidates.is_empty();

    // Preserve the parent's PowerShell match ordering (inner commands, then
    // outer wrapper) while applying per-candidate origins and match options.
    for candidate in analysis
        .candidates
        .iter()
        .filter(|candidate| candidate.role == ExecPolicyCandidateRole::InnerCommand)
    {
        let candidate_context = UnmatchedCommandContext {
            command_origin: candidate.command_origin,
            ..context
        };
        let inner_fallback =
            |command: &[String]| render_decision_for_unmatched_command(command, candidate_context);
        let inner_matches = exec_policy.matches_for_command_with_options(
            &candidate.argv,
            Some(&inner_fallback),
            &match_options,
        );
        every_candidate_explicit_allow &= !candidate.argv.is_empty()
            && inner_matches.iter().any(|rule_match| {
                matches!(
                    rule_match,
                    RuleMatch::PrefixRuleMatch {
                        decision: Decision::Allow,
                        ..
                    }
                )
            });
        matched_rules.extend(inner_matches);
    }

    let outer_fallback = |_command: &[String]| match context.approval_policy {
        AskForApproval::Never => Decision::Forbidden,
        AskForApproval::OnRequest | AskForApproval::UnlessTrusted | AskForApproval::Granular(_) => {
            Decision::Prompt
        }
    };
    let raw_match_options = MatchOptions {
        resolve_host_executables: false,
    };
    for candidate in analysis
        .candidates
        .iter()
        .filter(|candidate| candidate.role == ExecPolicyCandidateRole::UntrustedWrapper)
    {
        matched_rules.extend(exec_policy.matches_for_command_with_restrictive_host_rules(
            &candidate.argv,
            Some(&outer_fallback),
        ));
        let raw_full_outer_allow = candidate
            .argv
            .first()
            .is_some_and(|program| executable_spelling_is_absolute(program))
            && exec_policy
                .matches_for_command_with_options(
                    &candidate.argv,
                    /*heuristics_fallback*/ None,
                    &raw_match_options,
                )
                .iter()
                .any(|rule_match| {
                    matches!(
                        rule_match,
                        RuleMatch::PrefixRuleMatch {
                            matched_prefix,
                            decision: Decision::Allow,
                            ..
                        } if matched_prefix.len() == candidate.argv.len()
                    )
                });
        every_candidate_explicit_allow &= raw_full_outer_allow;
    }

    let complete_composed_full_authority = analysis.completeness
        == PosixAnalysisCompleteness::Complete
        && every_candidate_explicit_allow;

    let mut evaluation = Evaluation {
        decision: matched_rules
            .iter()
            .map(RuleMatch::decision)
            .max()
            .unwrap_or(Decision::Forbidden),
        matched_rules,
    };

    // Preserve configured-shell compatibility for a directly authored exact
    // allow of an opaque nested shell argv (for example, `/bin/sh script`).
    // This exception is unavailable to a locally model-resolved outer runtime,
    // and it does not apply when any parsed inner leaf, Prompt, or Forbidden is
    // present.
    let exact_opaque_environment_wrapper_authority =
        allow_exact_opaque_wrapper_for_environment_runtime
            && analysis.completeness == PosixAnalysisCompleteness::Incomplete
            && analysis
                .candidates
                .iter()
                .all(|candidate| candidate.role == ExecPolicyCandidateRole::UntrustedWrapper)
            && every_candidate_explicit_allow
            && evaluation.decision == Decision::Allow;
    let composed_full_authority = evaluation.decision == Decision::Allow
        && (complete_composed_full_authority || exact_opaque_environment_wrapper_authority);

    if analysis.completeness == PosixAnalysisCompleteness::Incomplete
        && evaluation.decision != Decision::Forbidden
        && !exact_opaque_environment_wrapper_authority
    {
        if exec_policy.rules().iter_all().next().is_some() {
            let reason = if analysis.contains_untrusted_wrapper {
                "cannot completely inspect an untrusted shell wrapper"
            } else {
                "cannot completely inspect nested execution in this shell command"
            };
            return ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{}` rejected: {reason} while command policy rules are active",
                    render_shlex_command(display_argv),
                ),
            };
        }
        // With no command rules to conceal, incomplete analysis may proceed
        // only through a callback-scoped prompt. Never let safe-command
        // heuristics turn an opaque wrapper into a sandboxed Skip.
        evaluation
            .matched_rules
            .push(RuleMatch::HeuristicsRuleMatch {
                command: display_argv.to_vec(),
                decision: Decision::Prompt,
            });
        evaluation.decision = Decision::Prompt;
    }

    let permission_or_backend_gate = permission_delta_requires_outer_authority(
        context.permission_profile,
        context.sandbox_permissions,
    ) || missing_managed_windows_sandbox_backend(
        context.permission_profile,
        context.windows_sandbox_level,
    );
    if evaluation.decision != Decision::Forbidden
        && permission_or_backend_gate
        && !composed_full_authority
    {
        evaluation
            .matched_rules
            .push(RuleMatch::HeuristicsRuleMatch {
                command: display_argv.to_vec(),
                decision: Decision::Prompt,
            });
        evaluation.decision = evaluation
            .matched_rules
            .iter()
            .map(RuleMatch::decision)
            .max()
            .unwrap_or(Decision::Forbidden);
    }

    match evaluation.decision {
        Decision::Forbidden => ExecApprovalRequirement::Forbidden {
            reason: derive_forbidden_reason(display_argv, &evaluation),
        },
        Decision::Prompt => {
            match prompt_is_rejected_by_policy(
                context.approval_policy,
                prompt_causes(&evaluation.matched_rules),
            ) {
                Some(reason) => ExecApprovalRequirement::Forbidden {
                    reason: reason.to_string(),
                },
                None => ExecApprovalRequirement::NeedsOneShotApproval {
                    reason: derive_prompt_reason(display_argv, &evaluation),
                },
            }
        }
        Decision::Allow => ExecApprovalRequirement::Skip {
            bypass_sandbox: composed_full_authority,
            proposed_execpolicy_amendment: None,
        },
    }
}

#[cfg(windows)]
fn create_untrusted_powershell_approval_requirement(
    exec_policy: &Policy,
    outer_argv: &[String],
    commands: &[Vec<String>],
    context: UnmatchedCommandContext<'_>,
) -> ExecApprovalRequirement {
    let mut candidates = commands
        .iter()
        .cloned()
        .map(|argv| ExecPolicyCandidate {
            argv,
            role: ExecPolicyCandidateRole::InnerCommand,
            command_origin: context.command_origin,
        })
        .collect::<Vec<_>>();
    candidates.push(ExecPolicyCandidate {
        argv: outer_argv.to_vec(),
        role: ExecPolicyCandidateRole::UntrustedWrapper,
        command_origin: ExecPolicyCommandOrigin::Generic,
    });
    let analysis = PosixPolicyAnalysis {
        candidates,
        completeness: PosixAnalysisCompleteness::Complete,
        contains_untrusted_wrapper: true,
        script_bytes: 0,
    };
    create_untrusted_wrapper_approval_requirement(
        exec_policy,
        outer_argv,
        &analysis,
        /*allow_exact_opaque_wrapper_for_environment_runtime*/ false,
        context,
    )
}

fn narrow_requirement_for_shell_provenance(
    requirement: ExecApprovalRequirement,
    provenance: ShellApprovalProvenance,
) -> ExecApprovalRequirement {
    if !provenance.selection_is_model_supplied() {
        return requirement;
    }

    match requirement {
        ExecApprovalRequirement::NeedsApproval { reason, .. } => {
            ExecApprovalRequirement::NeedsOneShotApproval { reason }
        }
        ExecApprovalRequirement::Skip { bypass_sandbox, .. } => ExecApprovalRequirement::Skip {
            bypass_sandbox,
            proposed_execpolicy_amendment: None,
        },
        ExecApprovalRequirement::NeedsOneShotApproval { .. }
        | ExecApprovalRequirement::Forbidden { .. } => requirement,
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
    // Disabled project layers already represent the trust decision, so hooks
    // and exec-policy loading can reuse the normal trusted-layer view.
    // Iterate the layers in increasing order of precedence, adding the *.rules
    // from each layer, so that higher-precedence layers can override
    // rules defined in lower-precedence ones.
    let mut policy_paths = Vec::new();
    for layer in config_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        if config_stack.ignore_user_and_project_exec_policy_rules()
            && matches!(
                layer.name,
                ConfigLayerSource::User { .. } | ConfigLayerSource::Project { .. }
            )
        {
            continue;
        }
        if let Some(config_folder) = layer.config_folder() {
            let policy_dir = config_folder.join(RULES_DIR_NAME);
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

    Ok(policy.merge_overlay(requirements_policy.as_ref()))
}

/// If a command is not matched by any execpolicy rule, derive a [`Decision`].
pub(crate) fn render_decision_for_unmatched_command(
    command: &[String],
    context: UnmatchedCommandContext<'_>,
) -> Decision {
    let UnmatchedCommandContext {
        approval_policy,
        permission_profile,
        windows_sandbox_level,
        sandbox_permissions,
        used_complex_parsing,
        command_origin,
    } = context;
    let file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();
    let is_known_safe = match command_origin {
        ExecPolicyCommandOrigin::Generic => is_known_safe_command(command),
        #[cfg(windows)]
        ExecPolicyCommandOrigin::PowerShell => {
            codex_shell_command::is_safe_command::is_safe_powershell_words(command)
        }
    };

    // When the Windows sandbox backend is disabled, managed filesystem
    // restrictions are only a policy shape; there is no platform sandbox to
    // enforce the boundary. Keep that legacy case conservative while still
    // relying on the real Windows sandbox when it is enabled.
    let windows_managed_fs_restrictions_without_sandbox_backend = cfg!(windows)
        && windows_sandbox_level == WindowsSandboxLevel::Disabled
        && profile_has_managed_filesystem_restrictions(permission_profile);

    // A requested permission expansion is a separate authority boundary. It
    // must be considered before a known-safe command can inherit trust from
    // the generic safelist. Effective preapproved permissions are normalized
    // to UseDefault before reaching exec-policy.
    if permission_delta_requires_outer_authority(permission_profile, sandbox_permissions) {
        return match approval_policy {
            AskForApproval::Never => Decision::Forbidden,
            AskForApproval::OnRequest
            | AskForApproval::UnlessTrusted
            | AskForApproval::Granular(_) => Decision::Prompt,
        };
    }

    if is_known_safe
        && !used_complex_parsing
        && (approval_policy == AskForApproval::UnlessTrusted
            || windows_managed_fs_restrictions_without_sandbox_backend)
    {
        return Decision::Allow;
    }

    // If the command is flagged as dangerous or we have no sandbox protection,
    // we should never allow it to run without approval.
    //
    // We prefer to prompt the user rather than outright forbid the command,
    // but if the user has explicitly disabled prompts, we must
    // forbid the command.
    let command_is_dangerous = match command_origin {
        ExecPolicyCommandOrigin::Generic => command_might_be_dangerous(command),
        #[cfg(windows)]
        ExecPolicyCommandOrigin::PowerShell => {
            codex_shell_command::is_dangerous_command::is_dangerous_powershell_words(command)
        }
    };
    if command_is_dangerous || windows_managed_fs_restrictions_without_sandbox_backend {
        return match approval_policy {
            AskForApproval::Never => {
                let sandbox_is_explicitly_disabled = matches!(
                    permission_profile,
                    PermissionProfile::Disabled | PermissionProfile::External { .. }
                );
                if sandbox_is_explicitly_disabled {
                    // If the sandbox is explicitly disabled, we should allow the command to run
                    Decision::Allow
                } else {
                    Decision::Forbidden
                }
            }
            AskForApproval::OnRequest
            | AskForApproval::UnlessTrusted
            | AskForApproval::Granular(_) => Decision::Prompt,
        };
    }

    match approval_policy {
        AskForApproval::Never => {
            // We allow the command to run, relying on the sandbox for
            // protection.
            Decision::Allow
        }
        AskForApproval::UnlessTrusted => {
            // We already checked the unmatched-command safelist and it
            // returned false, so we must prompt.
            Decision::Prompt
        }
        AskForApproval::OnRequest => {
            match file_system_sandbox_policy.kind {
                FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
                    // The user has indicated we should "just run" commands
                    // in their unrestricted environment, so we do so since the
                    // command has not been flagged as dangerous.
                    Decision::Allow
                }
                FileSystemSandboxKind::Restricted => {
                    // In restricted sandboxes, do not prompt for non-escalated,
                    // non-dangerous commands; let the sandbox enforce
                    // restrictions without a user prompt.
                    if sandbox_permissions.requests_sandbox_override() {
                        Decision::Prompt
                    } else {
                        Decision::Allow
                    }
                }
            }
        }
        AskForApproval::Granular(_) => match file_system_sandbox_policy.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
                // Mirror on-request behavior for unmatched commands; prompt-vs-reject is handled
                // by `prompt_is_rejected_by_policy`.
                Decision::Allow
            }
            FileSystemSandboxKind::Restricted => {
                if sandbox_permissions.requests_sandbox_override() {
                    Decision::Prompt
                } else {
                    Decision::Allow
                }
            }
        },
    }
}

fn profile_has_managed_filesystem_restrictions(permission_profile: &PermissionProfile) -> bool {
    let file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();
    matches!(permission_profile, PermissionProfile::Managed { .. })
        && matches!(
            file_system_sandbox_policy.kind,
            FileSystemSandboxKind::Restricted
        )
        && !file_system_sandbox_policy.has_full_disk_write_access()
}

fn permission_delta_requires_outer_authority(
    permission_profile: &PermissionProfile,
    sandbox_permissions: SandboxPermissions,
) -> bool {
    match (permission_profile, sandbox_permissions) {
        (PermissionProfile::Disabled, _) => false,
        (_, SandboxPermissions::WithAdditionalPermissions) => true,
        (PermissionProfile::External { .. }, SandboxPermissions::RequireEscalated) => true,
        (PermissionProfile::Managed { .. }, SandboxPermissions::RequireEscalated) => {
            unsandboxed_execution_allowed(&permission_profile.file_system_sandbox_policy())
        }
        (_, SandboxPermissions::UseDefault) => false,
    }
}

fn missing_managed_windows_sandbox_backend(
    permission_profile: &PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
) -> bool {
    cfg!(windows)
        && windows_sandbox_level == WindowsSandboxLevel::Disabled
        && profile_has_managed_filesystem_restrictions(permission_profile)
}

fn default_policy_path(codex_home: &Path) -> PathBuf {
    codex_home.join(RULES_DIR_NAME).join(DEFAULT_POLICY_FILE)
}

fn commands_for_exec_policy(command: &[String]) -> ExecPolicyCommands {
    if let Some(commands) = parse_shell_lc_plain_commands(command)
        && !commands.is_empty()
    {
        return ExecPolicyCommands {
            commands,
            used_complex_parsing: false,
            command_origin: ExecPolicyCommandOrigin::Generic,
        };
    }

    if let Some(single_command) = parse_shell_lc_single_command_prefix(command) {
        return ExecPolicyCommands {
            commands: vec![single_command],
            used_complex_parsing: true,
            command_origin: ExecPolicyCommandOrigin::Generic,
        };
    }

    ExecPolicyCommands {
        commands: vec![command.to_vec()],
        used_complex_parsing: false,
        command_origin: ExecPolicyCommandOrigin::Generic,
    }
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
    exec_policy: &Policy,
    commands: &[Vec<String>],
    exec_policy_fallback: &impl Fn(&[String]) -> Decision,
    match_options: &MatchOptions,
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

    let amendment = ExecPolicyAmendment::new(prefix_rule.clone());
    if prefix_rule_would_approve_all_commands(
        exec_policy,
        &amendment.command,
        commands,
        exec_policy_fallback,
        match_options,
    ) {
        Some(amendment)
    } else {
        None
    }
}

fn prefix_rule_would_approve_all_commands(
    exec_policy: &Policy,
    prefix_rule: &[String],
    commands: &[Vec<String>],
    exec_policy_fallback: &impl Fn(&[String]) -> Decision,
    match_options: &MatchOptions,
) -> bool {
    let mut policy_with_prefix_rule = exec_policy.clone();
    if policy_with_prefix_rule
        .add_prefix_rule(prefix_rule, Decision::Allow)
        .is_err()
    {
        return false;
    }

    commands.iter().all(|command| {
        policy_with_prefix_rule
            .check_with_options(command, exec_policy_fallback, match_options)
            .decision
            == Decision::Allow
    })
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
#[path = "exec_policy_tests.rs"]
mod tests;
