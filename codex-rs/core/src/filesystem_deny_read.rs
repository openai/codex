use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::function_tool::FunctionCallError;
use crate::protocol::SandboxPolicy;

const DENY_READ_POLICY_MESSAGE: &str =
    "access denied: reading this path is blocked by filesystem deny_read policy";

pub(crate) fn ensure_read_allowed(
    path: &Path,
    sandbox_policy: &SandboxPolicy,
) -> Result<(), FunctionCallError> {
    if is_read_denied(path, sandbox_policy) {
        return Err(FunctionCallError::RespondToModel(format!(
            "{DENY_READ_POLICY_MESSAGE}: `{}`",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn ensure_search_root_does_not_overlap_deny_read(
    search_root: &Path,
    sandbox_policy: &SandboxPolicy,
) -> Result<(), FunctionCallError> {
    if overlaps_deny_read(search_root, sandbox_policy) {
        return Err(FunctionCallError::RespondToModel(format!(
            "access denied: grep_files path `{}` overlaps a filesystem deny_read path; narrow the search path",
            search_root.display()
        )));
    }
    Ok(())
}

pub(crate) fn is_read_denied(path: &Path, sandbox_policy: &SandboxPolicy) -> bool {
    let denied_paths = sandbox_policy.denied_read_paths();
    if denied_paths.is_empty() {
        return false;
    }

    let path_candidates = normalized_and_canonical_candidates(path);
    denied_paths.iter().any(|denied| {
        let denied_candidates = normalized_and_canonical_candidates(denied.as_path());
        path_candidates.iter().any(|candidate| {
            denied_candidates.iter().any(|denied_candidate| {
                candidate == denied_candidate || candidate.starts_with(denied_candidate)
            })
        })
    })
}

pub(crate) fn overlaps_deny_read(path: &Path, sandbox_policy: &SandboxPolicy) -> bool {
    let denied_paths = sandbox_policy.denied_read_paths();
    if denied_paths.is_empty() {
        return false;
    }

    let path_candidates = normalized_and_canonical_candidates(path);
    denied_paths.iter().any(|denied| {
        let denied_candidates = normalized_and_canonical_candidates(denied.as_path());
        path_candidates.iter().any(|candidate| {
            denied_candidates.iter().any(|denied_candidate| {
                candidate.starts_with(denied_candidate) || denied_candidate.starts_with(candidate)
            })
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

#[cfg(test)]
mod tests {
    use super::is_read_denied;
    use super::overlaps_deny_read;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn deny_policy(path: &std::path::Path) -> SandboxPolicy {
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::FullAccess,
            deny_read_paths: vec![AbsolutePathBuf::try_from(path).expect("absolute deny path")],
        }
    }

    #[test]
    fn exact_path_and_descendants_are_denied() {
        let temp = tempdir().expect("temp dir");
        let denied_dir = temp.path().join("denied");
        let nested = denied_dir.join("nested.txt");
        std::fs::create_dir_all(&denied_dir).expect("create denied dir");
        std::fs::write(&nested, "secret").expect("write secret");

        let policy = deny_policy(&denied_dir);
        assert_eq!(is_read_denied(&denied_dir, &policy), true);
        assert_eq!(is_read_denied(&nested, &policy), true);
        assert_eq!(
            is_read_denied(&temp.path().join("other.txt"), &policy),
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
        assert_eq!(is_read_denied(&alias_secret, &policy), true);
    }

    #[test]
    fn overlap_detects_parent_and_child_relationships() {
        let temp = tempdir().expect("temp dir");
        let denied = temp.path().join("private");
        let search_root = temp.path();
        let nested_search_root = denied.join("nested");
        std::fs::create_dir_all(&nested_search_root).expect("create nested");

        let policy = deny_policy(&denied);
        assert_eq!(overlaps_deny_read(search_root, &policy), true);
        assert_eq!(overlaps_deny_read(&nested_search_root, &policy), true);
        assert_eq!(
            overlaps_deny_read(&temp.path().join("public"), &policy),
            false
        );
    }
}
