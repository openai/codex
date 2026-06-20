use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileSystemContext {
    workspace_roots: IndexMap<String, bool>,
    permission_profile: FileSystemPermissionProfileContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileSystemPermissionProfileContext {
    Managed(ManagedFileSystemContext),
    Disabled,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ManagedFileSystemContext {
    Restricted {
        entries: IndexMap<String, FileSystemSandboxEntry>,
        glob_scan_max_depth: Option<usize>,
    },
    Unrestricted,
}

impl FileSystemContext {
    pub(super) fn from_permission_profile(
        permission_profile: &PermissionProfile,
        workspace_roots: &[AbsolutePathBuf],
    ) -> Self {
        let permission_profile = permission_profile
            .clone()
            .materialize_project_roots_with_workspace_roots(workspace_roots);
        let workspace_roots = workspace_roots
            .iter()
            .map(|root| (root.to_string_lossy().into_owned(), true))
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

    pub(super) fn render(&self) -> String {
        let mut rendered = "<filesystem>".to_string();
        if !self.workspace_roots.is_empty() {
            rendered.push_str("<workspace_roots>");
            for root in self.workspace_roots.keys() {
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
                entries,
                glob_scan_max_depth,
            } => Self::Restricted {
                entries: entries
                    .into_iter()
                    .map(|entry| (file_system_entry_key(&entry), entry))
                    .collect(),
                glob_scan_max_depth: glob_scan_max_depth.map(usize::from),
            },
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
                for entry in entries.values() {
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

fn file_system_entry_key(entry: &FileSystemSandboxEntry) -> String {
    let path = match &entry.path {
        FileSystemPath::Path { path } => format!("path:{}", path.to_string_lossy()),
        FileSystemPath::GlobPattern { pattern } => format!("glob:{pattern}"),
        FileSystemPath::Special { value } => format!("special:{}", render_special_path(value)),
    };
    format!("{}:{path}", entry.access)
}

fn push_text_element(rendered: &mut String, name: &str, value: &str) {
    rendered.push_str(&format!("<{name}>"));
    push_xml_escaped_text(rendered, value);
    rendered.push_str(&format!("</{name}>"));
}

pub(super) fn push_xml_escaped_text(rendered: &mut String, value: &str) {
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct NetworkContext {
    domains: IndexMap<String, NetworkDomainAccess>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NetworkDomainAccess {
    Allow,
    Deny,
}

impl NetworkContext {
    pub(crate) fn new(allowed_domains: Vec<String>, denied_domains: Vec<String>) -> Self {
        Self {
            domains: allowed_domains
                .into_iter()
                .map(|domain| (domain, NetworkDomainAccess::Allow))
                .chain(
                    denied_domains
                        .into_iter()
                        .map(|domain| (domain, NetworkDomainAccess::Deny)),
                )
                .collect(),
        }
    }

    pub(super) fn render(&self) -> String {
        let mut rendered = "<network enabled=\"true\">".to_string();
        self.push_rendered_domain_element(&mut rendered, "allowed", NetworkDomainAccess::Allow);
        self.push_rendered_domain_element(&mut rendered, "denied", NetworkDomainAccess::Deny);
        rendered.push_str("</network>");
        rendered
    }

    fn push_rendered_domain_element(
        &self,
        rendered_network: &mut String,
        name: &str,
        access: NetworkDomainAccess,
    ) {
        let mut domains = self
            .domains
            .iter()
            .filter_map(|(domain, current_access)| (*current_access == access).then_some(domain));
        let Some(first) = domains.next() else {
            return;
        };

        rendered_network.push_str(&format!("<{name}>"));
        rendered_network.push_str(first);
        for domain in domains {
            rendered_network.push(',');
            rendered_network.push_str(domain);
        }
        rendered_network.push_str(&format!("</{name}>"));
    }
}
