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
fn mxc_selection_preserves_unsandboxed_requests_and_other_platforms() {
    let manager = SandboxManager::new();
    let platform_sandbox = if cfg!(windows) {
        SandboxType::WindowsMxc
    } else {
        get_platform_sandbox(/*windows_sandbox_enabled*/ false).unwrap_or(SandboxType::None)
    };
    for (permissions, preference, expected) in [
        (
            PermissionProfile::read_only(),
            SandboxablePreference::Auto,
            platform_sandbox,
        ),
        (
            PermissionProfile::Disabled,
            SandboxablePreference::Require,
            platform_sandbox,
        ),
        (
            PermissionProfile::read_only(),
            SandboxablePreference::Forbid,
            SandboxType::None,
        ),
        (
            PermissionProfile::Disabled,
            SandboxablePreference::Auto,
            SandboxType::None,
        ),
    ] {
        assert_eq!(
            manager.select_initial(
                &permissions,
                preference,
                WindowsSandboxLevel::Mxc,
                /*has_managed_network_requirements*/ false,
            ),
            expected,
        );
    }
}

#[test]
fn danger_full_access_defaults_to_no_sandbox_without_network_requirements() {
    let manager = SandboxManager::new();
    let sandbox = manager.select_initial(
        &PermissionProfile::Disabled,
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
        &PermissionProfile::Disabled,
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
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        }]),
        NetworkSandboxPolicy::Enabled,
    );
    let sandbox = manager.select_initial(
        &permissions,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ false,
    );
    assert_eq!(sandbox, expected);
}

#[test]
fn unsandboxed_transform_preserves_foreign_cwd_and_unrestricted_file_system_policy() {
    let manager = SandboxManager::new();
    let cwd_uri = if cfg!(windows) {
        PathUri::parse("file:///workspace/remote").expect("POSIX path URI")
    } else {
        PathUri::parse("file:///C:/workspace/remote").expect("Windows path URI")
    };
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
                managed_network: None,
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(exec_request.cwd, cwd_uri);
    assert_eq!(exec_request.sandbox_policy_cwd, cwd_uri);
    assert_eq!(
        exec_request.permission_profile.file_system_sandbox_policy(),
        FileSystemSandboxPolicy::unrestricted()
    );
    assert_eq!(
        exec_request.permission_profile.network_sandbox_policy(),
        NetworkSandboxPolicy::Restricted
    );
}

