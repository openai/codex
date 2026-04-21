use super::ContextualUserFragment;
use codex_execpolicy::Policy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::format_allow_prefixes;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::WritableRoot;
use codex_utils_template::Template;
use std::path::Path;
use std::sync::LazyLock;

const APPROVAL_POLICY_NEVER: &str = include_str!("prompts/permissions/approval_policy/never.md");
const APPROVAL_POLICY_UNLESS_TRUSTED: &str =
    include_str!("prompts/permissions/approval_policy/unless_trusted.md");
const APPROVAL_POLICY_ON_FAILURE: &str =
    include_str!("prompts/permissions/approval_policy/on_failure.md");
const APPROVAL_POLICY_ON_REQUEST_RULE: &str =
    include_str!("prompts/permissions/approval_policy/on_request.md");
const APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION: &str =
    include_str!("prompts/permissions/approval_policy/on_request_rule_request_permission.md");
const AUTO_REVIEW_APPROVAL_SUFFIX: &str = "`approvals_reviewer` is `auto_review`: Sandbox escalations with require_escalated will be reviewed for compliance with the policy. If a rejection happens, you should proceed only with a materially safer alternative, or inform the user of the risk and send a final message to ask for approval.";

const SANDBOX_MODE_DANGER_FULL_ACCESS: &str =
    include_str!("prompts/permissions/sandbox_mode/danger_full_access.md");
const SANDBOX_MODE_WORKSPACE_WRITE: &str =
    include_str!("prompts/permissions/sandbox_mode/workspace_write.md");
const SANDBOX_MODE_READ_ONLY: &str = include_str!("prompts/permissions/sandbox_mode/read_only.md");

static SANDBOX_MODE_DANGER_FULL_ACCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(SANDBOX_MODE_DANGER_FULL_ACCESS.trim_end())
        .unwrap_or_else(|err| panic!("danger-full-access sandbox template must parse: {err}"))
});
static SANDBOX_MODE_WORKSPACE_WRITE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(SANDBOX_MODE_WORKSPACE_WRITE.trim_end())
        .unwrap_or_else(|err| panic!("workspace-write sandbox template must parse: {err}"))
});
static SANDBOX_MODE_READ_ONLY_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(SANDBOX_MODE_READ_ONLY.trim_end())
        .unwrap_or_else(|err| panic!("read-only sandbox template must parse: {err}"))
});

struct PermissionsPromptConfig<'a> {
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    exec_policy: &'a Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PermissionsInstructions {
    text: String,
}

impl PermissionsInstructions {
    pub(crate) fn from_policy(
        sandbox_policy: &SandboxPolicy,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let network_access = if sandbox_policy.has_full_network_access() {
            NetworkAccess::Enabled
        } else {
            NetworkAccess::Restricted
        };

        let (sandbox_mode, writable_roots) = match sandbox_policy {
            SandboxPolicy::DangerFullAccess => (SandboxMode::DangerFullAccess, None),
            SandboxPolicy::ReadOnly { .. } => (SandboxMode::ReadOnly, None),
            SandboxPolicy::ExternalSandbox { .. } => (SandboxMode::DangerFullAccess, None),
            SandboxPolicy::WorkspaceWrite { .. } => {
                let roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
                (SandboxMode::WorkspaceWrite, Some(roots))
            }
        };

        Self::from_permissions_with_network(
            sandbox_mode,
            network_access,
            PermissionsPromptConfig {
                approval_policy,
                approvals_reviewer,
                exec_policy,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            },
            writable_roots,
        )
    }

    fn from_permissions_with_network(
        sandbox_mode: SandboxMode,
        network_access: NetworkAccess,
        config: PermissionsPromptConfig<'_>,
        writable_roots: Option<Vec<WritableRoot>>,
    ) -> Self {
        let mut text = String::new();
        append_section(&mut text, &sandbox_text(sandbox_mode, network_access));
        append_section(
            &mut text,
            &approval_text(
                config.approval_policy,
                config.approvals_reviewer,
                config.exec_policy,
                config.exec_permission_approvals_enabled,
                config.request_permissions_tool_enabled,
            ),
        );
        if let Some(writable_roots) = writable_roots_text(writable_roots) {
            append_section(&mut text, &writable_roots);
        }
        if text.ends_with('\n') {
            text.pop();
        }
        Self { text }
    }
}

