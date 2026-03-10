use std::path::Path;
use std::path::PathBuf;

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use globset::GlobBuilder;

use crate::function_tool::FunctionCallError;

const DENY_READ_POLICY_MESSAGE: &str =
    "access denied: reading this path is blocked by filesystem deny_read policy";

pub(crate) fn ensure_read_allowed(
    path: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> Result<(), FunctionCallError> {
    if is_read_denied(path, file_system_sandbox_policy, cwd) {
        return Err(FunctionCallError::RespondToModel(format!(
            "{DENY_READ_POLICY_MESSAGE}: `{}`",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn ensure_search_root_does_not_overlap_deny_read(
    search_root: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> Result<(), FunctionCallError> {
    if overlaps_deny_read(search_root, file_system_sandbox_policy, cwd) {
        return Err(FunctionCallError::RespondToModel(format!(
            "access denied: grep_files path `{}` overlaps a filesystem deny_read path; narrow the search path",
            search_root.display()
        )));
    }
    Ok(())
}

pub(crate) fn is_read_denied(
    path: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> bool {
    let denied_paths = file_system_sandbox_policy.get_unreadable_roots_with_cwd(cwd);
    let path_candidates = normalized_and_canonical_candidates(path);
    if denied_paths.iter().any(|denied| {
        let denied_candidates = normalized_and_canonical_candidates(denied.as_path());
        path_candidates.iter().any(|candidate| {
            denied_candidates.iter().any(|denied_candidate| {
                candidate == denied_candidate || candidate.starts_with(denied_candidate)
            })
        })
    }) {
        return true;
    }

    if !cfg!(target_os = "macos") {
        return false;
    }

    file_system_sandbox_policy
        .deny_read_patterns()
        .iter()
        .any(|pattern| glob_pattern_matches_any_candidate(&path_candidates, pattern))
}

pub(crate) fn overlaps_deny_read(
    path: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> bool {
    let denied_paths = file_system_sandbox_policy.get_unreadable_roots_with_cwd(cwd);
    let path_candidates = normalized_and_canonical_candidates(path);
    if denied_paths.iter().any(|denied| {
        let denied_candidates = normalized_and_canonical_candidates(denied.as_path());
        path_candidates.iter().any(|candidate| {
            denied_candidates.iter().any(|denied_candidate| {
                candidate.starts_with(denied_candidate) || denied_candidate.starts_with(candidate)
            })
        })
    }) {
        return true;
    }

    if !cfg!(target_os = "macos") {
        return false;
    }

    file_system_sandbox_policy
        .deny_read_patterns()
        .iter()
        .filter_map(|pattern| glob_pattern_prefix(pattern))
        .any(|prefix| {
            path_candidates.iter().any(|candidate| {
                candidate.starts_with(prefix.as_path()) || prefix.as_path().starts_with(candidate)
            })
        })
}

fn normalized_and_canonical_candidates(path: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(normalized) = AbsolutePathBuf::from_absolute_path(path) {
        push_unique(&mut candidates, normalized.to_path_buf());
    } else {
        push_unique(&mut candidates, path.to_path_buf());
    }

    if let Ok(canonical) = path.canonicalize()
        && let Ok(canonical_absolute) = AbsolutePathBuf::from_absolute_path(canonical)
    {
        push_unique(&mut candidates, canonical_absolute.to_path_buf());
    }

    candidates
}

fn push_unique(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn glob_pattern_matches_any_candidate(path_candidates: &[PathBuf], pattern: &str) -> bool {
    let Ok(glob) = GlobBuilder::new(pattern).literal_separator(true).build() else {
        return false;
    };
    let matcher = glob.compile_matcher();
    path_candidates
        .iter()
        .any(|candidate| matcher.is_match(candidate))
}

fn glob_pattern_prefix(pattern: &str) -> Option<AbsolutePathBuf> {
    let (root, _) = split_glob_pattern(pattern);
    AbsolutePathBuf::try_from(root).ok()
}

fn split_glob_pattern(pattern: &str) -> (&str, &str) {
    let Some(first_glob) = pattern.find(is_glob_metacharacter) else {
        return (pattern, "");
    };
    let separator_index = pattern[..first_glob]
        .char_indices()
        .rev()
        .find(|(_, ch)| is_path_separator(*ch))
        .map(|(index, _)| index);
    match separator_index {
        Some(0) => ("/", &pattern[1..]),
        Some(index)
            if cfg!(windows)
                && index == 2
                && pattern.as_bytes().get(1) == Some(&b':')
                && pattern.as_bytes().get(2).is_some() =>
        {
            (&pattern[..=index], &pattern[index + 1..])
        }
        Some(index) => (&pattern[..index], &pattern[index + 1..]),
        None => ("", pattern),
    }
}

fn is_path_separator(ch: char) -> bool {
    if cfg!(windows) {
        ch == '/' || ch == '\\'
    } else {
        ch == '/'
    }
}

fn is_glob_metacharacter(ch: char) -> bool {
    matches!(ch, '*' | '?' | '[')
}

#[cfg(test)]
mod tests {
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::is_read_denied;
    use super::overlaps_deny_read;
    use super::*;

    fn deny_policy(path: &std::path::Path) -> FileSystemSandboxPolicy {
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(path).expect("absolute deny path"),
            },
            access: FileSystemAccessMode::None,
        }])
    }

    #[test]
    fn exact_path_and_descendants_are_denied() {
        let temp = tempdir().expect("temp dir");
        let denied_dir = temp.path().join("denied");
        let nested = denied_dir.join("nested.txt");
        std::fs::create_dir_all(&denied_dir).expect("create denied dir");
        std::fs::write(&nested, "secret").expect("write secret");

        let policy = deny_policy(&denied_dir);
        assert_eq!(is_read_denied(&denied_dir, &policy, temp.path()), true);
        assert_eq!(is_read_denied(&nested, &policy, temp.path()), true);
        assert_eq!(
            is_read_denied(&temp.path().join("other.txt"), &policy, temp.path()),
            false
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonical_target_matches_denied_symlink_alias() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("temp dir");
        let real_dir = temp.path().join("real");
        let alias_dir = temp.path().join("alias");
        std::fs::create_dir_all(&real_dir).expect("create real dir");
        symlink(&real_dir, &alias_dir).expect("symlink alias");

        let secret = real_dir.join("secret.txt");
        std::fs::write(&secret, "secret").expect("write secret");
        let alias_secret = alias_dir.join("secret.txt");

        let policy = deny_policy(&real_dir);
        assert_eq!(is_read_denied(&alias_secret, &policy, temp.path()), true);
    }

    #[test]
    fn overlap_detects_parent_and_child_relationships() {
        let temp = tempdir().expect("temp dir");
        let denied = temp.path().join("private");
        let search_root = temp.path();
        let nested_search_root = denied.join("nested");
        std::fs::create_dir_all(&nested_search_root).expect("create nested");

        let policy = deny_policy(&denied);
        assert_eq!(overlaps_deny_read(search_root, &policy, temp.path()), true);
        assert_eq!(
            overlaps_deny_read(&nested_search_root, &policy, temp.path()),
            true
        );
        assert_eq!(
            overlaps_deny_read(&temp.path().join("public"), &policy, temp.path()),
            false
        );
    }

    #[test]
    fn literal_patterns_are_denied_and_globs_are_ignored_off_macos() {
        let temp = tempdir().expect("temp dir");
        let literal = temp.path().join("private");
        let other = temp.path().join("notes.txt");
        std::fs::create_dir_all(&literal).expect("create literal dir");
        std::fs::write(&other, "notes").expect("write notes");

        let mut policy = deny_policy(&literal);
        policy.deny_read_patterns = vec![format!("{}/**/*.txt", temp.path().display())];

        assert_eq!(is_read_denied(&literal, &policy, temp.path()), true);
        assert_eq!(
            is_read_denied(&other, &policy, temp.path()),
            cfg!(target_os = "macos")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn glob_patterns_deny_matching_paths_on_macos() {
        let temp = tempdir().expect("temp dir");
        let denied = temp.path().join("private").join("secret1.txt");
        std::fs::create_dir_all(denied.parent().expect("parent")).expect("create parent");
        std::fs::write(&denied, "secret").expect("write secret");

        let policy = FileSystemSandboxPolicy {
            deny_read_patterns: vec![format!("{}/private/secret?.txt", temp.path().display())],
            ..Default::default()
        };

        assert_eq!(is_read_denied(&denied, &policy, temp.path()), true);
        assert_eq!(
            overlaps_deny_read(&temp.path().join("private"), &policy, temp.path()),
            true
        );
    }
}
