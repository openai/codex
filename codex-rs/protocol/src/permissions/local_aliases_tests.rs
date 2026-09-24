use super::*;
use crate::permissions::FileSystemAccessMode;
use pretty_assertions::assert_eq;

#[test]
fn local_aliases_keep_read_only_and_deny_overrides() -> anyhow::Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let logical = PathUri::from_host_native_path(directory.path())?;
    let physical = PathUri::from_host_native_path(directory.path().canonicalize()?)?;
    let context = FileSystemSandboxPolicyContext {
        cwd: &physical,
        workspace_roots: std::slice::from_ref(&physical),
        user_home_dir: None,
        temporary_directories: None,
    };
    for writable in [&logical, &physical] {
        for restricted in [&logical, &physical] {
            for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Deny] {
                let policy = FileSystemSandboxPolicy::restricted(vec![
                    FileSystemSandboxEntry::new(
                        writable.clone().into(),
                        FileSystemAccessMode::Write,
                    ),
                    FileSystemSandboxEntry::new(restricted.join("protected")?.into(), access),
                ]);
                let matching = policy.prepare_local_matching(&context)?;
                for spelling in [&logical, &physical] {
                    let protected = spelling.join("protected/missing/file")?;
                    assert_eq!(
                        (
                            matching.can_write_path(&spelling.join("missing/file")?)?,
                            matching.can_write_path(&protected)?,
                            policy.resolve_access_for_local_path_with_cwd(
                                protected.to_abs_path()?.as_path(),
                                directory.path(),
                            ),
                            matching.can_write_path(&spelling.join(".codex/config.toml")?)?,
                        ),
                        (true, false, access, false),
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn equivalent_alias_entries_keep_restrictive_precedence() -> anyhow::Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let logical = PathUri::from_host_native_path(directory.path())?;
    let physical = PathUri::from_host_native_path(directory.path().canonicalize()?)?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(physical.clone().into(), FileSystemAccessMode::Write),
        FileSystemSandboxEntry::new(logical.into(), FileSystemAccessMode::Deny),
    ]);
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(
            physical.join("missing")?.to_abs_path()?.as_path(),
            directory.path(),
        ),
        FileSystemAccessMode::Deny,
    );
    Ok(())
}