impl ContextualUserFragment for PermissionsInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<permissions instructions>";
    const END_MARKER: &'static str = "</permissions instructions>";

    fn body(&self) -> String {
        self.text.clone()
    }
}

fn append_section(text: &mut String, section: &str) {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(section);
}

fn approval_text(
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    exec_policy: &Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
) -> String {
    let with_request_permissions_tool = |text: &str| {
        if request_permissions_tool_enabled {
            format!("{text}\n\n{}", request_permissions_tool_prompt_section())
        } else {
            text.to_string()
        }
    };
    let on_request_instructions = || {
        let on_request_rule = if exec_permission_approvals_enabled {
            APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string()
        } else {
            APPROVAL_POLICY_ON_REQUEST_RULE.to_string()
        };
        let mut sections = vec![on_request_rule];
        if request_permissions_tool_enabled {
            sections.push(request_permissions_tool_prompt_section().to_string());
        }
        if let Some(prefixes) = approved_command_prefixes_text(exec_policy) {
            sections.push(format!(
                "## Approved command prefixes\nThe following prefix rules have already been approved: {prefixes}"
            ));
        }
        sections.join("\n\n")
    };
    let text = match approval_policy {
        AskForApproval::Never => APPROVAL_POLICY_NEVER.to_string(),
        AskForApproval::UnlessTrusted => {
            with_request_permissions_tool(APPROVAL_POLICY_UNLESS_TRUSTED)
        }
        AskForApproval::OnFailure => with_request_permissions_tool(APPROVAL_POLICY_ON_FAILURE),
        AskForApproval::OnRequest => on_request_instructions(),
        AskForApproval::Granular(granular_config) => granular_instructions(
            granular_config,
            exec_policy,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
        ),
    };

    if approvals_reviewer == ApprovalsReviewer::GuardianSubagent
        && approval_policy != AskForApproval::Never
    {
        format!("{text}\n\n{AUTO_REVIEW_APPROVAL_SUFFIX}")
    } else {
        text
    }
}

fn sandbox_text(mode: SandboxMode, network_access: NetworkAccess) -> String {
    let template = match mode {
        SandboxMode::DangerFullAccess => &*SANDBOX_MODE_DANGER_FULL_ACCESS_TEMPLATE,
        SandboxMode::WorkspaceWrite => &*SANDBOX_MODE_WORKSPACE_WRITE_TEMPLATE,
        SandboxMode::ReadOnly => &*SANDBOX_MODE_READ_ONLY_TEMPLATE,
    };
    let network_access = network_access.to_string();
    template
        .render([("network_access", network_access.as_str())])
        .unwrap_or_else(|err| panic!("sandbox template must render: {err}"))
}

fn writable_roots_text(writable_roots: Option<Vec<WritableRoot>>) -> Option<String> {
    let roots = writable_roots?;
    if roots.is_empty() {
        return None;
    }

    let roots_list: Vec<String> = roots
        .iter()
        .map(|r| format!("`{}`", r.root.to_string_lossy()))
        .collect();
    Some(if roots_list.len() == 1 {
        format!(" The writable root is {}.", roots_list[0])
    } else {
        format!(" The writable roots are {}.", roots_list.join(", "))
    })
}

fn approved_command_prefixes_text(exec_policy: &Policy) -> Option<String> {
    format_allow_prefixes(exec_policy.get_allowed_prefixes())
        .filter(|prefixes| !prefixes.is_empty())
}

fn granular_prompt_intro_text() -> &'static str {
    "# Approval Requests\n\nApproval policy is `granular`. Categories set to `false` are automatically rejected instead of prompting the user."
}

fn request_permissions_tool_prompt_section() -> &'static str {
    "# request_permissions Tool\n\nThe built-in `request_permissions` tool is available in this session. Invoke it when you need to request additional `network` or `file_system` permissions before later shell-like commands need them. Request only the specific permissions required for the task."
}

