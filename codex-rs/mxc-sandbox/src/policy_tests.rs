//! Portable policy and launcher regressions, with Windows identity coverage.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::protocol::FileSystemAccessMode;
use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxEntry;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_protocol::protocol::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use wxc_common::models::NetworkAction;
use wxc_common::models::NetworkCidr;
use wxc_common::models::NetworkEgressPolicy;
use wxc_common::models::NetworkIngressPolicy;
use wxc_common::models::NetworkPeer;
use wxc_common::models::NetworkRule;

use crate::CreateMxcCommandArgsParams;
use crate::MxcCommand;
use crate::create_command_args;
use crate::policy::PolicyError;
use crate::policy::build_request;
use crate::policy::materialize_volume_roots;

#[test]
fn symbolic_root_precedence_is_independent_of_drive_and_cwd() -> Result<()> {
    use FileSystemAccessMode::Deny;
    use FileSystemAccessMode::Read;
    use FileSystemAccessMode::Write;

    let volumes = [
        PathUri::parse("file:///C:/")?,
        PathUri::parse("file:///D:/")?,
    ];
    let readonly = PathUri::parse("file:///D:/foobar")?;
    let writable = PathUri::parse("file:///D:/writable")?;
    let mut fs = FileSystemSandboxPolicy::read_only();
    fs.entries.extend([
        FileSystemSandboxEntry::new(volumes[1].clone().into(), Read),
        FileSystemSandboxEntry::new(readonly.clone().into(), Read),
        FileSystemSandboxEntry::new(writable.clone().into(), Read),
        FileSystemSandboxEntry::new(writable.clone().into(), Write),
    ]);
    for access in [Read, Write, Deny] {
        fs.entries[0].access = access;
        let resolved = materialize_volume_roots(fs.clone(), &volumes)?;
        for volume in &volumes {
            let cwd = volume.join("work")?;
            let context = FileSystemSandboxPolicyContext {
                cwd: &cwd,
                workspace_roots: std::slice::from_ref(&cwd),
                user_home_dir: None,
                temporary_directories: None,
            };
            assert_eq!(
                [&volumes[0], &volumes[1], &readonly, &writable]
                    .map(|path| resolved.resolve_access(path, &context)),
                [access, access, Read, Write]
            );
        }
    }
    fs.entries[0].access = Write;
    fs.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Tmpdir,
        },
        Read,
    ));
    assert!(matches!(
        materialize_volume_roots(fs, &volumes),
        Err(PolicyError::UnsupportedSymbolicPath)
    ));
    Ok(())
}

fn canonical_root(temp: &tempfile::TempDir) -> Result<PathBuf> {
    Ok(
        PathUri::from_host_native_path(std::fs::canonicalize(temp.path())?)?
            .to_abs_path()?
            .into_path_buf(),
    )
}

fn entry(path: &Path, access: FileSystemAccessMode) -> Result<FileSystemSandboxEntry> {
    Ok(FileSystemSandboxEntry::new(
        AbsolutePathBuf::from_absolute_path(path)?.into(),
        access,
    ))
}

fn command(permissions: &PermissionProfile, cwd: &Path) -> MxcCommand {
    MxcCommand {
        permissions: permissions.clone(),
        sandbox_policy_cwd: cwd.to_owned(),
        managed_network: None,
        command: vec!["program.exe".to_owned(), "--arg".to_owned()],
    }
}

