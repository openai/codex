use anyhow::Context;
use codex_utils_cargo_bin::find_resource;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_apply_patch_scenarios() -> anyhow::Result<()> {
    let scenarios_dir = find_resource!("tests/fixtures/scenarios")
        .map_err(|err| anyhow::anyhow!(err).context("resolve scenarios fixtures dir"))?;
    for scenario in fs::read_dir(&scenarios_dir)
        .with_context(|| format!("read scenarios dir: {}", scenarios_dir.display()))?
    {
        let scenario = scenario?;
        let path = scenario.path();
        if path.is_dir() {
            run_apply_patch_scenario(&path)?;
        }
    }
    Ok(())
}

/// Reads a scenario directory, copies the input files to a temporary directory, runs apply-patch,
/// and asserts that the final state matches the expected state exactly.
fn run_apply_patch_scenario(dir: &Path) -> anyhow::Result<()> {
    let tmp = tempdir().context("create temp dir for scenario")?;

    // Copy the input files to the temporary directory
    let input_dir = dir.join("input");
    if input_dir.is_dir() {
        copy_dir_recursive(&input_dir, tmp.path())
            .with_context(|| format!("copy input dir: {}", input_dir.display()))?;
    }

    // Read the patch.txt file
    let patch_path = dir.join("patch.txt");
    let patch = fs::read_to_string(&patch_path)
        .with_context(|| format!("read patch: {}", patch_path.display()))?;

    // Run apply_patch in the temporary directory. We intentionally do not assert
    // on the exit status here; the scenarios are specified purely in terms of
    // final filesystem state, which we compare below.
    Command::new(super::apply_patch_bin()?)
        .arg(patch)
        .current_dir(tmp.path())
        .output()
        .context("run apply_patch scenario")?;

    // Assert that the final state matches the expected state exactly
    let expected_dir = dir.join("expected");
    let expected_snapshot = snapshot_dir(&expected_dir)
        .with_context(|| format!("snapshot expected dir: {}", expected_dir.display()))?;
    let actual_snapshot = snapshot_dir(tmp.path())
        .with_context(|| format!("snapshot actual dir: {}", tmp.path().display()))?;

    assert_eq!(
        actual_snapshot,
        expected_snapshot,
        "Scenario {} did not match expected final state",
        dir.display()
    );

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File(Vec<u8>),
    Dir,
}

fn snapshot_dir(root: &Path) -> anyhow::Result<BTreeMap<PathBuf, Entry>> {
    let mut entries = BTreeMap::new();
    if root.is_dir() {
        snapshot_dir_recursive(root, root, &mut entries)
            .with_context(|| format!("snapshot dir recursively: {}", root.display()))?;
    }
    Ok(entries)
}

fn snapshot_dir_recursive(
    base: &Path,
    dir: &Path,
    entries: &mut BTreeMap<PathBuf, Entry>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir: {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read_dir entry: {}", dir.display()))?;
        let path = entry.path();
        let Some(stripped) = path.strip_prefix(base).ok() else {
            continue;
        };
        let rel = stripped.to_path_buf();

        // Under Buck2, files in `__srcs` are often materialized as symlinks.
        // Use `metadata()` (follows symlinks) so our fixture snapshots work
        // under both Cargo and Buck2.
        let metadata = fs::metadata(&path)
            .with_context(|| format!("metadata for snapshot entry: {}", path.display()))?;
        if metadata.is_dir() {
            entries.insert(rel.clone(), Entry::Dir);
            snapshot_dir_recursive(base, &path, entries)
                .with_context(|| format!("snapshot dir recursively: {}", path.display()))?;
        } else if metadata.is_file() {
            let contents =
                fs::read(&path).with_context(|| format!("read file: {}", path.display()))?;
            entries.insert(rel, Entry::File(contents));
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in fs::read_dir(src).with_context(|| format!("read_dir: {}", src.display()))? {
        let entry = entry.with_context(|| format!("read_dir entry: {}", src.display()))?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        // See note in `snapshot_dir_recursive` about Buck2 symlink trees.
        let metadata = fs::metadata(&path)
            .with_context(|| format!("metadata for copy entry: {}", path.display()))?;
        if metadata.is_dir() {
            fs::create_dir_all(&dest_path)
                .with_context(|| format!("create dir: {}", dest_path.display()))?;
            copy_dir_recursive(&path, &dest_path)
                .with_context(|| format!("copy dir recursively: {}", path.display()))?;
        } else if metadata.is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create dir: {}", parent.display()))?;
            }
            fs::copy(&path, &dest_path).with_context(|| {
                format!("copy file {} -> {}", path.display(), dest_path.display())
            })?;
        }
    }
    Ok(())
}
