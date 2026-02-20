use std::collections::HashSet;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::protocol::NetworkAccess;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;

pub(crate) fn extend_sandbox_policy(
    base: &SandboxPolicy,
    extension: &SandboxPolicy,
) -> SandboxPolicy {
    // Merge by intersection of capabilities: the combined policy must satisfy
    // restrictions from both `base` and `extension`.
    match (base, extension) {
        (SandboxPolicy::DangerFullAccess, other) | (other, SandboxPolicy::DangerFullAccess) => {
            other.clone()
        }
        (
            SandboxPolicy::ExternalSandbox {
                network_access: base_network,
            },
            SandboxPolicy::ExternalSandbox {
                network_access: extension_network,
            },
        ) => SandboxPolicy::ExternalSandbox {
            network_access: restrict_network_access(*base_network, *extension_network),
        },
        (SandboxPolicy::ExternalSandbox { .. }, SandboxPolicy::ReadOnly { access })
        | (SandboxPolicy::ReadOnly { access }, SandboxPolicy::ExternalSandbox { .. }) => {
            SandboxPolicy::ReadOnly {
                access: access.clone(),
            }
        }
        (
            SandboxPolicy::ExternalSandbox {
                network_access: external_network_access,
            },
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            },
        )
        | (
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            },
            SandboxPolicy::ExternalSandbox {
                network_access: external_network_access,
            },
        ) => SandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots.clone(),
            read_only_access: read_only_access.clone(),
            network_access: *network_access && external_network_access.is_enabled(),
            exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
            exclude_slash_tmp: *exclude_slash_tmp,
        },
        (
            SandboxPolicy::ReadOnly {
                access: base_access,
            },
            SandboxPolicy::ReadOnly {
                access: extension_access,
            },
        ) => SandboxPolicy::ReadOnly {
            access: intersect_read_only_access(base_access, extension_access),
        },
        (
            SandboxPolicy::ReadOnly {
                access: base_access,
            },
            SandboxPolicy::WorkspaceWrite {
                read_only_access, ..
            },
        ) => SandboxPolicy::ReadOnly {
            access: intersect_read_only_access(base_access, read_only_access),
        },
        (
            SandboxPolicy::WorkspaceWrite {
                read_only_access, ..
            },
            SandboxPolicy::ReadOnly {
                access: extension_access,
            },
        ) => SandboxPolicy::ReadOnly {
            access: intersect_read_only_access(read_only_access, extension_access),
        },
        (
            SandboxPolicy::WorkspaceWrite {
                writable_roots: base_writable_roots,
                read_only_access: base_read_only_access,
                network_access: base_network_access,
                exclude_tmpdir_env_var: base_exclude_tmpdir_env_var,
                exclude_slash_tmp: base_exclude_slash_tmp,
            },
            SandboxPolicy::WorkspaceWrite {
                writable_roots: extension_writable_roots,
                read_only_access: extension_read_only_access,
                network_access: extension_network_access,
                exclude_tmpdir_env_var: extension_exclude_tmpdir_env_var,
                exclude_slash_tmp: extension_exclude_slash_tmp,
            },
        ) => SandboxPolicy::WorkspaceWrite {
            writable_roots: intersect_absolute_roots(base_writable_roots, extension_writable_roots),
            read_only_access: intersect_read_only_access(
                base_read_only_access,
                extension_read_only_access,
            ),
            network_access: *base_network_access && *extension_network_access,
            exclude_tmpdir_env_var: *base_exclude_tmpdir_env_var
                || *extension_exclude_tmpdir_env_var,
            exclude_slash_tmp: *base_exclude_slash_tmp || *extension_exclude_slash_tmp,
        },
    }
}

fn restrict_network_access(base: NetworkAccess, extension: NetworkAccess) -> NetworkAccess {
    if base.is_enabled() && extension.is_enabled() {
        NetworkAccess::Enabled
    } else {
        NetworkAccess::Restricted
    }
}

