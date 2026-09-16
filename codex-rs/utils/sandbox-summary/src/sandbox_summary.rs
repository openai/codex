use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

pub fn summarize_sandbox_policy(sandbox_policy: &SandboxPolicy) -> String {
    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
        SandboxPolicy::ReadOnly { network_access, .. } => {
            let mut summary = "read-only".to_string();
            if *network_access {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
        SandboxPolicy::ExternalSandbox { network_access } => {
            let mut summary = "external-sandbox".to_string();
            if matches!(network_access, NetworkAccess::Enabled) {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => {
            let mut summary = "workspace-write".to_string();

            let mut writable_entries = Vec::<String>::new();
            writable_entries.push("workdir".to_string());
            if !*exclude_slash_tmp {
                writable_entries.push("/tmp".to_string());
            }
            if !*exclude_tmpdir_env_var {
                writable_entries.push("$TMPDIR".to_string());
            }
            writable_entries.extend(
                writable_roots
                    .iter()
                    .map(|p| p.to_string_lossy().to_string()),
            );

            summary.push_str(&format!(" [{}]", writable_entries.join(", ")));
            if *network_access {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
    }
}

pub fn summarize_permission_profile(
    permission_profile: &PermissionProfile,
    cwd: &PathUri,
    workspace_roots: &[PathUri],
) -> String {
    let network_access = permission_profile.network_sandbox_policy().is_enabled();
    let mut summary = match permission_profile {
        PermissionProfile::Disabled => return "danger-full-access".to_string(),
        PermissionProfile::External { .. } => "external-sandbox".to_string(),
        PermissionProfile::Managed { .. } => {
            let policy = permission_profile.file_system_sandbox_policy();
            let context = FileSystemSandboxPolicyContext {
                cwd,
                workspace_roots,
                user_home_dir: None,
                temporary_directories: None,
            };
            if policy.has_full_disk_write_access_with_context(&context) {
                return if network_access {
                    "danger-full-access".to_string()
                } else {
                    "external-sandbox".to_string()
                };
            }

            // Keep the legacy summary categories without projecting executor
            // paths into the controller's filesystem namespace.
            let mut workspace_writable = false;
            let mut other_writes = false;
            let mut tmpdir_writable = false;
            let mut slash_tmp_writable = false;
            for entry in policy
                .entries
                .iter()
                .filter(|entry| entry.access.can_write())
            {
                match &entry.path {
                    FileSystemPath::Path { path } => {
                        if path.to_string() == cwd.to_string() {
                            workspace_writable = true;
                        } else {
                            other_writes = true;
                        }
                    }
                    FileSystemPath::Special { value } => match value {
                        FileSystemSpecialPath::ProjectRoots { subpath: None } => {
                            workspace_writable = true;
                        }
                        FileSystemSpecialPath::ProjectRoots { subpath: Some(_) } => {
                            other_writes = true;
                        }
                        FileSystemSpecialPath::Root => other_writes = true,
                        FileSystemSpecialPath::Tmpdir => tmpdir_writable = true,
                        FileSystemSpecialPath::SlashTmp => slash_tmp_writable = true,
                        FileSystemSpecialPath::Minimal | FileSystemSpecialPath::Unknown { .. } => {}
                    },
                    FileSystemPath::GlobPattern { .. } => {}
                }
            }

            if workspace_writable {
                let mut writable_entries = vec!["workdir".to_string()];
                if slash_tmp_writable {
                    writable_entries.push("/tmp".to_string());
                }
                if tmpdir_writable {
                    writable_entries.push("$TMPDIR".to_string());
                }
                writable_entries.extend(
                    workspace_roots
                        .iter()
                        .filter(|root| root.to_string() != cwd.to_string())
                        .map(PathUri::inferred_native_path_string),
                );
                format!("workspace-write [{}]", writable_entries.join(", "))
            } else if other_writes
                || tmpdir_writable
                || (cwd.infer_path_convention() == Some(PathConvention::Posix)
                    && slash_tmp_writable)
            {
                "custom permissions".to_string()
            } else {
                "read-only".to_string()
            }
        }
    };
    if network_access {
        summary.push_str(" (network access enabled)");
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::LegacyAppPathString;
    use pretty_assertions::assert_eq;

    #[test]
    fn summarizes_external_sandbox_without_network_access_suffix() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        });
        assert_eq!(summary, "external-sandbox");
    }

    #[test]
    fn summarizes_external_sandbox_with_enabled_network() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        });
        assert_eq!(summary, "external-sandbox (network access enabled)");
    }

    #[test]
    fn summarizes_read_only_with_enabled_network() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::ReadOnly {
            network_access: true,
        });
        assert_eq!(summary, "read-only (network access enabled)");
    }

    #[test]
    fn workspace_write_summary_still_includes_network_access() {
        let root = if cfg!(windows) { "C:\\repo" } else { "/repo" };
        let writable_root = AbsolutePathBuf::try_from(root).unwrap();
        let summary = summarize_sandbox_policy(&SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![writable_root.clone()],
            network_access: true,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        });
        assert_eq!(
            summary,
            format!(
                "workspace-write [workdir, {}] (network access enabled)",
                writable_root.to_string_lossy()
            )
        );
    }

    #[test]
    fn permission_profile_summary_uses_runtime_workspace_roots_and_hides_internal_writes() {
        let cwd =
            AbsolutePathBuf::try_from(if cfg!(windows) { "C:\\repo" } else { "/repo" }).unwrap();
        let extra_root = AbsolutePathBuf::try_from(if cfg!(windows) {
            "C:\\repo-extra"
        } else {
            "/repo-extra"
        })
        .unwrap();
        let hidden_root = AbsolutePathBuf::try_from(if cfg!(windows) {
            "C:\\Users\\test\\.codex\\memories"
        } else {
            "/Users/test/.codex/memories"
        })
        .unwrap();
        let profile = PermissionProfile::workspace_write_with(
            std::slice::from_ref(&hidden_root),
            NetworkSandboxPolicy::Restricted,
            /*exclude_tmpdir_env_var*/ false,
            /*exclude_slash_tmp*/ false,
        );

        let summary = summarize_permission_profile(
            &profile,
            &PathUri::from_abs_path(&cwd),
            &[cwd.clone().into(), extra_root.clone().into()],
        );

        assert_eq!(
            summary,
            format!(
                "workspace-write [workdir, /tmp, $TMPDIR, {}]",
                extra_root.display()
            )
        );
    }

    #[test]
    fn permission_profile_summary_preserves_executor_workspace_paths() {
        for (cwd, extra_root, expected) in [
            (
                "file:///workspace/repo",
                "file:///workspace/shared%20files",
                "workspace-write [workdir, /workspace/shared files]",
            ),
            (
                "file:///C:/workspace/repo",
                "file:///C:/workspace/shared%20files",
                r"workspace-write [workdir, C:\workspace\shared files]",
            ),
            (
                "file://server/share/repo",
                "file://server/share/shared%20files",
                r"workspace-write [workdir, \\server\share\shared files]",
            ),
            (
                "file:///C:/repo",
                "file:///C:/REPO",
                r"workspace-write [workdir, C:\REPO]",
            ),
        ] {
            let cwd = PathUri::parse(cwd).expect("executor cwd");
            let extra_root = PathUri::parse(extra_root).expect("executor workspace root");
            let roots = [cwd.clone(), extra_root];
            let profile = PermissionProfile::workspace_write_with_path_uris(
                &[],
                NetworkSandboxPolicy::Restricted,
                /*exclude_tmpdir_env_var*/ true,
                /*exclude_slash_tmp*/ true,
            )
            .materialize_project_roots_with_path_uris(&roots);

            assert_eq!(
                summarize_permission_profile(&profile, &cwd, &roots),
                expected
            );

            let profile = PermissionProfile::from_runtime_permissions(
                &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                    roots[1].clone().into(),
                    FileSystemAccessMode::Write,
                )]),
                NetworkSandboxPolicy::Restricted,
            );
            assert_eq!(
                summarize_permission_profile(&profile, &cwd, &roots),
                "custom permissions"
            );
        }
    }

    #[test]
    fn permission_profile_summary_keeps_opaque_workspace_subpath_writes_custom() {
        let cwd = LegacyAppPathString::from_string("/C:/repo")
            .to_path_uri(PathConvention::Posix)
            .expect("opaque POSIX cwd");
        for subpath in ["src", "", "."] {
            let profile = PermissionProfile::from_runtime_permissions(
                &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::ProjectRoots {
                            subpath: Some(subpath.to_string()),
                        },
                    },
                    FileSystemAccessMode::Write,
                )]),
                NetworkSandboxPolicy::Restricted,
            );
            assert_eq!(
                summarize_permission_profile(&profile, &cwd, std::slice::from_ref(&cwd)),
                "custom permissions"
            );
        }
    }
}