#[test]
fn wrapper_preserves_exact_argv_and_separate_command_cwd() -> Result<()> {
    let root = tempfile::tempdir()?;
    let cwd = root.path().join("command-cwd");
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(Vec::new()),
        NetworkSandboxPolicy::Restricted,
    );
    let argv = vec![
        r"C:\Program Files\tool.exe".to_owned(),
        "--permissions".to_owned(),
        String::new(),
        "quotes\" and slash\\".to_owned(),
    ];
    let mut env = HashMap::new();
    let wrapped = create_command_args(CreateMxcCommandArgsParams {
        command: argv.clone(),
        permission_profile: &profile,
        sandbox_policy_cwd: root.path(),
        managed_network: None,
        env: &mut env,
    })?;
    assert_eq!(wrapped, vec![crate::CODEX_WINDOWS_MXC_ARG1]);
    let parsed = crate::transport::decode(&mut env)?;
    assert_eq!(
        (
            &parsed.permissions,
            parsed.sandbox_policy_cwd.as_path(),
            &parsed.command
        ),
        (&profile, root.path(), &argv)
    );
    let request = build_request(&parsed, &cwd, vec!["CUSTOM=value".to_owned()], &[], &[])?;
    assert_eq!(
        (request.script_code, request.working_directory, request.env),
        (
            r#""C:\Program Files\tool.exe" --permissions "" "quotes\" and slash\\""#.to_owned(),
            cwd.to_str().unwrap().to_owned(),
            vec!["CUSTOM=value".to_owned()],
        )
    );
    Ok(())
}

#[test]
fn empty_command_is_rejected_before_native_launch() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut parsed = command(&PermissionProfile::read_only(), root.path());
    parsed.command.clear();
    let error = build_request(&parsed, root.path(), Vec::new(), &[], &[]).unwrap_err();
    assert_eq!(error.to_string(), "MXC command must not be empty");
    Ok(())
}

#[test]
fn native_grants_preserve_denies_and_read_only_carveouts() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let canonical = canonical_root(&temp)?;
    let root = canonical.as_path();
    let gitdir = tempfile::tempdir()?;
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}", gitdir.path().display()),
    )?;
    let readonly = root.join("readonly");
    let writable_child = readonly.join("writable");
    let denied = root.join("secret");
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(root, FileSystemAccessMode::Write)?,
        entry(&readonly, FileSystemAccessMode::Read)?,
        entry(&writable_child, FileSystemAccessMode::Write)?,
        entry(&denied, FileSystemAccessMode::Deny)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(&command(&profile, root), root, Vec::new(), &[], &[])?;
    let mut expected_read = [
        root.join(".agents"),
        root.join(".codex"),
        root.join(".git"),
        readonly,
        writable_child.join(".agents"),
        writable_child.join(".codex"),
        writable_child.join(".git"),
    ];
    expected_read.sort();
    assert_eq!(
        (
            request.policy.readwrite_paths,
            request.policy.readonly_paths,
            request.policy.denied_paths
        ),
        (
            vec![
                root.to_str().unwrap().to_owned(),
                writable_child.to_str().unwrap().to_owned()
            ],
            expected_read
                .iter()
                .map(|path| path.to_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            vec![denied.to_str().unwrap().to_owned()],
        )
    );
    assert!(!request.policy.fallback.allow_dacl_mutation);
    Ok(())
}

#[test]
fn volume_expansion_does_not_turn_read_only_child_writable() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let canonical = canonical_root(&temp)?;
    let root = canonical.as_path();
    let readonly = root.join("protected");
    let writable = root.join("work");
    std::fs::create_dir(&readonly)?;
    std::fs::create_dir(&writable)?;
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(root, FileSystemAccessMode::Write)?,
        entry(&readonly, FileSystemAccessMode::Read)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, root),
        root,
        Vec::new(),
        &[root.to_owned()],
        &[],
    )?;
    assert_eq!(
        request.policy.readwrite_paths,
        vec![root.to_str().unwrap(), writable.to_str().unwrap()]
    );
    assert_eq!(
        request.policy.readonly_paths,
        vec![
            root.join(".agents").to_str().unwrap(),
            root.join(".codex").to_str().unwrap(),
            root.join(".git").to_str().unwrap(),
            readonly.to_str().unwrap()
        ]
    );
    Ok(())
}

