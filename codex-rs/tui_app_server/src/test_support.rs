#![expect(clippy::expect_used)]

use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;

pub(crate) trait PathExt {
    fn abs(&self) -> AbsolutePathBuf;
}

impl PathExt for Path {
    fn abs(&self) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(self.to_path_buf()).expect("path should already be absolute")
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
