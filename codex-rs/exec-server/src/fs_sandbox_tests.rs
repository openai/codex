use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use crate::ExecServerRuntimePaths;
use crate::FileSystemSandboxContext;

use super::FileSystemSandboxRunner;
use super::PATH_ENV_VAR;
use super::helper_env;
use super::helper_sandbox_inputs;
use super::sandbox_policy_with_helper_runtime_defaults;

#[test]
fn helper_sandbox_policy_enables_platform_defaults_for_read_only_access() {
    let sandbox_policy = SandboxPolicy::ReadOnly {
        access: ReadOnlyAccess::Restricted {
            include_platform_defaults: false,
            readable_roots: Vec::new(),
        },
        network_access: false,
    };

    let updated = sandbox_policy_with_helper_runtime_defaults(&sandbox_policy);

    assert_eq!(
        updated,
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: Vec::new(),
            },
            network_access: false,
        }
    );
}

#[test]
fn helper_sandbox_policy_enables_platform_defaults_for_workspace_read_access() {
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        read_only_access: ReadOnlyAccess::Restricted {
            include_platform_defaults: false,
            readable_roots: Vec::new(),
        },
        network_access: true,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let updated = sandbox_policy_with_helper_runtime_defaults(&sandbox_policy);

    assert_eq!(
        updated,
        SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: Vec::new(),
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        }
    );
}

#[test]
fn helper_sandbox_inputs_use_context_cwd_and_file_system_policy() {
    let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
        .expect("absolute temp dir");
    let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
    let file_system_policy =
        codex_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy(
            &sandbox_policy,
            cwd.as_path(),
        );
    let mut sandbox_context = FileSystemSandboxContext::new(sandbox_policy.clone());
    sandbox_context.sandbox_policy_cwd = Some(cwd.clone());
    sandbox_context.file_system_sandbox_policy = Some(file_system_policy.clone());

    let inputs = helper_sandbox_inputs(&sandbox_context).expect("helper sandbox inputs");

    assert_eq!(inputs.cwd, cwd);
    assert_eq!(inputs.sandbox_policy, sandbox_policy);
    assert_eq!(inputs.file_system_policy, file_system_policy);
    assert_eq!(inputs.network_policy, NetworkSandboxPolicy::Restricted);
}

#[test]
fn helper_sandbox_inputs_rejects_file_system_policy_without_cwd() {
    let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
        .expect("absolute temp dir");
    let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
    let file_system_policy =
        codex_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy(
            &sandbox_policy,
            cwd.as_path(),
        );
    let mut sandbox_context = FileSystemSandboxContext::new(sandbox_policy);
    sandbox_context.file_system_sandbox_policy = Some(file_system_policy);

    let err = match helper_sandbox_inputs(&sandbox_context) {
        Ok(_) => panic!("expected invalid sandbox inputs"),
        Err(err) => err,
    };

    assert_eq!(
        err.message,
        "fileSystemSandboxPolicy requires sandboxPolicyCwd"
    );
}

#[test]
fn helper_env_preserves_only_path() {
    let env = helper_env();

    match std::env::var_os(PATH_ENV_VAR) {
        Some(path) => assert_eq!(
            env,
            std::collections::HashMap::from([(
                PATH_ENV_VAR.to_string(),
                path.to_string_lossy().into_owned()
            )])
        ),
        None => assert_eq!(env, std::collections::HashMap::new()),
    }
}

#[test]
fn helper_permissions_strip_network_grants() {
    let codex_self_exe = std::env::current_exe().expect("current exe");
    let runtime_paths = ExecServerRuntimePaths::new(
        codex_self_exe.clone(),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths");
    let runner = FileSystemSandboxRunner::new(runtime_paths);
    let readable =
        AbsolutePathBuf::from_absolute_path(codex_self_exe.parent().expect("current exe parent"))
            .expect("absolute readable path");
    let writable = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().as_path())
        .expect("absolute writable path");

    let permissions = runner
        .helper_permissions(
            Some(&PermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![]),
                    write: Some(vec![writable.clone()]),
                }),
            }),
            /*include_helper_read_root*/ true,
        )
        .expect("helper permissions");

    assert_eq!(permissions.network, None);
    assert_eq!(
        permissions
            .file_system
            .as_ref()
            .and_then(|fs| fs.write.clone()),
        Some(vec![writable])
    );
    assert_eq!(
        permissions
            .file_system
            .as_ref()
            .and_then(|fs| fs.read.clone()),
        Some(vec![readable])
    );
}

#[test]
fn helper_permissions_include_helper_read_root_without_additional_permissions() {
    let codex_self_exe = std::env::current_exe().expect("current exe");
    let runtime_paths = ExecServerRuntimePaths::new(
        codex_self_exe.clone(),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths");
    let runner = FileSystemSandboxRunner::new(runtime_paths);
    let readable =
        AbsolutePathBuf::from_absolute_path(codex_self_exe.parent().expect("current exe parent"))
            .expect("absolute readable path");

    let permissions = runner
        .helper_permissions(
            /*additional_permissions*/ None, /*include_helper_read_root*/ true,
        )
        .expect("helper permissions");

    assert_eq!(permissions.network, None);
    assert_eq!(
        permissions.file_system,
        Some(FileSystemPermissions {
            read: Some(vec![readable]),
            write: None,
        })
    );
}

#[test]
fn legacy_landlock_helper_permissions_do_not_add_helper_read_root() {
    let codex_self_exe = std::env::current_exe().expect("current exe");
    let runtime_paths =
        ExecServerRuntimePaths::new(codex_self_exe, /*codex_linux_sandbox_exe*/ None)
            .expect("runtime paths");
    let runner = FileSystemSandboxRunner::new(runtime_paths);

    let permissions = runner.helper_permissions(
        /*additional_permissions*/ None, /*include_helper_read_root*/ false,
    );

    assert_eq!(permissions, None);
}