#[test]
fn volume_expansion_does_not_turn_read_only_alias_writable() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let readonly = root.join("protected");
    let alias = root.join("alias");
    std::fs::write(&readonly, "protected")?;
    std::fs::hard_link(&readonly, &alias)?;
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(&root, FileSystemAccessMode::Write)?,
        entry(&readonly, FileSystemAccessMode::Read)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, &root),
        &root,
        Vec::new(),
        std::slice::from_ref(&root),
        &[],
    )?;
    let alias = alias.display().to_string();
    assert_eq!(
        (
            request.policy.readwrite_paths.contains(&alias),
            request.policy.readonly_paths.contains(&alias),
        ),
        (false, true)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn volume_expansion_uses_normalized_root_access() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let volume = root.join("volume");
    let child = volume.join("child");
    let alias = root.join("alias");
    std::fs::create_dir_all(&child)?;
    std::os::unix::fs::symlink(&volume, &alias)?;
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(&volume, FileSystemAccessMode::Write)?,
        entry(&alias, FileSystemAccessMode::Read)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, &root),
        &root,
        Vec::new(),
        std::slice::from_ref(&volume),
        &[],
    )?;
    let child = child.display().to_string();
    assert_eq!(
        (
            request.policy.readwrite_paths.contains(&child),
            request.policy.readonly_paths.contains(&child),
        ),
        (false, true)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn volume_expansion_skips_uninspectable_children() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let uninspectable = root.join("loop");
    std::os::unix::fs::symlink(&uninspectable, &uninspectable)?;
    let denied = root.join("secret");
    std::fs::write(&denied, "secret")?;
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(&root, FileSystemAccessMode::Write)?,
        entry(&denied, FileSystemAccessMode::Deny)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, &root),
        &root,
        Vec::new(),
        std::slice::from_ref(&root),
        &[],
    )?;
    assert_eq!(
        (request.policy.readwrite_paths, request.policy.denied_paths),
        (
            vec![root.to_str().unwrap().to_owned()],
            vec![denied.to_str().unwrap().to_owned()],
        )
    );
    Ok(())
}

#[test]
fn full_access_enumerates_children_of_every_volume() -> Result<()> {
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    let unmapped = tempfile::tempdir()?;
    let explicit = canonical_root(&unmapped)?;
    let volumes = [canonical_root(&first)?, canonical_root(&second)?];
    std::fs::create_dir(volumes[0].join("one"))?;
    std::fs::create_dir(volumes[1].join("two"))?;
    let mut expected = [
        volumes[0].clone(),
        volumes[0].join("one"),
        volumes[1].clone(),
        volumes[1].join("two"),
        explicit.clone(),
    ];
    expected.sort();
    let mut fs = FileSystemSandboxPolicy::read_only();
    // Like an unmapped UNC share, this grant is outside the supplied volumes.
    fs.entries
        .push(entry(&explicit, FileSystemAccessMode::Read)?);
    for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Write] {
        fs.entries[0].access = access;
        fs.entries[1].access = access;
        let profile =
            PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
        let request = build_request(
            &command(&profile, &volumes[0]),
            &volumes[0],
            Vec::new(),
            &volumes,
            &[],
        )?;
        let native = request.policy;
        let (granted, other) = if access == FileSystemAccessMode::Write {
            (native.readwrite_paths, native.readonly_paths)
        } else {
            (native.readonly_paths, native.readwrite_paths)
        };
        assert_eq!(
            granted,
            expected
                .iter()
                .map(|path| path.to_str().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(other, Vec::<String>::new());
    }
    Ok(())
}

#[test]
fn managed_network_allows_authorized_loopback_without_lan_or_dns_access() -> Result<()> {
    let root = tempfile::tempdir()?;
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(Vec::new()),
        NetworkSandboxPolicy::Enabled,
    );
    let proxy = ManagedNetworkSandboxContext {
        loopback_ports: vec![43123, 43124],
        allow_local_binding: true,
    };
    let mut env = HashMap::new();
    create_command_args(CreateMxcCommandArgsParams {
        command: vec!["program.exe".to_owned()],
        permission_profile: &profile,
        sandbox_policy_cwd: root.path(),
        managed_network: Some(&proxy),
        env: &mut env,
    })?;
    let parsed = crate::transport::decode(&mut env)?;
    let request = build_request(&parsed, root.path(), Vec::new(), &[], &[])?;
    assert_eq!(
        request.policy.network_egress,
        Some(NetworkEgressPolicy {
            default: NetworkAction::Deny,
            allow: vec![NetworkRule {
                to: vec![
                    NetworkPeer {
                        cidr: "127.0.0.0/8".parse()?,
                        except: Vec::new()
                    },
                    NetworkPeer {
                        cidr: NetworkCidr {
                            address: std::net::Ipv6Addr::LOCALHOST.into(),
                            prefix_length: 128
                        },
                        except: Vec::new()
                    },
                ],
                ports: Vec::new(),
            }],
            deny: Vec::new(),
        })
    );
    assert_eq!(
        request.policy.network_ingress,
        Some(NetworkIngressPolicy {
            default: NetworkAction::Deny,
            host_loopback: NetworkAction::Allow,
        })
    );
    Ok(())
}

