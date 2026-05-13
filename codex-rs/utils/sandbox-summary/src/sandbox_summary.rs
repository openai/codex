use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
use std::path::Path;

pub fn summarize_permission_profile(
    permission_profile: &PermissionProfile,
    cwd: &AbsolutePathBuf,
    workspace_roots: &[AbsolutePathBuf],
) -> String {
    summarize_permission_profile_with_hidden_writable_roots(
        permission_profile,
        cwd,
        workspace_roots,
        &[],
    )
}

pub(crate) fn summarize_permission_profile_with_hidden_writable_roots(
    permission_profile: &PermissionProfile,
    cwd: &AbsolutePathBuf,
    workspace_roots: &[AbsolutePathBuf],
    hidden_writable_roots: &[AbsolutePathBuf],
) -> String {
    match permission_profile {
        PermissionProfile::Disabled => "danger-full-access".to_string(),
        PermissionProfile::External { network } => {
            let mut summary = "external-sandbox".to_string();
            append_network_summary(&mut summary, *network);
            summary
        }
        PermissionProfile::Managed { .. } => {
            let materialized_profile =
                permission_profile.materialize_project_roots_with_workspace_roots(workspace_roots);
            let file_system_policy = materialized_profile.file_system_sandbox_policy();
            let network_policy = materialized_profile.network_sandbox_policy();

            if file_system_policy.has_full_disk_write_access() {
                return if network_policy.is_enabled() {
                    "danger-full-access".to_string()
                } else {
                    "external-sandbox".to_string()
                };
            }

            let writable_roots = file_system_policy
                .get_writable_roots_with_cwd(cwd.as_path())
                .into_iter()
                .filter(|root| {
                    !hidden_writable_roots.iter().any(|hidden_root| {
                        paths_match_after_canonicalization(
                            root.root.as_path(),
                            hidden_root.as_path(),
                        )
                    })
                })
                .collect::<Vec<_>>();
            if writable_roots.is_empty() {
                let mut summary = "read-only".to_string();
                append_network_summary(&mut summary, network_policy);
                return summary;
            }

            let mut summary = "workspace-write".to_string();
            let writable_entries = writable_roots
                .iter()
                .map(|root| writable_root_label(root.root.as_path(), cwd.as_path()))
                .collect::<Vec<_>>();
            summary.push_str(&format!(" [{}]", writable_entries.join(", ")));
            append_network_summary(&mut summary, network_policy);
            summary
        }
    }
}

fn append_network_summary(summary: &mut String, network_policy: NetworkSandboxPolicy) {
    if network_policy.is_enabled() {
        summary.push_str(" (network access enabled)");
    }
}

fn writable_root_label(root: &Path, cwd: &Path) -> String {
    if paths_match_after_canonicalization(root, cwd) {
        return "workdir".to_string();
    }
    if paths_match_after_canonicalization(root, Path::new("/tmp")) {
        return "/tmp".to_string();
    }
    if std::env::var_os("TMPDIR")
        .filter(|tmpdir| !tmpdir.is_empty())
        .is_some_and(|tmpdir| paths_match_after_canonicalization(root, Path::new(&tmpdir)))
    {
        return "$TMPDIR".to_string();
    }
    display_path_label(root)
}

fn paths_match_after_canonicalization(left: &Path, right: &Path) -> bool {
    match (
        canonicalize_preserving_symlinks(left),
        canonicalize_preserving_symlinks(right),
    ) {
        (Ok(left), Ok(right)) if left == right => true,
        _ => display_path_label(left) == display_path_label(right),
    }
}

