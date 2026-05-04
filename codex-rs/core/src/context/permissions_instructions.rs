use super::ContextualUserFragment;
use codex_execpolicy::Policy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::format_allow_prefixes;
use codex_protocol::permissions::NetworkSandboxPolicy;
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
/// Developer instructions that describe the active sandbox and approval policy.
pub struct PermissionsInstructions {
    text: String,
}

impl PermissionsInstructions {
    /// Builds permissions instructions from the effective permission profile and approval policy.
    pub fn from_permission_profile(
        permission_profile: &PermissionProfile,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let (sandbox_mode, writable_roots) = sandbox_prompt_from_profile(permission_profile, cwd);

        Self::from_permissions_with_network(
            sandbox_mode,
            network_access_from_policy(permission_profile.network_sandbox_policy()),
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

    /// Builds permissions instructions from a legacy sandbox policy.
    pub fn from_policy(
        sandbox_policy: &SandboxPolicy,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        Self::from_permission_profile(
            &PermissionProfile::from_legacy_sandbox_policy(sandbox_policy),
            approval_policy,
            approvals_reviewer,
            exec_policy,
            cwd,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
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
        if !text.ends_with('\n') {
            text.push('\n');
        }
        Self { text }
    }
}

fn sandbox_prompt_from_profile(
    permission_profile: &PermissionProfile,
    cwd: &Path,
) -> (SandboxMode, Option<Vec<WritableRoot>>) {
    match permission_profile {
        PermissionProfile::Disabled | PermissionProfile::External { .. } => {
            (SandboxMode::DangerFullAccess, None)
        }
        PermissionProfile::Managed { .. } => {
            let file_system_policy = permission_profile.file_system_sandbox_policy();
            if file_system_policy.has_full_disk_write_access() {
                return (SandboxMode::DangerFullAccess, None);
            }

            let writable_roots = file_system_policy.get_writable_roots_with_cwd(cwd);
            if writable_roots.is_empty() {
                (SandboxMode::ReadOnly, None)
            } else {
                (SandboxMode::WorkspaceWrite, Some(writable_roots))
            }
        }
    }
}

fn network_access_from_policy(network_policy: NetworkSandboxPolicy) -> NetworkAccess {
    if network_policy.is_enabled() {
        NetworkAccess::Enabled
    } else {
        NetworkAccess::Restricted
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

    if approvals_reviewer == ApprovalsReviewer::AutoReview
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
#[path = "permissions_instructions_tests.rs"]
mod permissions_instructions_tests;
