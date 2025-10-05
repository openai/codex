use dirs::home_dir;
use std::fs;
use std::path::PathBuf;

/// This was copied from codex-core but codex-core depends on this crate.
/// TODO: move this to a shared crate lower in the dependency tree.
///
///
/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// `~/.codex`.
///
/// - If `CODEX_HOME` is set, the value will be canonicalized and this
///   function will Err if the path does not exist.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
pub(crate) fn find_codex_home() -> std::io::Result<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    if let Ok(val) = std::env::var("CODEX_HOME")
        && !val.is_empty()
    {
        let path = PathBuf::from(val);
        fs::create_dir_all(&path)?;
        return path.canonicalize();
    }

    let mut p = home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;
    p.push(".codex");
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use std::ffi::OsString;
    use std::path::Path;
    use tempfile::TempDir;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set_path(key: &'static str, value: &Path) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    #[test]
    fn find_codex_home_creates_missing_env_directory() {
        let tmp = TempDir::new().expect("tempdir");
        let custom_home = tmp.path().join("rmcp").join("codex-home");
        assert!(
            !custom_home.exists(),
            "custom codex home should not exist yet"
        );

        let _guard = EnvVarGuard::set_path("CODEX_HOME", &custom_home);
        let resolved = find_codex_home().expect("resolve codex home when env var is set");

        assert!(
            resolved.exists(),
            "resolved codex home should exist on disk"
        );
        let expected = custom_home
            .canonicalize()
            .expect("custom path should be canonicalizable");
        assert_eq!(resolved, expected);
    }
}
