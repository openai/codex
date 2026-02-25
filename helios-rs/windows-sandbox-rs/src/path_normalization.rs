use std::path::Path;
use std::path::PathBuf;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn canonical_path_key(path: &Path) -> String {
    canonicalize_path(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::canonical_path_key;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn canonical_path_key_normalizes_case_and_separators() {
        let windows_style = Path::new(r"C:\Users\Dev\Repo");
        let slash_style = Path::new("c:/users/dev/repo");

        assert_eq!(canonical_path_key(windows_style), canonical_path_key(slash_style));
    }
}
