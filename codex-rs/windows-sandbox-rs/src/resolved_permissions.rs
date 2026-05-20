use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Windows-local view of the runtime permission profile.
///
/// Most Windows sandbox code needs resolved runtime permissions plus a few
/// Windows-specific path conventions, not the user/config-facing
/// `PermissionProfile` enum itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedWindowsSandboxPermissions {
    file_system: FileSystemSandboxPolicy,
    network: NetworkSandboxPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowsWritableRoot {
    pub(crate) root: PathBuf,
    pub(crate) read_only_subpaths: Vec<PathBuf>,
}

impl ResolvedWindowsSandboxPermissions {
    pub(crate) fn from_legacy_policy(policy: &SandboxPolicy) -> Self {
        Self {
            file_system: FileSystemSandboxPolicy::from(policy),
            network: NetworkSandboxPolicy::from(policy),
        }
    }

    pub(crate) fn from_legacy_policy_for_cwd(policy: &SandboxPolicy, cwd: &Path) -> Self {
        Self {
            file_system: FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(policy, cwd),
            network: NetworkSandboxPolicy::from(policy),
        }
    }

    pub(crate) fn should_apply_network_block(&self) -> bool {
        !self.network.is_enabled()
    }

    pub(crate) fn writable_roots_for_cwd(
        &self,
        cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> Vec<WindowsWritableRoot> {
        let mut file_system = self.file_system.clone();
        file_system
            .entries
            .retain(|FileSystemSandboxEntry { path, .. }| {
                !matches!(
                    path,
                    FileSystemPath::Special {
                        value: codex_protocol::permissions::FileSystemSpecialPath::Tmpdir
                            | codex_protocol::permissions::FileSystemSpecialPath::SlashTmp,
                    }
                )
            });

        let mut roots = file_system
            .get_writable_roots_with_cwd(cwd)
            .into_iter()
            .map(|root| WindowsWritableRoot {
                root: root.root.into_path_buf(),
                read_only_subpaths: root
                    .read_only_subpaths
                    .into_iter()
                    .map(AbsolutePathBuf::into_path_buf)
                    .collect(),
            })
            .collect::<Vec<_>>();

        if self.has_writable_tmpdir_entry() {
            roots.extend(windows_temp_env_roots(env_map).into_iter().map(|root| {
                WindowsWritableRoot {
                    root,
                    read_only_subpaths: Vec::new(),
                }
            }));
        }

        roots
    }

    fn has_writable_tmpdir_entry(&self) -> bool {
        self.file_system
            .entries
            .iter()
            .any(|FileSystemSandboxEntry { path, access }| {
                matches!(
                    path,
                    FileSystemPath::Special {
                        value: codex_protocol::permissions::FileSystemSpecialPath::Tmpdir,
                    }
                ) && access.can_write()
            })
    }
}

fn windows_temp_env_roots(env_map: &HashMap<String, String>) -> Vec<PathBuf> {
    ["TEMP", "TMP"]
        .into_iter()
        .filter_map(|key| {
            env_map
                .get(key)
                .map(|value| PathBuf::from(value.as_str()))
                .or_else(|| std::env::var_os(key).map(PathBuf::from))
        })
        .filter(|path| path.is_absolute())
        .collect()
}
