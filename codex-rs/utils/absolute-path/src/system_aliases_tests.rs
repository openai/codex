use crate::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[cfg(target_os = "macos")]
#[test]
fn system_aliases_preserve_mutable_symlinks_and_missing_descendants() -> std::io::Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let outside = tempfile::tempdir()?;
    let logical = AbsolutePathBuf::from_absolute_path(directory.path())?;
    let physical = logical.canonicalize()?;
    std::os::unix::fs::symlink(outside.path(), logical.join("mutable"))?;

    for suffix in ["", "missing/child", "mutable", "mutable/missing/child"] {
        assert_eq!(
            logical.join(suffix).normalize_system_aliases()?,
            physical.join(suffix),
        );
    }
    Ok(())
}

#[test]
fn ordinary_paths_keep_missing_descendants() -> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    let physical = AbsolutePathBuf::from_absolute_path(directory.path())?.canonicalize()?;
    let missing = physical.join("missing/child");
    assert_eq!(missing.normalize_system_aliases()?, missing);
    Ok(())
}