fn granular_instructions(
    granular_config: GranularApprovalConfig,
    exec_policy: &Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
) -> String {
    let sandbox_approval_prompts_allowed = granular_config.allows_sandbox_approval();
    let shell_permission_requests_available =
        exec_permission_approvals_enabled && sandbox_approval_prompts_allowed;
    let request_permissions_tool_prompts_allowed =
        request_permissions_tool_enabled && granular_config.allows_request_permissions();
    let categories = [
        Some((
            granular_config.allows_sandbox_approval(),
            "`sandbox_approval`",
        )),
        Some((granular_config.allows_rules_approval(), "`rules`")),
        Some((granular_config.allows_skill_approval(), "`skill_approval`")),
        request_permissions_tool_enabled.then_some((
            granular_config.allows_request_permissions(),
            "`request_permissions`",
        )),
        Some((
            granular_config.allows_mcp_elicitations(),
            "`mcp_elicitations`",
        )),
    ];
    let prompted_categories = categories
        .iter()
        .flatten()
        .filter(|&&(is_allowed, _)| is_allowed)
        .map(|&(_, category)| format!("- {category}"))
        .collect::<Vec<_>>();
    let rejected_categories = categories
        .iter()
        .flatten()
        .filter(|&&(is_allowed, _)| !is_allowed)
        .map(|&(_, category)| format!("- {category}"))
        .collect::<Vec<_>>();

    let mut sections = vec![granular_prompt_intro_text().to_string()];

    if !prompted_categories.is_empty() {
        sections.push(format!(
            "These approval categories may still prompt the user when needed:\n{}",
            prompted_categories.join("\n")
        ));
    }
    if !rejected_categories.is_empty() {
        sections.push(format!(
            "These approval categories are automatically rejected instead of prompting the user:\n{}",
            rejected_categories.join("\n")
        ));
    }

    if shell_permission_requests_available {
        sections.push(APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string());
    }

    if request_permissions_tool_prompts_allowed {
        sections.push(request_permissions_tool_prompt_section().to_string());
    }

    if let Some(prefixes) = approved_command_prefixes_text(exec_policy) {
        sections.push(format!(
            "## Approved command prefixes\nThe following prefix rules have already been approved: {prefixes}"
        ));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_execpolicy::Decision;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn renders_sandbox_mode_text() {
        assert_eq!(
            sandbox_text(SandboxMode::WorkspaceWrite, NetworkAccess::Restricted),
            "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: The sandbox permits reading files, and editing files in `cwd` and `writable_roots`. Editing files in other directories requires approval. Network access is restricted."
        );

        assert_eq!(
            sandbox_text(SandboxMode::ReadOnly, NetworkAccess::Restricted),
            "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `read-only`: The sandbox only permits reading files. Network access is restricted."
        );

        assert_eq!(
            sandbox_text(SandboxMode::DangerFullAccess, NetworkAccess::Enabled),
            "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled."
        );
    }

    #[test]
    fn builds_permissions_with_network_access_override() {
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: false,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(
            text.contains("Network access is enabled."),
            "expected network access to be enabled in message"
        );
        assert!(
            text.contains("How to request escalation"),
            "expected approval guidance to be included"
        );
    }

    #[test]
    fn builds_permissions_from_policy() {
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let instructions = PermissionsInstructions::from_policy(
            &policy,
            AskForApproval::UnlessTrusted,
            ApprovalsReviewer::User,
            &Policy::empty(),
            &PathBuf::from("/tmp"),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );
        let text = instructions.body();
        assert!(text.contains("Network access is enabled."));
        assert!(text.contains("`approval_policy` is `unless-trusted`"));
    }

    #[test]
    fn includes_request_rule_instructions_for_on_request() {
        let mut exec_policy = Policy::empty();
        exec_policy
            .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
            .expect("add rule");
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &exec_policy,
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: false,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(text.contains("prefix_rule"));
        assert!(text.contains("Approved command prefixes"));
        assert!(text.contains(r#"["git", "pull"]"#));
    }

    #[test]
    fn includes_request_permissions_tool_instructions_for_unless_trusted_when_enabled() {
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::UnlessTrusted,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(text.contains("`approval_policy` is `unless-trusted`"));
        assert!(text.contains("# request_permissions Tool"));
    }

    #[test]
    fn includes_request_permissions_tool_instructions_for_on_failure_when_enabled() {
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnFailure,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(text.contains("`approval_policy` is `on-failure`"));
        assert!(text.contains("# request_permissions Tool"));
    }

    #[test]
    fn includes_request_permission_rule_instructions_for_on_request_when_enabled() {
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: true,
                request_permissions_tool_enabled: false,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(text.contains("with_additional_permissions"));
        assert!(text.contains("additional_permissions"));
    }

    #[test]
    fn includes_request_permissions_tool_instructions_for_on_request_when_tool_is_enabled() {
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(text.contains("# request_permissions Tool"));
        assert!(
            text.contains("The built-in `request_permissions` tool is available in this session.")
        );
    }

    #[test]
    fn on_request_includes_tool_guidance_alongside_inline_permission_guidance_when_both_exist() {
        let instructions = PermissionsInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: true,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.body();
        assert!(text.contains("with_additional_permissions"));
        assert!(text.contains("# request_permissions Tool"));
    }

    #[test]
    fn guardian_subagent_approvals_append_guardian_specific_guidance() {
        let text = approval_text(
            AskForApproval::OnRequest,
            ApprovalsReviewer::GuardianSubagent,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );

        assert!(text.contains("`approvals_reviewer` is `auto_review`"));
        assert!(!text.contains("`approvals_reviewer` is `guardian_subagent`"));
        assert!(text.contains("materially safer alternative"));
    }

    #[test]
    fn guardian_subagent_approvals_omit_guardian_specific_guidance_when_approval_is_never() {
        let text = approval_text(
            AskForApproval::Never,
            ApprovalsReviewer::GuardianSubagent,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );

        assert!(!text.contains("`approvals_reviewer` is `auto_review`"));
        assert!(!text.contains("`approvals_reviewer` is `guardian_subagent`"));
    }

    fn granular_categories_section(title: &str, categories: &[&str]) -> String {
        format!("{title}\n{}", categories.join("\n"))
    }

    fn granular_prompt_expected(
        prompted_categories: &[&str],
        rejected_categories: &[&str],
        include_shell_permission_request_instructions: bool,
        include_request_permissions_tool_section: bool,
    ) -> String {
        let mut sections = vec![granular_prompt_intro_text().to_string()];
        if !prompted_categories.is_empty() {
            sections.push(granular_categories_section(
                "These approval categories may still prompt the user when needed:",
                prompted_categories,
            ));
        }
        if !rejected_categories.is_empty() {
            sections.push(granular_categories_section(
                "These approval categories are automatically rejected instead of prompting the user:",
                rejected_categories,
            ));
        }
        if include_shell_permission_request_instructions {
            sections.push(APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string());
        }
        if include_request_permissions_tool_section {
            sections.push(request_permissions_tool_prompt_section().to_string());
        }
        sections.join("\n\n")
    }

    #[test]
    fn granular_policy_lists_prompted_and_rejected_categories_separately() {
        let text = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: false,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ false,
        );

        assert_eq!(
            text,
            [
                granular_prompt_intro_text().to_string(),
                granular_categories_section(
                    "These approval categories may still prompt the user when needed:",
                    &["- `rules`"],
                ),
                granular_categories_section(
                    "These approval categories are automatically rejected instead of prompting the user:",
                    &[
                        "- `sandbox_approval`",
                        "- `skill_approval`",
                        "- `mcp_elicitations`",
                    ],
                ),
            ]
            .join("\n\n")
        );
    }

    #[test]
    fn granular_policy_includes_command_permission_instructions_when_sandbox_approval_can_prompt() {
        let text = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ false,
        );

        assert_eq!(
            text,
            granular_prompt_expected(
                &[
                    "- `sandbox_approval`",
                    "- `rules`",
                    "- `skill_approval`",
                    "- `mcp_elicitations`",
                ],
                &[],
                /*include_shell_permission_request_instructions*/ true,
                /*include_request_permissions_tool_section*/ false,
            )
        );
    }

    #[test]
    fn granular_policy_omits_shell_permission_instructions_when_inline_requests_are_disabled() {
        let text = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );

        assert_eq!(
            text,
            granular_prompt_expected(
                &[
                    "- `sandbox_approval`",
                    "- `rules`",
                    "- `skill_approval`",
                    "- `mcp_elicitations`",
                ],
                &[],
                /*include_shell_permission_request_instructions*/ false,
                /*include_request_permissions_tool_section*/ false,
            )
        );
    }

    #[test]
    fn granular_policy_includes_request_permissions_tool_only_when_that_prompt_can_still_fire() {
        let allowed = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ true,
        );
        assert!(allowed.contains("# request_permissions Tool"));

        let rejected = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: false,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ true,
        );
        assert!(!rejected.contains("# request_permissions Tool"));
    }

    #[test]
    fn granular_policy_lists_request_permissions_category_without_tool_section_when_tool_unavailable()
     {
        let text = approval_text(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: false,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: false,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ false,
        );

        assert!(!text.contains("- `request_permissions`"));
        assert!(!text.contains("# request_permissions Tool"));
    }
}