#[test]
fn invalid_managed_network_is_rejected_at_both_boundaries() -> Result<()> {
    let root = tempfile::tempdir()?;
    let cwd = root.path();
    let profile = PermissionProfile::read_only();
    for (loopback_ports, allow_local_binding) in
        [(Vec::new(), true), (vec![0], true), (vec![43123], false)]
    {
        let proxy = ManagedNetworkSandboxContext {
            loopback_ports,
            allow_local_binding,
        };
        let mut env = HashMap::new();
        assert!(
            create_command_args(CreateMxcCommandArgsParams {
                command: Vec::new(),
                permission_profile: &profile,
                sandbox_policy_cwd: cwd,
                managed_network: Some(&proxy),
                env: &mut env,
            })
            .is_err()
        );
        let mut parsed = command(&profile, cwd);
        parsed.managed_network = Some(proxy);
        assert!(build_request(&parsed, cwd, Vec::new(), &[], &[]).is_err());
    }
    Ok(())
}

#[test]
fn deny_globs_expand_files_and_directories_before_launch() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let nested = root.join("nested");
    std::fs::create_dir(&nested)?;
    let file = nested.join("file.secret");
    let directory = nested.join("directory.secret");
    std::fs::write(&file, "secret")?;
    std::fs::create_dir(&directory)?;
    std::fs::write(nested.join("allowed.txt"), "allowed")?;
    let missing = root.join("explicit.secret");
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(&root, FileSystemAccessMode::Write)?,
        entry(&missing, FileSystemAccessMode::Deny)?,
        FileSystemSandboxEntry::new(
            FileSystemPath::GlobPattern {
                pattern: "**/*.secret".to_owned(),
            },
            FileSystemAccessMode::Deny,
        ),
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(&command(&profile, &root), &nested, Vec::new(), &[], &[])?;
    let mut expected = [file, directory, missing];
    expected.sort();
    assert_eq!(
        request.policy.denied_paths,
        expected.map(|path| path.to_str().unwrap().to_owned())
    );
    Ok(())
}

#[test]
fn root_deny_keeps_only_narrow_explicit_grants() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let canonical = canonical_root(&temp)?;
    let root = canonical.as_path();
    let child = root.join("allowed");
    for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Write] {
        let fs = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Deny,
            ),
            entry(&child, access)?,
        ]);
        let profile =
            PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
        let request = build_request(
            &command(&profile, root),
            root,
            Vec::new(),
            &[root.to_owned()],
            &[],
        )?;
        let allowed = vec![child.to_str().unwrap().to_owned()];
        let expected = match access {
            FileSystemAccessMode::Read => (Vec::new(), allowed),
            FileSystemAccessMode::Write => (
                allowed,
                [".agents", ".codex", ".git"]
                    .map(|name| child.join(name).to_str().unwrap().to_owned())
                    .to_vec(),
            ),
            FileSystemAccessMode::Deny => unreachable!(),
        };
        assert_eq!(
            (
                request.policy.readwrite_paths,
                request.policy.readonly_paths
            ),
            expected
        );
        assert_eq!(request.policy.denied_paths, Vec::<String>::new());
    }
    Ok(())
}

