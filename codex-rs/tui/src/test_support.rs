pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
use std::path::PathBuf;

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).abs().display().to_string()
}

pub(crate) fn test_path_buf(path: &str) -> PathBuf {
    if cfg!(windows) {
        let mut platform_path = PathBuf::from(r"C:\");
        platform_path.extend(
            path.trim_start_matches('/')
                .split('/')
                .filter(|segment| !segment.is_empty()),
        );
        platform_path
    } else {
        PathBuf::from(path)
    }
}
