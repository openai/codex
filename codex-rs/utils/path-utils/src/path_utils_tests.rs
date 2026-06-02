#[cfg(unix)]
mod symlinks {
    use super::super::open_file_for_read_no_follow;
    use super::super::resolve_symlink_write_paths;
    use pretty_assertions::assert_eq;
    use std::io::Read;
    use std::os::unix::fs::symlink;

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

    #[test]
    fn no_follow_read_rejects_symlinked_parent_directory() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let dir = dir.path().canonicalize()?;
        let target_dir = dir.join("target");
        let symlinked_dir = dir.join("link");
        std::fs::create_dir(&target_dir)?;
        std::fs::write(target_dir.join("payload"), "secret")?;
        symlink(&target_dir, &symlinked_dir)?;

        let err = open_file_for_read_no_follow(&symlinked_dir.join("payload"))
            .expect_err("symlinked parent should fail");

        assert!(err.raw_os_error().is_some());
        Ok(())
    }

    #[test]
    fn no_follow_read_reads_regular_file() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().canonicalize()?.join("payload");

        std::fs::write(&path, "hello")?;
        let mut reader = open_file_for_read_no_follow(&path)?;
        let mut contents = String::new();
        reader.read_to_string(&mut contents)?;

        assert_eq!(contents, "hello");
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
