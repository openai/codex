use std::borrow::Cow;
use std::env;
use std::path::{Path, PathBuf};

/// The directory containing this test binary.
pub fn exe_dir() -> PathBuf {
    let exe = env::current_exe().unwrap();
    exe.parent().unwrap().to_path_buf()
}

/// The directory to use for test probes.
pub fn out_dir() -> Cow<'static, Path> {
    if let Some(tmpdir) = option_env!("CARGO_TARGET_TMPDIR") {
        Cow::Borrowed(tmpdir.as_ref())
    } else if let Some(tmpdir) = env::var_os("TESTS_TARGET_DIR") {
        Cow::Owned(tmpdir.into())
    } else {
        // Use the same path as this test binary.
        Cow::Owned(exe_dir())
    }
}
