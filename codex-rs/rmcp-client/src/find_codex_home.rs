use dirs::home_dir;
use std::path::PathBuf;

/// This was copied from codex-core but codex-core depends on this crate.
/// TODO: move this to a shared crate lower in the dependency tree.
///
///
/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// `~/.codex`.
///
/// - If `CODEX_HOME` is set, the value must exist and be a directory. The
///   value will be canonicalized and this function will Err otherwise.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
pub(crate) fn find_codex_home() -> std::io::Result<PathBuf> {
    let codex_home_env = std::env::var("CODEX_HOME")
        .ok()
        .filter(|val| !val.is_empty());
    find_codex_home_from_env(codex_home_env.as_deref())
}

fn find_codex_home_from_env(codex_home_env: Option<&str>) -> std::io::Result<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    if let Some(val) = codex_home_env {
        let path = PathBuf::from(val);
        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("CODEX_HOME points to {val:?}, but that path does not exist"),
            ));
        }
        if !path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("CODEX_HOME points to {val:?}, but that path is not a directory"),
            ));
        }

        return path.canonicalize().map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!("failed to canonicalize CODEX_HOME {val:?}: {err}"),
            )
        });
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
    use super::find_codex_home_from_env;
    use dirs::home_dir;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[test]
    fn find_codex_home_env_missing_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let missing = temp_home.path().join("missing-codex-home");
        let missing_str = missing
            .to_str()
            .expect("missing codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(missing_str)).expect_err("missing CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(
            err.to_string().contains("CODEX_HOME"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_file_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let file_path = temp_home.path().join("codex-home.txt");
        fs::write(&file_path, "not a directory").expect("write temp file");
        let file_str = file_path
            .to_str()
            .expect("file codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(file_str)).expect_err("file CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_valid_directory_canonicalizes() {
        let temp_home = TempDir::new().expect("temp home");
        let temp_str = temp_home
            .path()
            .to_str()
            .expect("temp codex home path should be valid utf-8");

        let resolved = find_codex_home_from_env(Some(temp_str)).expect("valid CODEX_HOME");
        let expected = temp_home
            .path()
            .canonicalize()
            .expect("canonicalize temp home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_default_home_dir() {
        let resolved = find_codex_home_from_env(None).expect("default CODEX_HOME");
        let mut expected = home_dir().expect("home dir");
        expected.push(".codex");
        assert_eq!(resolved, expected);
    }
}