#[test]
fn windows_temp_roots_use_filtered_environment_and_keep_explicit_denies() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let canonical = canonical_root(&temp)?;
    let root = canonical.as_path();
    let first = root.join("temp").join("~");
    let second = root.join("tmp with spaces");
    let denied = first.join("secret");
    let fs = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::SlashTmp,
            },
            FileSystemAccessMode::Write,
        ),
        entry(&denied, FileSystemAccessMode::Deny)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, root),
        root,
        vec![
            format!("temp={}", first.display()),
            format!("Tmp={}", second.display()),
        ],
        &[],
        &[],
    )?;
    assert_eq!(
        request.policy.readwrite_paths,
        vec![first.to_str().unwrap(), second.to_str().unwrap()]
    );
    assert_eq!(request.policy.denied_paths, vec![denied.to_str().unwrap()]);

    for value in [
        None,
        Some(""),
        Some("relative"),
        Some("~/scratch"),
        Some("~\\scratch"),
        Some("C:temp"),
        Some("\\temp"),
    ] {
        let request = build_request(
            &command(&profile, root),
            root,
            value
                .map(|value| vec![format!("TEMP={value}"), format!("TMP={value}")])
                .unwrap_or_default(),
            &[],
            &[],
        )?;
        assert_eq!(request.policy.readwrite_paths, Vec::<String>::new());
        assert_eq!(request.policy.denied_paths, vec![denied.to_str().unwrap()]);
    }
    Ok(())
}

#[test]
fn minimal_platform_roots_preserve_explicit_denies() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let allowed = root.join("platform");
    let denied = root.join("secret");
    for include_defaults in [false, true] {
        let mut entries = vec![entry(&denied, FileSystemAccessMode::Deny)?];
        if include_defaults {
            entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Minimal,
                },
                FileSystemAccessMode::Read,
            ));
        }
        let profile = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(entries),
            NetworkSandboxPolicy::Restricted,
        );
        let request = build_request(
            &command(&profile, &root),
            &root,
            Vec::new(),
            &[],
            &[allowed.clone(), denied.clone()],
        )?;
        let expected_read = if include_defaults {
            vec![allowed.to_str().unwrap().to_owned()]
        } else {
            Vec::new()
        };
        assert_eq!(
            (request.policy.readonly_paths, request.policy.denied_paths),
            (expected_read, vec![denied.to_str().unwrap().to_owned()])
        );
    }
    Ok(())
}

