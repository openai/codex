use std::num::NonZeroUsize;

use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn sandbox_context_round_trips_between_core_and_exec_server_types() {
    let contexts = vec![
        FileSystemSandboxContext {
            permissions: PermissionProfile::Managed {
                file_system: ManagedFileSystemPermissions::Restricted {
                    entries: all_path_variants(),
                    glob_scan_max_depth: NonZeroUsize::new(4),
                },
                network: NetworkSandboxPolicy::Enabled,
            },
            cwd: Some(path_uri("workspace")),
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            windows_sandbox_private_desktop: true,
            use_legacy_landlock: true,
        },
        FileSystemSandboxContext {
            permissions: PermissionProfile::Managed {
                file_system: ManagedFileSystemPermissions::Unrestricted,
                network: NetworkSandboxPolicy::Restricted,
            },
            cwd: None,
            windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
            windows_sandbox_private_desktop: false,
            use_legacy_landlock: false,
        },
        FileSystemSandboxContext {
            permissions: PermissionProfile::Disabled,
            cwd: None,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
            use_legacy_landlock: false,
        },
        FileSystemSandboxContext {
            permissions: PermissionProfile::External {
                network: NetworkSandboxPolicy::Enabled,
            },
            cwd: None,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
            use_legacy_landlock: false,
        },
    ];

    for context in contexts {
        let exec_server_context = ExecServerFileSystemSandboxContext::from(context.clone());
        let round_trip = FileSystemSandboxContext::try_from(exec_server_context)
            .expect("exec-server context should convert to native core paths");
        assert_eq!(round_trip, context);
    }
}

#[test]
fn sandbox_context_serializes_concrete_paths_as_uris() {
    let native_path = absolute_path("readable");
    let path = PathUri::from_abs_path(&native_path);
    let cwd = path_uri("workspace");
    let context = ExecServerFileSystemSandboxContext {
        permissions: ExecServerPermissionProfile::Managed {
            file_system: ExecServerManagedFileSystemPermissions::Restricted {
                entries: vec![
                    ExecServerFileSystemSandboxEntry {
                        path: ExecServerFileSystemPath::Path { path: path.clone() },
                        access: ExecServerFileSystemAccessMode::Read,
                    },
                    ExecServerFileSystemSandboxEntry {
                        path: ExecServerFileSystemPath::GlobPattern {
                            pattern: "**/*.env".to_string(),
                        },
                        access: ExecServerFileSystemAccessMode::Deny,
                    },
                    ExecServerFileSystemSandboxEntry {
                        path: ExecServerFileSystemPath::Special {
                            value: ExecServerFileSystemSpecialPath::ProjectRoots {
                                subpath: Some("docs".into()),
                            },
                        },
                        access: ExecServerFileSystemAccessMode::Write,
                    },
                ],
                glob_scan_max_depth: NonZeroUsize::new(3),
            },
            network: ExecServerNetworkSandboxPolicy::Restricted,
        },
        cwd: Some(cwd.clone()),
        windows_sandbox_level: ExecServerWindowsSandboxLevel::RestrictedToken,
        windows_sandbox_private_desktop: true,
        use_legacy_landlock: true,
    };
    let expected = json!({
        "permissions": {
            "type": "managed",
            "file_system": {
                "type": "restricted",
                "entries": [
                    {
                        "path": {"type": "path", "path": path.to_string()},
                        "access": "read"
                    },
                    {
                        "path": {"type": "glob_pattern", "pattern": "**/*.env"},
                        "access": "deny"
                    },
                    {
                        "path": {
                            "type": "special",
                            "value": {"kind": "project_roots", "subpath": "docs"}
                        },
                        "access": "write"
                    }
                ],
                "glob_scan_max_depth": 3
            },
            "network": "restricted"
        },
        "cwd": cwd.to_string(),
        "windowsSandboxLevel": "restricted-token",
        "windowsSandboxPrivateDesktop": true,
        "useLegacyLandlock": true
    });

    let serialized = serde_json::to_value(&context).expect("sandbox context should serialize");
    assert_eq!(serialized, expected);
    assert_eq!(
        serde_json::from_value::<ExecServerFileSystemSandboxContext>(serialized)
            .expect("sandbox context should deserialize"),
        context
    );

    let mut legacy_native = expected;
    legacy_native["permissions"]["file_system"]["entries"][0]["path"]["path"] =
        json!(native_path.to_string_lossy());
    assert_eq!(
        serde_json::from_value::<ExecServerFileSystemSandboxContext>(legacy_native)
            .expect("legacy native permission path should deserialize"),
        context
    );
}

