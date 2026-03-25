use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;

pub(crate) trait PathExt {
    fn abs(&self) -> AbsolutePathBuf;
}

impl PathExt for Path {
    fn abs(&self) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(self.to_path_buf())
            .unwrap_or_else(|_| panic!("path should already be absolute"))
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
    Path::new(path).abs().display().to_string()
}
