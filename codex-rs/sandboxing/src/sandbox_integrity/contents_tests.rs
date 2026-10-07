//! Policy grants, denials, and current file identities used by the contents checker.

use super::*;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemAccessMode::Deny;
use codex_protocol::permissions::FileSystemAccessMode::Read;
use codex_protocol::permissions::FileSystemAccessMode::Write;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use pretty_assertions::assert_eq;

fn policy(entries: &[(&AbsolutePathBuf, FileSystemAccessMode)]) -> FileSystemSandboxPolicy {
    FileSystemSandboxPolicy::restricted(
        entries
            .iter()
            .map(|(path, access)| FileSystemSandboxEntry::new((*path).clone().into(), *access))
            .collect(),
    )
}

#[test]
fn contents_respect_write_grants_and_carveouts() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(directory.path())?.canonicalize()?;
    let target = root.join("sandbox-helper");
    fs::write(&target, "fixture")?;
    for (entries, write_roots) in [
        (vec![(&root, Write)], Some(vec![root.clone()])),
        (vec![(&root, Write), (&target, Read)], None),
        (
            vec![(&root, Read), (&target, Write)],
            Some(vec![target.clone()]),
        ),
        (vec![(&root, Write), (&target, Deny)], None),
        (
            vec![(&root, Write), (&target, Write)],
            Some(vec![root.clone(), target.clone()]),
        ),
    ] {
        let checker = FileContentsChecker::new(&policy(&entries), &root)?;
        assert_eq!(
            checker.check(&target)?,
            write_roots.map(|write_roots| IntegrityFinding {
                target: target.clone(),
                write_roots,
                details: IntegrityFindingDetails::Contents {
                    resolved_target: target.clone(),
                },
            })
        );
    }
    Ok(())
}

#[test]
fn deny_read_globs_protect_matching_contents_without_disabling_other_checks() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(directory.path())?.canonicalize()?;
    let target = root.join("sandbox-helper");
    let unrelated = root.join("other-helper");
    fs::write(&target, "fixture")?;
    fs::write(&unrelated, "fixture")?;
    let mut policy = policy(&[(&root, Write)]);
    policy.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: format!("{}/sandbox-*", root.display()),
        },
        Deny,
    ));
    let checker = FileContentsChecker::new(&policy, &root)?;
    assert_eq!(checker.check(&target)?, None);
    assert_eq!(
        checker.check(&unrelated)?,
        Some(IntegrityFinding {
            target: unrelated.clone(),
            write_roots: vec![root],
            details: IntegrityFindingDetails::Contents {
                resolved_target: unrelated,
            },
        })
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn contents_follow_links_and_resolved_write_grants() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(directory.path())?.canonicalize()?;
    let writable = root.join("workspace");
    let destination = root.join("sandbox-helper");
    let link = writable.join("helper-link");
    fs::create_dir(&writable)?;
    fs::write(&destination, "fixture")?;
    std::os::unix::fs::symlink(&destination, &link)?;
    let mut alias_denied = policy(&[(&root, Write)]);
    alias_denied.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: format!("{}/helper-*", writable.display()),
        },
        Deny,
    ));
    for (policy, expected) in [
        (policy(&[(&writable, Write)]), None),
        (policy(&[(&link, Write)]), Some(destination.clone())),
        (policy(&[(&root, Write), (&link, Read)]), None),
        (alias_denied, Some(root.clone())),
    ] {
        let checker = FileContentsChecker::new(&policy, &root)?;
        assert_eq!(
            checker.check(&link)?,
            expected.map(|write_root| IntegrityFinding {
                target: link.clone(),
                write_roots: vec![write_root],
                details: IntegrityFindingDetails::Contents {
                    resolved_target: destination.clone(),
                },
            })
        );
    }
    Ok(())
}

#[test]
fn protected_metadata_contents_remain_read_only() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(directory.path())?.canonicalize()?;
    let target = root.join(".codex/config.toml");
    fs::create_dir(root.join(".codex"))?;
    fs::write(&target, "fixture")?;
    let checker = FileContentsChecker::new(&policy(&[(&root, Write)]), &root)?;
    assert_eq!(checker.check(&target)?, None);
    Ok(())
}

#[test]
fn missing_target_has_no_contents_to_mutate() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = AbsolutePathBuf::from_absolute_path(directory.path())?.canonicalize()?;
    let checker = FileContentsChecker::new(&policy(&[(&root, Write)]), &root)?;
    assert_eq!(checker.check(&root.join("missing"))?, None);
    Ok(())
}
