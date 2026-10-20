use super::replace_path_and_deduplicate;
use super::symlink_metadata_if_exists;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn symlink_metadata_if_exists_distinguishes_files_directories_and_missing_paths()
-> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("file");
    std::fs::write(&file, "contents")?;

    let directory_metadata =
        symlink_metadata_if_exists(directory.path())?.expect("directory exists");
    let file_metadata = symlink_metadata_if_exists(&file)?.expect("file exists");
    assert_eq!(
        (directory_metadata.is_dir(), file_metadata.is_file()),
        (true, true)
    );
    assert!(symlink_metadata_if_exists(&directory.path().join("missing"))?.is_none());
    Ok(())
}

#[test]
fn symlink_metadata_if_exists_preserves_io_errors() {
    let error = symlink_metadata_if_exists(Path::new("invalid\0path"))
        .expect_err("an invalid path must not be treated as missing");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn replace_path_and_deduplicate_preserves_other_paths_and_order() {
    let old_path = Path::new("old");
    let new_path = Path::new("new");
    let extra = Path::new("extra");
    let descendant = Path::new("old/child");

    assert_eq!(
        replace_path_and_deduplicate(
            vec![old_path, extra, descendant, new_path, extra],
            old_path,
            new_path,
        ),
        vec![new_path, extra, descendant]
    );
}

#[cfg(unix)]
mod symlinks {
    use super::super::resolve_symlink_write_paths;
    use super::super::symlink_metadata_if_exists;
    use pretty_assertions::assert_eq;
    use std::os::unix::fs::symlink;

    #[test]
    fn symlink_metadata_if_exists_preserves_links_with_and_without_targets() -> std::io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("target");
        let link = directory.path().join("link");
        std::fs::write(&target, "contents")?;
        symlink(&target, &link)?;

        let linked_metadata = symlink_metadata_if_exists(&link)?.expect("symlink exists");
        std::fs::remove_file(target)?;
        let dangling_metadata = symlink_metadata_if_exists(&link)?.expect("symlink still exists");
        assert!(linked_metadata.is_symlink());
        assert_eq!(dangling_metadata.file_type(), linked_metadata.file_type());
        Ok(())
    }

    #[test]
    fn symlink_cycles_fall_back_to_root_write_path() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let a = dir.path().join("a");
        let b = dir.path().join("b");

        symlink(&b, &a)?;
        symlink(&a, &b)?;

        let resolved = resolve_symlink_write_paths(&a)?;

        assert_eq!(resolved.read_path, None);
        assert_eq!(resolved.write_path, a);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod wsl {
    use super::super::normalize_for_wsl_with_flag;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn wsl_mnt_drive_paths_lowercase() {
        let normalized =
            normalize_for_wsl_with_flag(PathBuf::from("/mnt/C/Users/Dev"), /*is_wsl*/ true);

        assert_eq!(normalized, PathBuf::from("/mnt/c/users/dev"));
    }

    #[test]
    fn wsl_non_drive_paths_unchanged() {
        let path = PathBuf::from("/mnt/cc/Users/Dev");
        let normalized = normalize_for_wsl_with_flag(path.clone(), /*is_wsl*/ true);

        assert_eq!(normalized, path);
    }

    #[test]
    fn wsl_non_mnt_paths_unchanged() {
        let path = PathBuf::from("/home/Dev");
        let normalized = normalize_for_wsl_with_flag(path.clone(), /*is_wsl*/ true);

        assert_eq!(normalized, path);
    }
}

mod native_workdir {
    use super::super::normalize_for_native_workdir_with_flag;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_verbatim_paths_are_simplified() {
        let path = PathBuf::from(r"\\?\D:\c\x\worktrees\2508\swift-base");
        let normalized = normalize_for_native_workdir_with_flag(path, /*is_windows*/ true);

        assert_eq!(
            normalized,
            PathBuf::from(r"D:\c\x\worktrees\2508\swift-base")
        );
    }

    #[test]
    fn non_windows_paths_are_unchanged() {
        let path = PathBuf::from(r"\\?\D:\c\x\worktrees\2508\swift-base");
        let normalized =
            normalize_for_native_workdir_with_flag(path.clone(), /*is_windows*/ false);

        assert_eq!(normalized, path);
    }
}

mod path_comparison {
    use super::super::paths_match_after_normalization;
    use std::path::PathBuf;

    #[test]
    fn matches_identical_existing_paths() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;

        assert!(paths_match_after_normalization(dir.path(), dir.path()));
        Ok(())
    }

    #[test]
    fn falls_back_to_raw_equality_when_paths_cannot_be_normalized() {
        assert!(paths_match_after_normalization(
            PathBuf::from("missing"),
            PathBuf::from("missing"),
        ));
        assert!(!paths_match_after_normalization(
            PathBuf::from("missing-a"),
            PathBuf::from("missing-b"),
        ));
    }

    #[cfg(windows)]
    #[test]
    fn matches_windows_verbatim_paths() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let verbatim_dir = PathBuf::from(format!(r"\\?\{}", dir.path().display()));

        assert!(paths_match_after_normalization(verbatim_dir, dir.path()));
        Ok(())
    }
}
