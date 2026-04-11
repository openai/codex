pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
pub(crate) use codex_utils_absolute_path::test_support::PathExt;
use std::path::Path;

pub(crate) fn test_path_display(path: &str) -> String {
    Path::new(path).abs().display().to_string()
}
