pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
pub(crate) use codex_utils_absolute_path::test_support::test_path_buf;

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).display().to_string()
}
