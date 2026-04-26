use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::path::Prefix;

pub fn canonicalize_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn normalize_spawn_cwd(path: &Path) -> PathBuf {
    let simplified = dunce::simplified(path).to_path_buf();
    if path_uses_unc_prefix(&simplified) {
        return simplified;
    }

    let canonical = dunce::canonicalize(path).ok();
    let canonical = canonical
        .as_deref()
        .map(dunce::simplified)
        .map(Path::to_path_buf);
    if let Some(canonical) = canonical
        && path_uses_unc_prefix(&canonical)
    {
        return canonical;
    }

    simplified
}

pub fn canonical_path_key(path: &Path) -> String {
    canonicalize_path(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub fn path_uses_unc_prefix(path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        matches!(
            path.components().next(),
            Some(std::path::Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_path_key;
    use super::normalize_spawn_cwd;
    use super::path_uses_unc_prefix;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn canonical_path_key_normalizes_case_and_separators() {
        let windows_style = Path::new(r"C:\Users\Dev\Repo");
        let slash_style = Path::new("c:/users/dev/repo");

        assert_eq!(
            canonical_path_key(windows_style),
            canonical_path_key(slash_style)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn path_uses_unc_prefix_matches_standard_and_verbatim_unc_paths() {
        assert!(path_uses_unc_prefix(Path::new(r"\\server\share\repo")));
        assert!(path_uses_unc_prefix(Path::new(
            r"\\?\UNC\server\share\repo"
        )));
        assert!(!path_uses_unc_prefix(Path::new(r"C:\repo")));
    }

    #[test]
    fn normalize_spawn_cwd_preserves_regular_local_paths() {
        let path = PathBuf::from(r"C:\repo");

        assert_eq!(normalize_spawn_cwd(&path), path);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn normalize_spawn_cwd_simplifies_verbatim_unc_paths() {
        let path = PathBuf::from(r"\\?\UNC\server\share\repo");

        assert_eq!(
            normalize_spawn_cwd(&path),
            PathBuf::from(r"\\server\share\repo")
        );
    }
}
