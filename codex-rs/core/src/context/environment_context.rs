use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::Shell;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashSet;
use std::path::PathBuf;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvironmentContext {
    pub(crate) environments: EnvironmentContextEnvironments,
    pub(crate) current_date: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) network: Option<NetworkContext>,
    pub(crate) filesystem: Option<FileSystemContext>,
    pub(crate) subagents: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvironmentContextEnvironment {
    pub(crate) id: String,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) shell: String,
}

impl EnvironmentContextEnvironment {
    fn legacy(cwd: AbsolutePathBuf, shell: String) -> Self {
        Self {
            id: String::new(),
            cwd,
            shell,
        }
    }

    fn from_turn_environments(environments: &[TurnEnvironment], shell: &Shell) -> Vec<Self> {
        environments
            .iter()
            .map(|environment| Self {
                id: environment.environment_id.clone(),
                cwd: environment.cwd.clone(),
                shell: environment
                    .shell
                    .clone()
                    .unwrap_or_else(|| shell.name().to_string()),
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EnvironmentContextEnvironments {
    None,
    Single(EnvironmentContextEnvironment),
    Multiple(Vec<EnvironmentContextEnvironment>),
}

impl EnvironmentContextEnvironments {
    fn from_vec(environments: Vec<EnvironmentContextEnvironment>) -> Self {
        let mut environments = environments;
        match environments.pop() {
            None => Self::None,
            Some(environment) if environments.is_empty() => Self::Single(environment),
            Some(environment) => {
                environments.push(environment);
                Self::Multiple(environments)
            }
        }
    }

    fn equals_except_shell(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Single(left), Self::Single(right)) => left.cwd == right.cwd,
            (Self::Multiple(left), Self::Multiple(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(left, right)| left.id == right.id && left.cwd == right.cwd)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileSystemContext {
    workspace_roots: Vec<String>,
    permission_profile: FileSystemPermissionProfileContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileSystemPermissionProfileContext {
    Managed(ManagedFileSystemContext),
    Disabled,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ManagedFileSystemContext {
    Restricted {
        entries: Vec<FileSystemSandboxEntry>,
        glob_scan_max_depth: Option<usize>,
    },
    Unrestricted,
}

impl FileSystemContext {
    fn from_permission_profile(
        permission_profile: &PermissionProfile,
        workspace_roots: &[AbsolutePathBuf],
    ) -> Self {
        let permission_profile = permission_profile
            .clone()
            .materialize_project_roots_with_workspace_roots(workspace_roots);
        let workspace_roots = workspace_roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect();
        let permission_profile = match permission_profile {
            PermissionProfile::Managed { file_system, .. } => {
                FileSystemPermissionProfileContext::Managed(ManagedFileSystemContext::from(
                    file_system,
                ))
            }
            PermissionProfile::Disabled => FileSystemPermissionProfileContext::Disabled,
            PermissionProfile::External { .. } => FileSystemPermissionProfileContext::External,
        };
        Self {
            workspace_roots,
            permission_profile,
        }
    }

    fn render(&self) -> String {
        let mut rendered = "<filesystem>".to_string();
        if !self.workspace_roots.is_empty() {
            rendered.push_str("<workspace_roots>");
            for root in &self.workspace_roots {
                push_text_element(&mut rendered, "root", root);
            }
            rendered.push_str("</workspace_roots>");
        }
        self.permission_profile.render(&mut rendered);
        rendered.push_str("</filesystem>");
        rendered
    }
}

impl From<ManagedFileSystemPermissions> for ManagedFileSystemContext {
    fn from(file_system: ManagedFileSystemPermissions) -> Self {
        match file_system {
            ManagedFileSystemPermissions::Restricted {
                mut entries,
                glob_scan_max_depth,
            } => {
                dedupe_file_system_entries(&mut entries);
                Self::Restricted {
                    entries,
                    glob_scan_max_depth: glob_scan_max_depth.map(usize::from),
                }
            }
            ManagedFileSystemPermissions::Unrestricted => Self::Unrestricted,
        }
    }
}

impl FileSystemPermissionProfileContext {
    fn render(&self, rendered: &mut String) {
        match self {
            Self::Managed(file_system) => {
                rendered.push_str("<permission_profile type=\"managed\">");
                file_system.render(rendered);
                rendered.push_str("</permission_profile>");
            }
            Self::Disabled => {
                rendered.push_str(
                    "<permission_profile type=\"disabled\"><file_system type=\"unrestricted\" /></permission_profile>",
                );
            }
            Self::External => {
                rendered.push_str(
                    "<permission_profile type=\"external\"><file_system type=\"external\" /></permission_profile>",
                );
            }
        }
    }
}

impl ManagedFileSystemContext {
    fn render(&self, rendered: &mut String) {
        match self {
            Self::Restricted {
                entries,
                glob_scan_max_depth,
            } => {
                if entries.is_empty() && glob_scan_max_depth.is_none() {
                    rendered.push_str("<file_system type=\"restricted\" />");
                    return;
                }

                rendered.push_str("<file_system type=\"restricted\"");
                if let Some(glob_scan_max_depth) = glob_scan_max_depth {
                    rendered.push_str(&format!(" glob_scan_max_depth=\"{glob_scan_max_depth}\""));
                }
                rendered.push('>');
                for entry in entries {
                    render_file_system_entry(rendered, entry);
                }
                rendered.push_str("</file_system>");
            }
            Self::Unrestricted => {
                rendered.push_str("<file_system type=\"unrestricted\" />");
            }
        }
    }
}

fn render_file_system_entry(rendered: &mut String, entry: &FileSystemSandboxEntry) {
    rendered.push_str("<entry access=\"");
    let access = entry.access.to_string();
    rendered.push_str(&access);
    if entry.access == FileSystemAccessMode::Deny {
        rendered.push_str("\" escalatable=\"false");
    }
    rendered.push_str("\">");
    match &entry.path {
        FileSystemPath::Path { path } => {
            push_text_element(rendered, "path", path.to_string_lossy().as_ref());
        }
        FileSystemPath::GlobPattern { pattern } => {
            push_text_element(rendered, "glob", pattern);
        }
        FileSystemPath::Special { value } => {
            let value = render_special_path(value);
            push_text_element(rendered, "special", &value);
        }
    }
    rendered.push_str("</entry>");
}

fn render_special_path(value: &FileSystemSpecialPath) -> String {
    match value {
        FileSystemSpecialPath::Root => ":root".to_string(),
        FileSystemSpecialPath::Minimal => ":minimal".to_string(),
        FileSystemSpecialPath::ProjectRoots { subpath } => {
            render_special_path_with_subpath(":workspace_roots", subpath)
        }
        FileSystemSpecialPath::Tmpdir => ":tmpdir".to_string(),
        FileSystemSpecialPath::SlashTmp => ":slash_tmp".to_string(),
        FileSystemSpecialPath::Unknown { path, subpath } => {
            render_special_path_with_subpath(path, subpath)
        }
    }
}

fn render_special_path_with_subpath(base: &str, subpath: &Option<PathBuf>) -> String {
    match subpath {
        Some(subpath) => format!("{base}/{}", subpath.display()),
        None => base.to_string(),
    }
}

fn dedupe_file_system_entries(entries: &mut Vec<FileSystemSandboxEntry>) {
    let mut seen = HashSet::new();
    entries.retain(|entry| seen.insert(entry.clone()));
}

fn push_text_element(rendered: &mut String, name: &str, value: &str) {
    rendered.push_str(&format!("<{name}>"));
    push_xml_escaped_text(rendered, value);
    rendered.push_str(&format!("</{name}>"));
}

fn push_xml_escaped_text(rendered: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => rendered.push_str("&amp;"),
            '<' => rendered.push_str("&lt;"),
            '>' => rendered.push_str("&gt;"),
            '"' => rendered.push_str("&quot;"),
            '\'' => rendered.push_str("&apos;"),
            _ => rendered.push(ch),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct NetworkContext {
    allowed_domains: Vec<String>,
    denied_domains: Vec<String>,
}

impl NetworkContext {
    pub(crate) fn new(allowed_domains: Vec<String>, denied_domains: Vec<String>) -> Self {
        Self {
            allowed_domains,
            denied_domains,
        }
    }

    fn render(&self) -> String {
        let mut rendered = "<network enabled=\"true\">".to_string();
        Self::push_rendered_domain_element(&mut rendered, "allowed", &self.allowed_domains);
        Self::push_rendered_domain_element(&mut rendered, "denied", &self.denied_domains);
        rendered.push_str("</network>");
        rendered
    }

    fn push_rendered_domain_element(rendered_network: &mut String, name: &str, domains: &[String]) {
        if domains.is_empty() {
            return;
        }

        rendered_network.push_str(&format!("<{name}>"));
        rendered_network.push_str(&domains.join(","));
        rendered_network.push_str(&format!("</{name}>"));
    }
}

impl EnvironmentContext {
    pub(crate) fn new(
        environments: Vec<EnvironmentContextEnvironment>,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
        subagents: Option<String>,
    ) -> Self {
        Self {
            environments: EnvironmentContextEnvironments::from_vec(environments),
            current_date,
            timezone,
            network,
            filesystem: None,
            subagents,
        }
    }

    fn new_with_environments(
        environments: EnvironmentContextEnvironments,
        current_date: Option<String>,
        timezone: Option<String>,
        network: Option<NetworkContext>,
        filesystem: Option<FileSystemContext>,
        subagents: Option<String>,
    ) -> Self {
        Self {
            environments,
            current_date,
            timezone,
            network,
            filesystem,
            subagents,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub(crate) fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        self.environments.equals_except_shell(&other.environments)
            && self.current_date == other.current_date
            && self.timezone == other.timezone
            && self.network == other.network
            && self.filesystem == other.filesystem
            && self.subagents == other.subagents
    }

    pub(crate) fn diff_from_turn_context_item(
        before: &TurnContextItem,
        after: &EnvironmentContext,
    ) -> Self {
        let before_network = Self::network_from_turn_context_item(before);
        let before_filesystem = Self::filesystem_from_turn_context_item(before);
        let environments = match &after.environments {
            EnvironmentContextEnvironments::Single(environment) => {
                if before.cwd.as_path() != environment.cwd.as_path() {
                    EnvironmentContextEnvironments::Single(EnvironmentContextEnvironment::legacy(
                        environment.cwd.clone(),
                        environment.shell.clone(),
                    ))
                } else {
                    EnvironmentContextEnvironments::None
                }
            }
            EnvironmentContextEnvironments::Multiple(environments) => {
                EnvironmentContextEnvironments::Multiple(environments.clone())
            }
            EnvironmentContextEnvironments::None => EnvironmentContextEnvironments::None,
        };
        let network = if before_network != after.network {
            after.network.clone()
        } else {
            before_network
        };
        let filesystem = if before_filesystem != after.filesystem {
            after.filesystem.clone()
        } else {
            before_filesystem
        };
        EnvironmentContext::new_with_environments(
            environments,
            after.current_date.clone(),
            after.timezone.clone(),
            network,
            filesystem,
            /*subagents*/ None,
        )
    }

    pub(crate) fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        let mut context = Self::new(
            EnvironmentContextEnvironment::from_turn_environments(
                &turn_context.environments.turn_environments,
                shell,
            ),
            turn_context.current_date.clone(),
            turn_context.timezone.clone(),
            Self::network_from_turn_context(turn_context),
            /*subagents*/ None,
        );
        context.filesystem = Some(FileSystemContext::from_permission_profile(
            &turn_context.permission_profile,
            &turn_context.config.effective_workspace_roots(),
        ));
        context
    }

    pub(crate) fn from_turn_context_item(
        turn_context_item: &TurnContextItem,
        shell: String,
    ) -> Self {
        let cwd = match AbsolutePathBuf::try_from(turn_context_item.cwd.clone()) {
            Ok(cwd) => cwd,
            Err(_) => AbsolutePathBuf::resolve_path_against_base(&turn_context_item.cwd, "/"),
        };
        Self::new_with_environments(
            EnvironmentContextEnvironments::from_vec(vec![EnvironmentContextEnvironment::legacy(
                cwd, shell,
            )]),
            turn_context_item.current_date.clone(),
            turn_context_item.timezone.clone(),
            Self::network_from_turn_context_item(turn_context_item),
            Self::filesystem_from_turn_context_item(turn_context_item),
            /*subagents*/ None,
        )
    }

    pub(crate) fn with_subagents(mut self, subagents: String) -> Self {
        if !subagents.is_empty() {
            self.subagents = Some(subagents);
        }
        self
    }

    fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkContext> {
        let network = turn_context
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;

        Some(NetworkContext::new(
            network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
                .unwrap_or_default(),
            network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains)
                .unwrap_or_default(),
        ))
    }

    fn network_from_turn_context_item(
        turn_context_item: &TurnContextItem,
    ) -> Option<NetworkContext> {
        let TurnContextNetworkItem {
            allowed_domains,
            denied_domains,
        } = turn_context_item.network.as_ref()?;
        Some(NetworkContext::new(
            allowed_domains.clone(),
            denied_domains.clone(),
        ))
    }

    fn filesystem_from_turn_context_item(
        turn_context_item: &TurnContextItem,
    ) -> Option<FileSystemContext> {
        Some(FileSystemContext::from_permission_profile(
            &turn_context_item.permission_profile(),
            &workspace_roots_from_turn_context_item(turn_context_item),
        ))
    }
}

fn workspace_roots_from_turn_context_item(
    turn_context_item: &TurnContextItem,
) -> Vec<AbsolutePathBuf> {
    if let Some(workspace_roots) = turn_context_item.workspace_roots.as_ref() {
        return workspace_roots.clone();
    }

    // Older rollout items did not persist workspace roots. Fall back to the
    // legacy cwd binding only when reconstructing that historical context.
    match AbsolutePathBuf::try_from(turn_context_item.cwd.clone()) {
        Ok(cwd) => vec![cwd],
        Err(_) => Vec::new(),
    }
}

impl ContextualUserFragment for EnvironmentContext {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG,
            codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        let mut lines = Vec::new();
        match &self.environments {
            EnvironmentContextEnvironments::Single(environment) => {
                lines.push(format!(
                    "  <cwd>{}</cwd>",
                    environment.cwd.to_string_lossy()
                ));
                lines.push(format!("  <shell>{}</shell>", environment.shell));
            }
            EnvironmentContextEnvironments::Multiple(environments) => {
                lines.push("  <environments>".to_string());
                for environment in environments {
                    lines.push(format!("    <environment id=\"{}\">", environment.id));
                    lines.push(format!(
                        "      <cwd>{}</cwd>",
                        environment.cwd.to_string_lossy()
                    ));
                    lines.push(format!("      <shell>{}</shell>", environment.shell));
                    lines.push("    </environment>".to_string());
                }
                lines.push("  </environments>".to_string());
            }
            EnvironmentContextEnvironments::None => {}
        }
        if let Some(current_date) = &self.current_date {
            lines.push(format!("  <current_date>{current_date}</current_date>"));
        }
        if let Some(timezone) = &self.timezone {
            lines.push(format!("  <timezone>{timezone}</timezone>"));
        }
        match &self.network {
            Some(network) => {
                lines.push(format!("  {}", network.render()));
            }
            None => {
                // TODO(mbolin): Include this line if it helps the model.
                // lines.push("  <network enabled=\"false\" />".to_string());
            }
        }
        if let Some(filesystem) = &self.filesystem {
            lines.push(format!("  {}", filesystem.render()));
        }
        if let Some(subagents) = &self.subagents {
            lines.push("  <subagents>".to_string());
            lines.extend(subagents.lines().map(|line| format!("    {line}")));
            lines.push("  </subagents>".to_string());
        }
        format!("\n{}\n", lines.join("\n"))
    }
}

#[cfg(test)]
#[path = "environment_context_tests.rs"]
mod tests;
