use super::*;
use pretty_assertions::assert_eq;

#[test]
fn normalize_absolute_path_for_platform_simplifies_windows_verbatim_paths() {
    let parsed =
        normalize_absolute_path_for_platform(r"\\?\D:\c\x\worktrees\2508\swift-base", true);
    assert_eq!(parsed, PathBuf::from(r"D:\c\x\worktrees\2508\swift-base"));
}
