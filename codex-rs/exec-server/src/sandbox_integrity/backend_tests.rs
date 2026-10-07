//! Exercise backend policy preparation and dependency discovery.

use super::backend;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
#[cfg(target_os = "linux")]
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::sandbox::SandboxOverride;
use codex_sandboxing::FileContentsChecker;
use codex_sandboxing::SandboxExecRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::fs;
use std::io;

#[test]
fn snapshot_denials_replace_patterns_including_an_empty_expansion() -> io::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(temp.path())?.canonicalize()?;
    let target = root.join("helper");
    fs::write(&target, "fixture")?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(root.clone().into(), FileSystemAccessMode::Write),
        deny_glob(format!("{}/*", root.display())),
    ]);
    for (expanded, writable) in [(vec![], true), (vec![target.clone()], false)] {
        let prepared = policy.clone().with_expanded_deny_globs(expanded);
        let checker = FileContentsChecker::new(&prepared, &root)?;
        assert_eq!(checker.check(&target)?.is_some(), writable);
    }
    Ok(())
}

pub(super) fn deny_glob(pattern: impl Into<String>) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: pattern.into(),
        },
        FileSystemAccessMode::Deny,
    )
}

pub(super) fn sandbox_request(
    cwd: &AbsolutePathBuf,
    policy: &FileSystemSandboxPolicy,
) -> SandboxExecRequest {
    SandboxExecRequest {
        sandbox_override: SandboxOverride::NoOverride,
        command: vec![cwd.join("not-executed").to_string_lossy().into_owned()],
        cwd: PathUri::from_abs_path(cwd),
        sandbox_policy_cwd: PathUri::from_abs_path(cwd),
        env: Default::default(),
        network: None,
        network_environment_id: None,
        sandbox: backend::SANDBOX_TYPE,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        permission_profile: PermissionProfile::from_runtime_permissions(
            policy,
            NetworkSandboxPolicy::Restricted,
        ),
        arg0: None,
    }
}
#[cfg(target_os = "linux")]
#[test]
fn dependency_inventory_uses_launcher_location_and_command_path() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(temp.path())?.canonicalize()?;
    let cwd = root.join("command");
    let tools = root.join("tools");
    let bundled = tools.join("codex-resources/bwrap");
    fs::create_dir(&cwd)?;
    fs::create_dir(&tools)?;
    fs::create_dir(tools.join("codex-resources"))?;
    for path in [
        tools.join("bwrap"),
        tools.join("rg"),
        cwd.join("rg"),
        tools.join("codex"),
        bundled.clone(),
    ] {
        fs::write(&path, "fixture")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(/*mode*/ 0o755))?;
    }
    let launcher = cwd.join("sandbox-helper");
    std::os::unix::fs::symlink(tools.join("codex"), &launcher)?;
    let policy = FileSystemSandboxPolicy::restricted(vec![deny_glob("*.key")]);
    let mut request = sandbox_request(&cwd, &policy);
    request.command[0] = launcher.to_string_lossy().into_owned();
    for (path, scanner) in [
        ("../tools", tools.join("rg")),
        (":../tools", tools.join("rg")),
    ] {
        request.env.insert("PATH".to_owned(), path.to_owned());
        let dependencies = backend::critical_dependencies(&request, &policy, &cwd)?
            .into_iter()
            .filter(|(name, _)| {
                matches!(
                    *name,
                    "bundled_bwrap_candidate" | "system_bwrap_candidate" | "glob_scanner"
                )
            })
            .map(|(name, path)| path.and_then(fs::canonicalize).map(|path| (name, path)))
            .collect::<io::Result<Vec<_>>>()?;
        assert_eq!(
            dependencies,
            vec![
                ("bundled_bwrap_candidate", bundled.to_path_buf()),
                (
                    "system_bwrap_candidate",
                    tools.join("bwrap").into_path_buf()
                ),
                ("glob_scanner", scanner.into_path_buf()),
            ]
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn policy_preparation_uses_command_tmpdir_and_home() -> io::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(temp.path())?.canonicalize()?;
    let cwd = root.join("command");
    let home = root.join("home");
    fs::create_dir(&cwd)?;
    fs::create_dir(&home)?;
    let denied = home.join("secret.key");
    fs::write(&denied, "fixture")?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            FileSystemAccessMode::Write,
        ),
        deny_glob("~/secret.*"),
    ]);
    let mut request = sandbox_request(&cwd, &policy);
    request
        .env
        .insert("HOME".to_owned(), home.to_string_lossy().into_owned());
    for (tmpdir, expected) in [
        ("", None),
        ("scratch", Some(cwd.join("scratch"))),
        ("~/scratch", Some(home.join("scratch"))),
    ] {
        request.env.insert("TMPDIR".to_owned(), tmpdir.to_owned());
        let mut entries = expected
            .into_iter()
            .map(|path| FileSystemSandboxEntry::new(path.into(), FileSystemAccessMode::Write))
            .collect::<Vec<_>>();
        entries.push(FileSystemSandboxEntry::new(
            denied.clone().into(),
            FileSystemAccessMode::Deny,
        ));
        assert_eq!(
            backend::prepare_policy(&request, policy.clone(), &cwd)?,
            FileSystemSandboxPolicy::restricted(entries),
        );
    }
    Ok(())
}