#[cfg(target_os = "macos")]
#[test]
fn symlinked_workspace_reports_seatbelt_preparation_error() {
    use std::os::unix::fs::symlink;

    let manager = SandboxManager::new();
    let temp_dir = TempDir::new().expect("create temp dir");
    let target = temp_dir.path().join("target");
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir(&target).expect("create target");
    symlink(&target, &workspace).expect("create symlinked workspace");
    let workspace = AbsolutePathBuf::from_absolute_path(workspace).expect("absolute workspace");
    let workspace_uri = PathUri::from_abs_path(&workspace);
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::workspace_write(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        ),
        NetworkSandboxPolicy::Restricted,
    );

    let error = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: workspace_uri.clone(),
                env: HashMap::new(),
                managed_network: None,
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::MacosSeatbelt,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &workspace_uri,
            sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect_err("symlinked workspace should be rejected");

    assert!(matches!(
        &error,
        super::SandboxTransformError::SeatbeltPreparation(message)
            if message.contains("symlinked writable roots are not supported")
    ));
    assert!(
        !error.to_string().contains("network proxy"),
        "filesystem error should not be attributed to network proxy: {error}"
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
                managed_network: None,
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
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            sandbox_exe: None,
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
        exec_request.permission_profile.network_sandbox_policy(),
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
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: denied_path.clone().into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
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
                managed_network: None,
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
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.permission_profile.file_system_sandbox_policy(),
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: denied_path.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: allowed_path.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ])
    );
    assert_eq!(
        exec_request.permission_profile.network_sandbox_policy(),
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
            path: cwd.clone().into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
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
                path: cwd.into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: managed_bundle_path.into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
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
                managed_network: None,
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::LinuxSeccomp,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            sandbox_exe: Some(codex_linux_sandbox_exe),
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
        missing_path_behavior: None,
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
        missing_path_behavior: None,
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

#[cfg(unix)]
#[tokio::test]
async fn linux_unix_socket_grant_uses_effective_managed_policy() -> anyhow::Result<()> {
    use codex_network_proxy::ConfigReloader;
    use codex_network_proxy::ConfigReloaderFuture;
    use codex_network_proxy::ConfigState;
    use codex_network_proxy::ManagedNetworkSandboxContext;
    use codex_network_proxy::NetworkProxy;
    use codex_network_proxy::NetworkProxyConfig;
    use codex_network_proxy::NetworkProxyConstraints;
    use codex_network_proxy::NetworkProxyState;
    use codex_network_proxy::build_config_state;
    use std::sync::Arc;

    struct TestConfigReloader;
    impl ConfigReloader for TestConfigReloader {
        fn source_label(&self) -> String {
            "sandbox manager test config".to_string()
        }

        fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
            Box::pin(async { Ok(None) })
        }

        fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
            Box::pin(async { Err(anyhow::anyhow!("test config cannot reload")) })
        }
    }

    let state = build_config_state(
        NetworkProxyConfig {
            enabled: true,
            dangerously_allow_all_unix_sockets: true,
            ..Default::default()
        },
        NetworkProxyConstraints::default(),
    )?;
    let network = NetworkProxy::builder()
        .state(Arc::new(NetworkProxyState::with_reloader(
            state,
            Arc::new(TestConfigReloader),
        )))
        .managed_by_codex(/*managed_by_codex*/ false)
        .build()
        .await?;
    let prepared =
        network.prepare_for_optional_environment(HashMap::new(), /*environment_id*/ None)?;
    let allow_all = prepared.sandbox_context;
    let path_only = ManagedNetworkSandboxContext {
        allow_unix_sockets: vec!["/tmp/daemon.sock".to_string()],
        ..Default::default()
    };
    let denied = ManagedNetworkSandboxContext::default();
    let cwd = AbsolutePathBuf::current_dir()?;
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let manager = SandboxManager::new();
    for (label, context, live_proxy, enforce_managed_network, expected) in [
        (
            "missing policy stays managed with restrictive defaults",
            None,
            false,
            true,
            Some(denied.clone()),
        ),
        (
            "path grant stays restricted",
            Some(path_only.clone()),
            false,
            true,
            Some(path_only),
        ),
        (
            "prepared allow-all",
            Some(allow_all.clone()),
            false,
            true,
            Some(allow_all.clone()),
        ),
        (
            "prepared denial overrides live allow-all",
            Some(denied.clone()),
            true,
            true,
            Some(denied),
        ),
        (
            "live proxy fallback",
            None,
            true,
            true,
            Some(allow_all.clone()),
        ),
        ("no managed network", Some(allow_all), true, false, None),
    ] {
        let request = manager.transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                managed_network: context,
                additional_permissions: None,
            },
            permissions: &PermissionProfile::Disabled,
            sandbox: SandboxType::LinuxSeccomp,
            enforce_managed_network,
            environment_id: None,
            network: live_proxy.then_some(&network),
            sandbox_policy_cwd: &cwd_uri,
            sandbox_exe: Some(std::path::Path::new("/tmp/codex-linux-sandbox")),
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })?;
        let transported_context = request
            .command
            .windows(2)
            .find(|args| args[0] == "--managed-network")
            .map(|args| serde_json::from_str::<ManagedNetworkSandboxContext>(&args[1]))
            .transpose()?;
        assert_eq!(transported_context, expected, "{label}");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[test]
