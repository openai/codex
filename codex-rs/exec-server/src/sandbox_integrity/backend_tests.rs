//! Exercise backend policy preparation and dependency discovery.

use super::backend;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
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
