//! Composes selected permission text and prepared runtime facts into a stored fragment body.
//! Only the profile adapter inspects the host filesystem; composition uses the supplied facts.
//! Catalog text uses literal substitution; bundled sandbox templates are parsed and cached here.

use crate::ResolvedMessage;
use crate::ResolvedModelMessages;
use crate::model_messages::permissions::DANGER_FULL_ACCESS_TEMPLATE;
use crate::model_messages::permissions::READ_ONLY_TEMPLATE;
use crate::model_messages::permissions::ResolvedApprovalMessages;
use crate::model_messages::permissions::ResolvedPermissionMessages;
use crate::model_messages::permissions::WORKSPACE_WRITE_TEMPLATE;
use codex_context_fragments::ContextualUserFragment;
use codex_execpolicy::Policy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::format_allow_prefixes;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::NetworkAccess;
use codex_utils_template::Template;
use std::path::Path;
use std::sync::LazyLock;

const REQUEST_PERMISSION_RULE: &str =
    include_str!("../templates/permissions/approval_policy/on_request_rule_request_permission.md");
const REQUEST_PERMISSIONS_TOOL: &str = "# request_permissions Tool\n\nThe built-in `request_permissions` tool is available in this session. Invoke it when you need to request additional `network` or `file_system` permissions before later shell-like commands need them. Request only the specific permissions required for the task.";
const AUTO_REVIEW_SUFFIX: &str = "`approvals_reviewer` is `auto_review`: Sandbox escalations with require_escalated will be reviewed for compliance with the policy. If a rejection happens, you should proceed only with a materially safer alternative, or inform the user of the risk and send a final message to ask for approval.";
const APPROVED_PREFIXES: &str =
    "## Approved command prefixes\nThe following prefix rules have already been approved: ";
const GRANULAR_INTRO: &str = "# Approval Requests\n\nApproval policy is `granular`. Categories set to `false` are automatically rejected instead of prompting the user.";
const GRANULAR_PROMPTED_CATEGORIES: &str =
    "These approval categories may still prompt the user when needed:";
const GRANULAR_REJECTED_CATEGORIES: &str =
    "These approval categories are automatically rejected instead of prompting the user:";

static DANGER_FULL_ACCESS: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(DANGER_FULL_ACCESS_TEMPLATE.trim_end())
        .unwrap_or_else(|err| panic!("danger-full-access sandbox template must parse: {err}"))
});
static WORKSPACE_WRITE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(WORKSPACE_WRITE_TEMPLATE.trim_end())
        .unwrap_or_else(|err| panic!("workspace-write sandbox template must parse: {err}"))
});
static READ_ONLY: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(READ_ONLY_TEMPLATE.trim_end())
        .unwrap_or_else(|err| panic!("read-only sandbox template must parse: {err}"))
});

#[derive(Debug, Clone, Copy)]
pub struct ApprovalPromptContext<'a> {
    reviewer: ApprovalsReviewer,
    messages: ResolvedApprovalMessages<'a>,
    permission_messages: ResolvedPermissionMessages<'a>,
}

impl<'a> ApprovalPromptContext<'a> {
    pub fn new(reviewer: ApprovalsReviewer, model_messages: ResolvedModelMessages<'a>) -> Self {
        Self {
            reviewer,
            messages: model_messages.approvals(),
            permission_messages: model_messages.permissions(),
        }
    }
}

/// Prepared runtime facts borrowed only while composing the fragment.
/// Paths retain their resolved display spellings and order.
#[derive(Debug, Clone, Copy)]
struct PermissionsRenderContext<'a> {
    sandbox_mode: SandboxMode,
    network_access: NetworkAccess,
    approval_policy: AskForApproval,
    approved_command_prefixes: &'a [Vec<String>],
    writable_roots: &'a [String],
    denied_read_paths: &'a [String],
    denied_read_globs: &'a [String],
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
}

/// Developer fragment describing the active sandbox and approval policy.
#[derive(Debug, Clone)]
pub struct PermissionsInstructions {
    text: String,
}

