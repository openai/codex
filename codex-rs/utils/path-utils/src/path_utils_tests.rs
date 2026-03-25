#[cfg(unix)]
mod symlinks {
    use super::super::resolve_symlink_write_paths;
    use pretty_assertions::assert_eq;
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
}

#[cfg(target_os = "linux")]
mod wsl {
    use super::super::normalize_for_wsl_with_flag;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn wsl_mnt_drive_paths_lowercase() {
        let normalized = normalize_for_wsl_with_flag(PathBuf::from("/mnt/C/Users/Dev"), true);

        assert_eq!(normalized, PathBuf::from("/mnt/c/users/dev"));
    }

    #[test]
    fn wsl_non_drive_paths_unchanged() {
        let path = PathBuf::from("/mnt/cc/Users/Dev");
        let normalized = normalize_for_wsl_with_flag(path.clone(), true);

        assert_eq!(normalized, path);
    }

    #[test]
    fn wsl_non_mnt_paths_unchanged() {
        let path = PathBuf::from("/home/Dev");
        let normalized = normalize_for_wsl_with_flag(path.clone(), true);

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
        let normalized = normalize_for_native_workdir_with_flag(path, true);

        assert_eq!(
            normalized,
            PathBuf::from(r"D:\c\x\worktrees\2508\swift-base")
        );
    }

    #[test]
    fn non_windows_paths_are_unchanged() {
        let path = PathBuf::from(r"\\?\D:\c\x\worktrees\2508\swift-base");
        let normalized = normalize_for_native_workdir_with_flag(path.clone(), false);

        assert_eq!(normalized, path);
    }
}
