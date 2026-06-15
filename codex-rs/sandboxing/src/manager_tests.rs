use super::SandboxCommand;
#[cfg(target_os = "windows")]
use super::SandboxDirectSpawnTransformRequest;
use super::SandboxManager;
use super::SandboxTransformRequest;
use super::SandboxType;
use super::SandboxablePreference;
use super::get_platform_sandbox;
use super::with_managed_mitm_ca_readable_root;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use dunce::canonicalize;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn danger_full_access_defaults_to_no_sandbox_without_network_requirements() {
    let manager = SandboxManager::new();
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ false,
    );
    assert_eq!(sandbox, SandboxType::None);
}

#[test]
fn danger_full_access_uses_platform_sandbox_with_network_requirements() {
    let manager = SandboxManager::new();
    let expected =
        get_platform_sandbox(/*windows_sandbox_enabled*/ false).unwrap_or(SandboxType::None);
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ true,
    );
    assert_eq!(sandbox, expected);
}

#[test]
fn restricted_file_system_uses_platform_sandbox_without_managed_network() {
    let manager = SandboxManager::new();
    let expected =
        get_platform_sandbox(/*windows_sandbox_enabled*/ false).unwrap_or(SandboxType::None);
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ false,
    );
    assert_eq!(sandbox, expected);
}

#[test]
fn transform_preserves_unrestricted_file_system_policy_for_restricted_network() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Restricted,
    );
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(exec_request.cwd, cwd);
    assert_eq!(exec_request.sandbox_policy_cwd, cwd);
    assert_eq!(
        exec_request.file_system_sandbox_policy,
        FileSystemSandboxPolicy::unrestricted()
    );
    assert_eq!(
        exec_request.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
}

#[test]
fn transform_additional_permissions_enable_network_for_external_sandbox() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: Some(AdditionalPermissionProfile {
                    network: Some(NetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(FileSystemPermissions::from_read_write_roots(
                        Some(vec![path]),
                        Some(Vec::new()),
                    )),
                }),
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.permission_profile,
        PermissionProfile::External {
            network: NetworkSandboxPolicy::Enabled,
        }
    );
    assert_eq!(
        exec_request.network_sandbox_policy,
        NetworkSandboxPolicy::Enabled
    );
}

#[test]
fn transform_additional_permissions_preserves_denied_entries() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let temp_dir = TempDir::new().expect("create temp dir");
    let workspace_root = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let allowed_path = workspace_root.join("allowed");
    let denied_path = workspace_root.join("denied");
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: denied_path.clone(),
            },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permissions = PermissionProfile::from_runtime_permissions(
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: Some(AdditionalPermissionProfile {
                    file_system: Some(FileSystemPermissions::from_read_write_roots(
                        /*read*/ None,
                        Some(vec![allowed_path.clone()]),
                    )),
                    ..Default::default()
                }),
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: denied_path },
                access: FileSystemAccessMode::Deny,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: allowed_path },
                access: FileSystemAccessMode::Write,
            },
        ])
    );
    assert_eq!(
        exec_request.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
}

#[test]
fn managed_mitm_ca_bundle_becomes_readable_for_restricted_sandbox() {
    let cwd = TempDir::new().expect("create cwd");
    let cwd =
        AbsolutePathBuf::from_absolute_path(canonicalize(cwd.path()).expect("canonicalize cwd"))
            .expect("absolute cwd");
    let managed_bundle_dir = TempDir::new().expect("create managed bundle dir");
    let managed_bundle_path =
        AbsolutePathBuf::from_absolute_path(managed_bundle_dir.path().join("ca-bundle.pem"))
            .expect("absolute managed bundle path");
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: cwd.clone() },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    );

    let permission_profile = with_managed_mitm_ca_readable_root(
        permission_profile,
        Some(&managed_bundle_path),
        cwd.as_path(),
    );
    let (file_system_sandbox_policy, _) = permission_profile.to_runtime_permissions();

    assert_eq!(
        file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: cwd },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: managed_bundle_path,
                },
                access: FileSystemAccessMode::Read,
            },
        ])
    );
}

#[cfg(target_os = "linux")]
fn transform_linux_seccomp_request(
    codex_linux_sandbox_exe: &std::path::Path,
) -> super::SandboxExecRequest {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = PermissionProfile::Disabled;
    manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::LinuxSeccomp,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: Some(codex_linux_sandbox_exe),
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform")
}

