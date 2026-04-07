use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;

pub(crate) trait PathExt {
    fn abs(&self) -> AbsolutePathBuf;
}

impl PathExt for Path {
    fn abs(&self) -> AbsolutePathBuf {
        if let Ok(path) = AbsolutePathBuf::try_from(self.to_path_buf()) {
            return path;
        }
        if cfg!(windows)
            && let Some(path) = self.to_str()
            && path.starts_with('/')
        {
            return AbsolutePathBuf::try_from(test_path_buf(path))
                .expect("windows test path should be absolute");
        }
        panic!("path should already be absolute");
    }
}

pub(crate) trait PathBufExt {
    fn abs(&self) -> AbsolutePathBuf;
}

impl PathBufExt for PathBuf {
    fn abs(&self) -> AbsolutePathBuf {
        self.as_path().abs()
    }
}

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