fn transform_for_direct_spawn_windows_preserves_only_wrapper_setup_environment() {
    let mut env = HashMap::from([
        ("Path".to_string(), r"C:\Windows\System32".to_string()),
        ("username".to_string(), "wrong-user".to_string()),
        ("UserProfile".to_string(), r"C:\wrong".to_string()),
        ("SYSTEMROOT".to_string(), r"C:\wrong".to_string()),
    ]);

    super::add_windows_sandbox_wrapper_setup_env_from_vars(
        &mut env,
        [
            ("USERNAME", "alice"),
            ("USERPROFILE", r"C:\Users\alice"),
            ("SystemRoot", r"C:\Windows"),
            ("OPENAI_API_KEY", "secret"),
            ("HTTP_PROXY", "http://127.0.0.1:7890"),
        ]
        .map(|(key, value)| {
            (
                std::ffi::OsString::from(key),
                std::ffi::OsString::from(value),
            )
        }),
        /*registered_core*/ false,
    );

    assert_eq!(
        env,
        HashMap::from([
            ("Path".to_string(), r"C:\Windows\System32".to_string()),
            ("USERNAME".to_string(), "alice".to_string()),
            ("USERPROFILE".to_string(), r"C:\Users\alice".to_string()),
            ("SystemRoot".to_string(), r"C:\Windows".to_string()),
        ])
    );
}

#[cfg(target_os = "windows")]
#[test]
fn wrapper_runtime_selection_uses_the_parent_not_environment_overrides() {
    for registered_core in [false, true] {
        let mut env = HashMap::from([("codex_windows_registered_core".into(), "1".into())]);
        super::add_windows_sandbox_wrapper_setup_env_from_vars(
            &mut env,
            [("CODEX_WINDOWS_REGISTERED_CORE".into(), "0".into())],
            registered_core,
        );
        assert_eq!(
            env,
            if registered_core {
                HashMap::from([("CODEX_WINDOWS_REGISTERED_CORE".into(), "1".into())])
            } else {
                HashMap::new()
            }
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn transform_for_direct_spawn_windows_materializes_inner_helper() {
    let codex_home = tempfile::TempDir::new().expect("codex home");
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
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: blocked.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
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
        .transform_for_direct_spawn_with_codex_home(
            SandboxDirectSpawnTransformRequest {
                workspace_roots: workspace_roots.as_slice(),
                windows_sandbox_proxy_settings_mode:
                    codex_windows_sandbox::WindowsSandboxProxySettingsMode::Preserve,
                transform: SandboxTransformRequest {
                    command: SandboxCommand {
                        program: configured_helper.as_os_str().to_owned(),
                        args: vec!["--codex-run-as-fs-helper".to_string()],
                        cwd: cwd_uri.clone(),
                        env: HashMap::from([(
                            "Path".to_string(),
                            r"C:\Windows\System32".to_string(),
                        )]),
                        managed_network: None,
                        additional_permissions: None,
                    },
                    permissions: &permissions,
                    sandbox: SandboxType::WindowsRestrictedToken,
                    enforce_managed_network: false,
                    environment_id: None,
                    network: None,
                    sandbox_policy_cwd: &cwd_uri,
                    sandbox_exe: None,
                    use_legacy_landlock: false,
                    windows_sandbox_level: WindowsSandboxLevel::Elevated,
                    windows_sandbox_private_desktop: false,
                },
            },
            codex_home.path(),
        )
        .expect("transform for direct spawn");

    let inner_env_json = exec_request
        .command
        .windows(2)
        .find(|args| args[0] == "--env-json")
        .expect("inner command environment");
    assert_eq!(
        serde_json::from_str::<HashMap<String, String>>(&inner_env_json[1])
            .expect("decode inner command environment"),
        HashMap::from([("Path".to_string(), r"C:\Windows\System32".to_string())])
    );

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
            .any(|arg| arg == "--preserve-proxy-settings")
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