#[cfg(target_os = "linux")]
#[test]
fn wsl1_rejects_linux_bubblewrap_path() {
    let restricted_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }]);

    assert!(matches!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ false,
            /*is_wsl1*/ true,
        ),
        Err(super::SandboxTransformError::Wsl1UnsupportedForBubblewrap)
    ));
    assert!(matches!(
        super::ensure_linux_bubblewrap_is_supported(
            &FileSystemSandboxPolicy::unrestricted(),
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ true,
            /*is_wsl1*/ true,
        ),
        Err(super::SandboxTransformError::Wsl1UnsupportedForBubblewrap)
    ));
    assert!(matches!(
        super::ensure_linux_bubblewrap_is_supported(
            &FileSystemSandboxPolicy::unrestricted(),
            /*use_legacy_landlock*/ true,
            /*allow_network_for_proxy*/ true,
            /*is_wsl1*/ true,
        ),
        Err(super::SandboxTransformError::Wsl1UnsupportedForBubblewrap)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn wsl1_allows_non_bubblewrap_linux_paths() {
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &FileSystemSandboxPolicy::unrestricted(),
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ false,
            /*is_wsl1*/ true,
        )
        .is_ok()
    );

    let restricted_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }]);
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ true,
            /*allow_network_for_proxy*/ false,
            /*is_wsl1*/ true,
        )
        .is_ok()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn transform_linux_seccomp_preserves_helper_path_in_arg0_when_available() {
    let codex_linux_sandbox_exe = std::path::PathBuf::from("/tmp/codex-linux-sandbox");
    let exec_request = transform_linux_seccomp_request(&codex_linux_sandbox_exe);

    assert_eq!(
        exec_request.arg0,
        Some(codex_linux_sandbox_exe.to_string_lossy().into_owned())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn transform_linux_seccomp_uses_helper_alias_when_launcher_is_not_helper_path() {
    let codex_linux_sandbox_exe = std::path::PathBuf::from("/tmp/codex");
    let exec_request = transform_linux_seccomp_request(&codex_linux_sandbox_exe);

    assert_eq!(exec_request.arg0, Some("codex-linux-sandbox".to_string()));
}

#[cfg(target_os = "windows")]
#[test]
fn transform_for_direct_spawn_windows_preserves_only_wrapper_setup_identity() {
    let mut env = HashMap::from([
        ("Path".to_string(), r"C:\Windows\System32".to_string()),
        ("username".to_string(), "wrong-user".to_string()),
        ("UserProfile".to_string(), r"C:\wrong".to_string()),
    ]);

    super::add_windows_sandbox_wrapper_setup_env_from_vars(
        &mut env,
        [
            ("USERNAME", "alice"),
            ("USERPROFILE", r"C:\Users\alice"),
            ("OPENAI_API_KEY", "secret"),
        ]
        .map(|(key, value)| {
            (
                std::ffi::OsString::from(key),
                std::ffi::OsString::from(value),
            )
        }),
    );

    assert_eq!(
        env,
        HashMap::from([
            ("Path".to_string(), r"C:\Windows\System32".to_string()),
            ("USERNAME".to_string(), "alice".to_string()),
            ("USERPROFILE".to_string(), r"C:\Users\alice".to_string()),
        ])
    );
}

#[cfg(target_os = "windows")]
static CODEX_HOME_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(target_os = "windows")]
struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

#[cfg(target_os = "windows")]
impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

#[cfg(target_os = "windows")]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn transform_for_direct_spawn_windows_materializes_inner_helper() {
    let _lock = CODEX_HOME_TEST_LOCK.lock().expect("lock CODEX_HOME test");
    let codex_home = tempfile::TempDir::new().expect("codex home");
    let _guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    let helper_dir = tempfile::TempDir::new().expect("helper dir");
    let configured_helper = helper_dir.path().join("configured-codex-helper.exe");
    std::fs::write(&configured_helper, b"helper").expect("write configured helper");
    let cwd = AbsolutePathBuf::from_absolute_path(helper_dir.path()).expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let blocked = cwd.join("blocked");
    std::fs::create_dir_all(blocked.as_path()).expect("create blocked path");
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: blocked },
                access: FileSystemAccessMode::Deny,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    let other_workspace = tempfile::TempDir::new().expect("other workspace");
    let other_workspace_root = AbsolutePathBuf::from_absolute_path(other_workspace.path())
        .expect("absolute other workspace");
    let workspace_roots = vec![cwd, other_workspace_root];
    let manager = SandboxManager::new();
    let exec_request = manager
        .transform_for_direct_spawn(SandboxDirectSpawnTransformRequest {
            workspace_roots: workspace_roots.as_slice(),
            transform: SandboxTransformRequest {
                command: SandboxCommand {
                    program: configured_helper.as_os_str().to_owned(),
                    args: vec!["--codex-run-as-fs-helper".to_string()],
                    cwd: cwd_uri.clone(),
                    env: HashMap::from([("Path".to_string(), r"C:\Windows\System32".to_string())]),
                    additional_permissions: None,
                },
                permissions: &permissions,
                sandbox: SandboxType::WindowsRestrictedToken,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: &cwd_uri,
                codex_linux_sandbox_exe: None,
                use_legacy_landlock: false,
                windows_sandbox_level: WindowsSandboxLevel::Elevated,
                windows_sandbox_private_desktop: false,
            },
        })
        .expect("transform for direct spawn");

    let separator_index = exec_request
        .command
        .iter()
        .position(|arg| arg == "--")
        .expect("wrapper argv separator");
    let materialized_helper = std::path::PathBuf::from(&exec_request.command[separator_index + 1]);
    assert_eq!(exec_request.sandbox, SandboxType::None);
    assert_eq!(
        exec_request.command.first(),
        Some(&configured_helper.display().to_string())
    );
    assert!(
        exec_request
            .command
            .iter()
            .any(|arg| arg == "--run-as-windows-sandbox")
    );
    assert!(
        exec_request
            .command
            .iter()
            .any(|arg| arg == "--deny-read-paths-json")
    );
    assert_eq!(
        exec_request.command[separator_index + 2],
        "--codex-run-as-fs-helper"
    );
    assert_eq!(
        exec_request
            .command
            .windows(2)
            .filter_map(|args| {
                (args[0] == "--workspace-root").then_some(std::path::PathBuf::from(&args[1]))
            })
            .collect::<Vec<_>>(),
        workspace_roots
            .iter()
            .map(|root| root.as_path().to_path_buf())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        materialized_helper
            .parent()
            .and_then(std::path::Path::file_name),
        Some(std::ffi::OsStr::new(".sandbox-bin"))
    );
    assert!(materialized_helper.exists());
}