impl PermissionsInstructions {
    /// Composes a permission fragment from resolved messages and caller-supplied facts.
    /// Paths are used verbatim; this constructor does not inspect the filesystem or environment.
    fn from_resolved(
        context: PermissionsRenderContext<'_>,
        approval_context: ApprovalPromptContext<'_>,
    ) -> Self {
        let mut text = String::new();
        let sandbox = sandbox_text(
            context.sandbox_mode,
            context.network_access,
            approval_context.permission_messages,
        );
        if !sandbox.is_empty() {
            append_section(&mut text, &sandbox);
        }
        append_section(
            &mut text,
            &approval_text(
                context.approval_policy,
                approval_context.reviewer,
                approval_context.messages,
                context.approved_command_prefixes,
                context.exec_permission_approvals_enabled,
                context.request_permissions_tool_enabled,
            ),
        );
        if let Some(writable_roots) = writable_roots_text(context.writable_roots) {
            append_section(&mut text, &writable_roots);
        }
        if let Some(denied_reads) =
            denied_reads_text(context.denied_read_paths, context.denied_read_globs)
        {
            append_section(&mut text, &denied_reads);
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        Self { text }
    }

    /// Resolves a permission profile against the host filesystem before rendering instructions.
    pub fn from_permission_profile(
        permission_profile: &PermissionProfile,
        approval_policy: AskForApproval,
        approval_context: ApprovalPromptContext<'_>,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let file_system_policy = permission_profile.file_system_sandbox_policy();
        let (sandbox_mode, mut writable_roots) = if file_system_policy.has_full_disk_write_access()
        {
            (SandboxMode::DangerFullAccess, Vec::new())
        } else {
            let roots = file_system_policy.get_writable_roots_with_cwd(cwd);
            let mode = if roots.is_empty() {
                SandboxMode::ReadOnly
            } else {
                SandboxMode::WorkspaceWrite
            };
            (mode, roots)
        };
        writable_roots.sort_by(|left, right| left.root.as_path().cmp(right.root.as_path()));
        let writable_roots = writable_roots
            .iter()
            .map(|root| root.root.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let denied_read_paths = file_system_policy
            .get_unreadable_roots_with_cwd(cwd)
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let denied_read_globs = file_system_policy.get_unreadable_globs_with_cwd(cwd);
        Self::from_resolved(
            PermissionsRenderContext {
                sandbox_mode,
                network_access: if permission_profile.network_sandbox_policy().is_enabled() {
                    NetworkAccess::Enabled
                } else {
                    NetworkAccess::Restricted
                },
                approval_policy,
                approved_command_prefixes: &exec_policy.get_allowed_prefixes(),
                writable_roots: &writable_roots,
                denied_read_paths: &denied_read_paths,
                denied_read_globs: &denied_read_globs,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            },
            approval_context,
        )
    }
}

impl ContextualUserFragment for PermissionsInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("permissions.instructions".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<permissions instructions>", "</permissions instructions>")
    }

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
    messages: ResolvedApprovalMessages<'_>,
    approved_command_prefixes: &[Vec<String>],
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
) -> String {
    let selected = match approval_policy {
        AskForApproval::OnRequest => match approvals_reviewer {
            ApprovalsReviewer::User => messages.on_request,
            ApprovalsReviewer::AutoReview => messages.on_request_auto_review,
        },
        AskForApproval::Never => messages.never,
        AskForApproval::UnlessTrusted => messages.unless_trusted,
        AskForApproval::Granular(_) => ResolvedMessage::Bundled(GRANULAR_INTRO),
    };
    let base = match selected {
        ResolvedMessage::Catalog(text) => return text.to_string(),
        ResolvedMessage::Bundled(text) => text,
    };
    let text = match approval_policy {
        AskForApproval::Never => return base.to_string(),
        AskForApproval::UnlessTrusted => {
            if request_permissions_tool_enabled {
                format!("{base}\n\n{REQUEST_PERMISSIONS_TOOL}")
            } else {
                base.to_string()
            }
        }
        AskForApproval::OnRequest => {
            let rule = if exec_permission_approvals_enabled {
                REQUEST_PERMISSION_RULE
            } else {
                base
            };
            let mut sections = vec![rule.to_string()];
            if request_permissions_tool_enabled {
                sections.push(REQUEST_PERMISSIONS_TOOL.to_string());
            }
            if let Some(prefixes) = approved_command_prefixes_text(approved_command_prefixes) {
                sections.push(format!("{APPROVED_PREFIXES}{prefixes}"));
            }
            sections.join("\n\n")
        }
        AskForApproval::Granular(granular_config) => granular_instructions(
            granular_config,
            approved_command_prefixes,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
        ),
    };

    if approvals_reviewer == ApprovalsReviewer::AutoReview {
        format!("{text}\n\n{AUTO_REVIEW_SUFFIX}")
    } else {
        text
    }
}

fn sandbox_text(
    mode: SandboxMode,
    network_access: NetworkAccess,
    messages: ResolvedPermissionMessages<'_>,
) -> String {
    let (selected, template) = match mode {
        SandboxMode::DangerFullAccess => (messages.danger_full_access, &DANGER_FULL_ACCESS),
        SandboxMode::WorkspaceWrite => (messages.workspace_write, &WORKSPACE_WRITE),
        SandboxMode::ReadOnly => (messages.read_only, &READ_ONLY),
    };
    let network_access = network_access.to_string();
    match selected {
        ResolvedMessage::Catalog(text) => text.replace("{{ network_access }}", &network_access),
        ResolvedMessage::Bundled(_) => template
            .render([("network_access", network_access.as_str())])
            .unwrap_or_else(|err| panic!("sandbox template must render: {err}")),
    }
}

fn writable_roots_text(writable_roots: &[String]) -> Option<String> {
    if writable_roots.is_empty() {
        return None;
    }

    let roots_list: Vec<String> = writable_roots
        .iter()
        .map(|root| format!("`{root}`"))
        .collect();
    Some(if roots_list.len() == 1 {
        format!(" The writable root is {}.", roots_list[0])
    } else {
        format!(" The writable roots are {}.", roots_list.join(", "))
    })
}

fn denied_reads_text(paths: &[String], globs: &[String]) -> Option<String> {
    let mut entries = paths
        .iter()
        .map(|root| format!("- path `{root}`"))
        .collect::<Vec<_>>();
    entries.extend(globs.iter().map(|glob| format!("- glob `{glob}`")));
    if entries.is_empty() {
        return None;
    }

    Some(format!(
        "## Denied filesystem reads\nThe active permission profile denies reading these paths/globs. Do not request escalation or additional permissions to read them; these denials are policy restrictions.\n{}",
        entries.join("\n")
    ))
}

fn approved_command_prefixes_text(approved_command_prefixes: &[Vec<String>]) -> Option<String> {
    format_allow_prefixes(approved_command_prefixes.to_vec())
        .filter(|prefixes| !prefixes.is_empty())
}

fn granular_instructions(
    granular_config: GranularApprovalConfig,
    approved_command_prefixes: &[Vec<String>],
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

    let mut sections = vec![GRANULAR_INTRO.to_string()];

    if !prompted_categories.is_empty() {
        sections.push(format!(
            "{GRANULAR_PROMPTED_CATEGORIES}\n{}",
            prompted_categories.join("\n")
        ));
    }
    if !rejected_categories.is_empty() {
        sections.push(format!(
            "{GRANULAR_REJECTED_CATEGORIES}\n{}",
            rejected_categories.join("\n")
        ));
    }

    if shell_permission_requests_available {
        sections.push(REQUEST_PERMISSION_RULE.to_string());
    }

    if request_permissions_tool_prompts_allowed {
        sections.push(REQUEST_PERMISSIONS_TOOL.to_string());
    }

    if let Some(prefixes) = approved_command_prefixes_text(approved_command_prefixes) {
        sections.push(format!("{APPROVED_PREFIXES}{prefixes}"));
    }

    sections.join("\n\n")
}

#[cfg(test)]
#[path = "permissions_instructions_tests.rs"]
mod tests;