fn display_path_label(path: &Path) -> String {
    path.strip_prefix("/private/tmp")
        .ok()
        .map(|suffix| Path::new("/tmp").join(suffix))
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    #[test]
    fn summarizes_disabled_permission_profile_as_danger_full_access() {
        let cwd = test_path_buf("/tmp/project").abs();
        let summary = summarize_permission_profile(&PermissionProfile::Disabled, &cwd, &[]);
        assert_eq!(summary, "danger-full-access");
    }

    #[test]
    fn summarizes_external_sandbox_without_network_access_suffix() {
        let cwd = test_path_buf("/tmp/project").abs();
        let summary = summarize_permission_profile(
            &PermissionProfile::External {
                network: NetworkSandboxPolicy::Restricted,
            },
            &cwd,
            &[],
        );
        assert_eq!(summary, "external-sandbox");
    }

    #[test]
    fn summarizes_external_sandbox_with_enabled_network() {
        let cwd = test_path_buf("/tmp/project").abs();
        let summary = summarize_permission_profile(
            &PermissionProfile::External {
                network: NetworkSandboxPolicy::Enabled,
            },
            &cwd,
            &[],
        );
        assert_eq!(summary, "external-sandbox (network access enabled)");
    }

    #[test]
    fn summarizes_read_only_with_enabled_network() {
        let cwd = test_path_buf("/tmp/project").abs();
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(Vec::new()),
            NetworkSandboxPolicy::Enabled,
        );
        let summary = summarize_permission_profile(&profile, &cwd, &[]);
        assert_eq!(summary, "read-only (network access enabled)");
    }

    #[test]
    fn summarizes_unrestricted_managed_profile_with_enabled_network_as_danger_full_access() {
        let cwd = test_path_buf("/tmp/project").abs();
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Enabled,
        );
        let summary = summarize_permission_profile(&profile, &cwd, &[]);
        assert_eq!(summary, "danger-full-access");
    }

    #[test]
    fn summarizes_unrestricted_managed_profile_without_network_as_external_sandbox() {
        let cwd = test_path_buf("/tmp/project").abs();
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Restricted,
        );
        let summary = summarize_permission_profile(&profile, &cwd, &[]);
        assert_eq!(summary, "external-sandbox");
    }

    #[test]
    fn workspace_write_summary_includes_workspace_roots_and_network_access() {
        let cwd = test_path_buf("/tmp/project").abs();
        let cache_root = test_path_buf("/tmp/cache").abs();
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: codex_protocol::permissions::FileSystemSpecialPath::ProjectRoots {
                        subpath: None,
                    },
                },
                access: FileSystemAccessMode::Write,
            }]),
            NetworkSandboxPolicy::Enabled,
        );
        let summary =
            summarize_permission_profile(&profile, &cwd, &[cwd.clone(), cache_root.clone()]);
        assert_eq!(
            summary,
            format!(
                "workspace-write [workdir, {}] (network access enabled)",
                cache_root.display()
            )
        );
    }

    #[test]
    fn workspace_write_summary_does_not_treat_cwd_as_root_unless_listed() {
        let cwd = test_path_buf("/tmp/project").abs();
        let cache_root = test_path_buf("/tmp/cache").abs();
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: codex_protocol::permissions::FileSystemSpecialPath::ProjectRoots {
                        subpath: None,
                    },
                },
                access: FileSystemAccessMode::Write,
            }]),
            NetworkSandboxPolicy::Restricted,
        );
        let summary =
            summarize_permission_profile(&profile, &cwd, std::slice::from_ref(&cache_root));
        assert_eq!(
            summary,
            format!("workspace-write [{}]", cache_root.display())
        );
    }

    #[test]
    fn workspace_write_summary_hides_internal_writable_roots() {
        let cwd = test_path_buf("/tmp/project").abs();
        let memories_root = test_path_buf("/tmp/codex-home/memories").abs();
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: codex_protocol::permissions::FileSystemSpecialPath::ProjectRoots {
                            subpath: None,
                        },
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: memories_root.clone(),
                    },
                    access: FileSystemAccessMode::Write,
                },
            ]),
            NetworkSandboxPolicy::Restricted,
        );
        let summary = summarize_permission_profile_with_hidden_writable_roots(
            &profile,
            &cwd,
            std::slice::from_ref(&cwd),
            std::slice::from_ref(&memories_root),
        );
        assert_eq!(summary, "workspace-write [workdir]");
    }

    #[test]
    fn distinct_missing_paths_do_not_match_after_canonicalization() {
        assert!(!paths_match_after_canonicalization(
            test_path_buf("/tmp/codex-missing-left").as_path(),
            test_path_buf("/tmp/codex-missing-right").as_path(),
        ));
    }
}