#[test]
fn symbolic_root_preserves_equal_path_precedence_and_denies() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let base = canonical_root(&temp)?;
    let volumes = [
        base.join("first"),
        base.join("readonly"),
        base.join("denied"),
    ];
    for volume in &volumes {
        std::fs::create_dir_all(volume.join("child"))?;
    }
    let fs = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Write,
        ),
        entry(&volumes[1], FileSystemAccessMode::Read)?,
        entry(&volumes[2], FileSystemAccessMode::Deny)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, &volumes[0]),
        &volumes[0],
        Vec::new(),
        &volumes,
        &[],
    )?;
    assert_eq!(
        request.policy.readwrite_paths,
        vec![
            volumes[0].to_str().unwrap().to_owned(),
            volumes[0].join("child").to_str().unwrap().to_owned(),
            volumes[1].to_str().unwrap().to_owned(),
            volumes[1].join("child").to_str().unwrap().to_owned(),
        ]
    );
    let reads_in_volumes: Vec<_> = request
        .policy
        .readonly_paths
        .into_iter()
        .filter(|path| Path::new(path).starts_with(&base))
        .collect();
    assert_eq!(
        reads_in_volumes,
        volumes[..2]
            .iter()
            .flat_map(|volume| [".agents", ".codex", ".git"].map(|name| volume
                .join(name)
                .to_str()
                .unwrap()
                .to_owned()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        request.policy.denied_paths,
        vec![volumes[2].to_str().unwrap()]
    );
    Ok(())
}

#[test]
fn narrowing_one_volume_keeps_the_other_volume_root_grant() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let base = canonical_root(&temp)?;
    let volumes = [base.join("first"), base.join("second")];
    for volume in &volumes {
        std::fs::create_dir_all(volume.join("child"))?;
    }
    for (root_access, narrowed) in [
        (FileSystemAccessMode::Read, FileSystemAccessMode::Deny),
        (FileSystemAccessMode::Write, FileSystemAccessMode::Read),
    ] {
        let fs = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                root_access,
            ),
            entry(&volumes[0].join("child"), narrowed)?,
        ]);
        let profile =
            PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
        let request = build_request(
            &command(&profile, &volumes[0]),
            &volumes[0],
            Vec::new(),
            &volumes,
            &[],
        )?;
        let granted = if root_access == FileSystemAccessMode::Write {
            request.policy.readwrite_paths
        } else {
            request.policy.readonly_paths
        };
        let second_grants: Vec<_> = granted
            .into_iter()
            .filter(|path| Path::new(path).starts_with(&volumes[1]))
            .collect();
        assert_eq!(
            second_grants,
            vec![
                volumes[1].to_str().unwrap().to_owned(),
                volumes[1].join("child").to_str().unwrap().to_owned(),
            ]
        );
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn volume_enumeration_uses_windows_identity_for_read_write_overrides() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let child = root.join("MiXeD");
    std::fs::create_dir(&child)?;
    let alias = child.to_str().unwrap().to_ascii_lowercase();
    let fs = FileSystemSandboxPolicy::restricted(vec![
        entry(&root, FileSystemAccessMode::Read)?,
        entry(Path::new(&alias), FileSystemAccessMode::Write)?,
    ]);
    let profile =
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
    let request = build_request(
        &command(&profile, &root),
        &root,
        Vec::new(),
        std::slice::from_ref(&root),
        &[],
    )?;
    assert_eq!(
        request.policy.readwrite_paths,
        vec![
            PathUri::from_host_native_path(&alias)?
                .to_abs_path()?
                .as_path()
                .to_str()
                .unwrap()
                .to_owned()
        ]
    );
    let readonly_child: Vec<_> = request
        .policy
        .readonly_paths
        .into_iter()
        .filter(|path| path.eq_ignore_ascii_case(child.to_str().unwrap()))
        .collect();
    assert_eq!(readonly_child, Vec::<String>::new());
    Ok(())
}

#[test]
fn relative_working_directories_fail_before_launch() -> Result<()> {
    let root = tempfile::tempdir()?;
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(Vec::new()),
        NetworkSandboxPolicy::Restricted,
    );
    for (policy_cwd, command_cwd, kind) in [
        (Path::new("relative"), root.path(), "policy"),
        (root.path(), Path::new("relative"), "command"),
    ] {
        let error = build_request(
            &command(&profile, policy_cwd),
            command_cwd,
            Vec::new(),
            &[],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("MXC requires an absolute {kind} working directory")
        );
    }
    Ok(())
}

#[test]
fn non_unicode_paths_fail_before_launch() -> Result<()> {
    #[cfg(unix)]
    let name = <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(vec![0xff]);
    #[cfg(windows)]
    let name = <std::ffi::OsString as std::os::windows::ffi::OsStringExt>::from_wide(&[0xd800]);
    let temp = tempfile::tempdir()?;
    let root = canonical_root(&temp)?;
    let invalid = root.join(name);
    for (policy_path, command_cwd, expected) in [
        (
            root.as_path(),
            invalid.as_path(),
            "MXC requires a Unicode command working directory",
        ),
        (
            invalid.as_path(),
            root.as_path(),
            "MXC requires Unicode filesystem policy paths",
        ),
    ] {
        let fs = FileSystemSandboxPolicy::restricted(vec![entry(
            policy_path,
            FileSystemAccessMode::Deny,
        )?]);
        let profile =
            PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted);
        let error = build_request(&command(&profile, &root), command_cwd, Vec::new(), &[], &[])
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
    }
    Ok(())
}