#[test]
fn sandbox_context_accepts_legacy_read_write_roots_and_serializes_canonically() {
    let read = absolute_path("legacy-read");
    let write = absolute_path("legacy-write");
    let value = json!({
        "permissions": {
            "network": {"enabled": true},
            "file_system": {
                "read": [read.to_string_lossy()],
                "write": [write.to_string_lossy()]
            }
        },
        "windowsSandboxLevel": "disabled"
    });
    let context: ExecServerFileSystemSandboxContext =
        serde_json::from_value(value).expect("legacy permission profile should deserialize");
    let expected = ExecServerFileSystemSandboxContext {
        permissions: ExecServerPermissionProfile::Managed {
            file_system: ExecServerManagedFileSystemPermissions::Restricted {
                entries: vec![
                    ExecServerFileSystemSandboxEntry {
                        path: ExecServerFileSystemPath::Path {
                            path: PathUri::from_abs_path(&read),
                        },
                        access: ExecServerFileSystemAccessMode::Read,
                    },
                    ExecServerFileSystemSandboxEntry {
                        path: ExecServerFileSystemPath::Path {
                            path: PathUri::from_abs_path(&write),
                        },
                        access: ExecServerFileSystemAccessMode::Write,
                    },
                ],
                glob_scan_max_depth: None,
            },
            network: ExecServerNetworkSandboxPolicy::Enabled,
        },
        cwd: None,
        windows_sandbox_level: ExecServerWindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        use_legacy_landlock: false,
    };
    assert_eq!(context, expected);

    let serialized = serde_json::to_value(context).expect("sandbox context should serialize");
    assert_eq!(serialized["permissions"]["type"], "managed");
    assert_eq!(
        serialized["permissions"]["file_system"]["entries"][0]["path"]["path"],
        PathUri::from_abs_path(&read).to_string()
    );
    assert!(
        serialized["permissions"]["file_system"]
            .get("read")
            .is_none()
    );
    assert!(
        serialized["permissions"]["file_system"]
            .get("write")
            .is_none()
    );
}

#[test]
fn sandbox_context_preserves_legacy_aliases_canonically() {
    let value = json!({
        "permissions": {
            "type": "managed",
            "file_system": {
                "type": "restricted",
                "entries": [{
                    "path": {
                        "type": "special",
                        "value": {"kind": "current_working_directory"}
                    },
                    "access": "none"
                }]
            },
            "network": "restricted"
        },
        "windowsSandboxLevel": "disabled"
    });
    let context: ExecServerFileSystemSandboxContext =
        serde_json::from_value(value).expect("legacy aliases should deserialize");
    let serialized = serde_json::to_value(context).expect("sandbox context should serialize");

    assert_eq!(
        serialized["permissions"]["file_system"]["entries"][0],
        json!({
            "path": {
                "type": "special",
                "value": {"kind": "project_roots"}
            },
            "access": "deny"
        })
    );
}

#[test]
fn non_native_permission_path_is_rejected_when_converting_to_core() {
    let path = non_native_path_uri();
    let context = ExecServerFileSystemSandboxContext {
        permissions: ExecServerPermissionProfile::Managed {
            file_system: ExecServerManagedFileSystemPermissions::Restricted {
                entries: vec![ExecServerFileSystemSandboxEntry {
                    path: ExecServerFileSystemPath::Path { path: path.clone() },
                    access: ExecServerFileSystemAccessMode::Read,
                }],
                glob_scan_max_depth: None,
            },
            network: ExecServerNetworkSandboxPolicy::Restricted,
        },
        cwd: None,
        windows_sandbox_level: ExecServerWindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        use_legacy_landlock: false,
    };

    let error = FileSystemSandboxContext::try_from(context)
        .expect_err("foreign permission path should not convert on this host");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        format!(
            "sandbox permission path URI `{path}` is not valid on this exec-server host: {}",
            path.to_abs_path()
                .expect_err("test URI should be foreign to this host")
        )
    );
}

fn all_path_variants() -> Vec<FileSystemSandboxEntry> {
    vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: absolute_path("exact"),
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.secret".to_string(),
            },
            access: FileSystemAccessMode::Deny,
        },
        special_entry(FileSystemSpecialPath::Root, FileSystemAccessMode::Write),
        special_entry(FileSystemSpecialPath::Minimal, FileSystemAccessMode::Read),
        special_entry(
            FileSystemSpecialPath::ProjectRoots {
                subpath: Some("docs".into()),
            },
            FileSystemAccessMode::Write,
        ),
        special_entry(FileSystemSpecialPath::Tmpdir, FileSystemAccessMode::Read),
        special_entry(FileSystemSpecialPath::SlashTmp, FileSystemAccessMode::Write),
        special_entry(
            FileSystemSpecialPath::Unknown {
                path: ":future_root".to_string(),
                subpath: Some("nested".into()),
            },
            FileSystemAccessMode::Deny,
        ),
    ]
}

fn special_entry(
    value: FileSystemSpecialPath,
    access: FileSystemAccessMode,
) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry {
        path: FileSystemPath::Special { value },
        access,
    }
}

fn absolute_path(name: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(std::env::temp_dir().join(name))
        .expect("test path should be absolute")
}

fn path_uri(name: &str) -> PathUri {
    PathUri::from_abs_path(&absolute_path(name))
}

fn non_native_path_uri() -> PathUri {
    #[cfg(unix)]
    let value = "file://server/share/private";
    #[cfg(windows)]
    let value = "file:///usr/local/private";
    PathUri::parse(value).expect("non-native path URI should parse")
}
