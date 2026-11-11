#![cfg(test)]
//! Covers registration of an existing Windows home through its directory junction.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn only_the_saved_junction_needs_migration() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let target = tempfile::tempdir_in(dir.path())?;
    let alias = dir.path().join("alias");
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/j"])
        .arg(&alias)
        .arg(target.path())
        .status()?;
    anyhow::ensure!(status.success(), "create test junction");
    let physical = target.path().canonicalize()?;
    let resolve = |saved: &Path| {
        resolve_restored_home(saved, /*runtime*/ None, &mut Vec::new())
    };
    assert_eq!(resolve(&alias)?, Some(physical.clone()));
    assert_eq!(resolve(&physical)?, None);
    Ok(())
}