fn intersect_read_only_access(base: &ReadOnlyAccess, extension: &ReadOnlyAccess) -> ReadOnlyAccess {
    match (base, extension) {
        (ReadOnlyAccess::FullAccess, access) | (access, ReadOnlyAccess::FullAccess) => {
            access.clone()
        }
        (
            ReadOnlyAccess::Restricted {
                include_platform_defaults: base_include_platform_defaults,
                readable_roots: base_readable_roots,
            },
            ReadOnlyAccess::Restricted {
                include_platform_defaults: extension_include_platform_defaults,
                readable_roots: extension_readable_roots,
            },
        ) => ReadOnlyAccess::Restricted {
            include_platform_defaults: *base_include_platform_defaults
                && *extension_include_platform_defaults,
            readable_roots: intersect_absolute_roots(base_readable_roots, extension_readable_roots),
        },
    }
}

fn intersect_absolute_roots(
    base_roots: &[AbsolutePathBuf],
    extension_roots: &[AbsolutePathBuf],
) -> Vec<AbsolutePathBuf> {
    let extension_roots_set: HashSet<_> = extension_roots
        .iter()
        .map(AbsolutePathBuf::to_path_buf)
        .collect();
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for root in base_roots {
        let root_path = root.to_path_buf();
        if extension_roots_set.contains(&root_path) && seen.insert(root_path) {
            roots.push(root.clone());
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::extend_sandbox_policy;
    use crate::protocol::NetworkAccess;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    #[test]
    fn extend_sandbox_policy_combines_read_only_and_workspace_write_as_read_only() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let base_read_root =
            AbsolutePathBuf::try_from(tempdir.path().join("base-read")).expect("absolute path");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![base_read_root],
                },
            },
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: Vec::new(),
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: Vec::new(),
                },
            }
        );
    }

    #[test]
    fn extend_sandbox_policy_uses_extension_when_base_is_danger_full_access() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let extension_root =
            AbsolutePathBuf::try_from(tempdir.path().join("extension")).expect("absolute path");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::DangerFullAccess,
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![extension_root.clone()],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: true,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![extension_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: true,
            }
        );
    }

    #[test]
    fn extend_sandbox_policy_external_and_workspace_write_keeps_workspace_write_restrictions() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let workspace_root =
            AbsolutePathBuf::try_from(tempdir.path().join("workspace")).expect("absolute path");
        let read_root = AbsolutePathBuf::try_from(tempdir.path().join("read")).expect("absolute");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![workspace_root.clone()],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![read_root.clone()],
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![workspace_root],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![read_root],
                },
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        );
    }

    #[test]
    fn extend_sandbox_policy_external_and_read_only_returns_read_only() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let read_root = AbsolutePathBuf::try_from(tempdir.path().join("read")).expect("absolute");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            },
            &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![read_root.clone()],
                },
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![read_root],
                },
            }
        );
    }

    #[test]
    fn extend_sandbox_policy_intersects_workspace_roots_and_restricts_network_access() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let shared_root =
            AbsolutePathBuf::try_from(tempdir.path().join("shared")).expect("absolute path");
        let base_root =
            AbsolutePathBuf::try_from(tempdir.path().join("base")).expect("absolute path");
        let extension_root =
            AbsolutePathBuf::try_from(tempdir.path().join("extension")).expect("absolute path");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root.clone(), base_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root.clone(), extension_root.clone()],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![extension_root.clone()],
                },
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![extension_root],
                },
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }

    #[test]
    fn extend_sandbox_policy_keeps_network_access_enabled_only_when_both_policies_enable_it() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let shared_root =
            AbsolutePathBuf::try_from(tempdir.path().join("shared")).expect("absolute path");
        let base_root =
            AbsolutePathBuf::try_from(tempdir.path().join("base")).expect("absolute path");
        let extension_root =
            AbsolutePathBuf::try_from(tempdir.path().join("extension")).expect("absolute path");

        let merged = extend_sandbox_policy(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![base_root, shared_root.clone()],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root.clone(), extension_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
        );

        assert_eq!(
            merged,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![shared_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }
}
