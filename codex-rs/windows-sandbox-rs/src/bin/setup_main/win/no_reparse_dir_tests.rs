use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use super::open_or_create_no_reparse;

fn create_directory_junction(target: &Path, alias: &Path) -> Result<()> {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(alias)
        .arg(target)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mklink /J failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[test]
fn creates_and_opens_plain_directory_leaf() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join(".sandbox-bin");

    drop(open_or_create_no_reparse(&directory)?);
    let _handle = open_or_create_no_reparse(&directory)?;

    assert!(directory.is_dir());
    Ok(())
}

#[test]
fn rejects_final_directory_junction() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target = temporary.path().join("target");
    let alias = temporary.path().join(".sandbox-bin");
    fs::create_dir(&target)?;
    create_directory_junction(&target, &alias)?;

    let _ =
        open_or_create_no_reparse(&alias).expect_err("final directory junction must be rejected");
    fs::remove_dir(&alias)?;
    Ok(())
}

#[test]
fn rejects_ancestor_directory_junction() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target_home = temporary.path().join("target-home");
    let alias_home = temporary.path().join("linked-home");
    fs::create_dir(&target_home)?;
    fs::create_dir(target_home.join(".sandbox-bin"))?;
    create_directory_junction(&target_home, &alias_home)?;

    let _ = open_or_create_no_reparse(&alias_home.join(".sandbox-bin"))
        .expect_err("ancestor directory junction must be rejected");
    fs::remove_dir(&alias_home)?;
    Ok(())
}
