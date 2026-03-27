use std::path::Path;
use std::path::PathBuf;

use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::function_tool::FunctionCallError;

const DENY_READ_POLICY_MESSAGE: &str =
    "access denied: reading this path is blocked by filesystem deny_read policy";

pub(crate) struct ReadDenyMatcher {
    denied_candidates: Vec<Vec<PathBuf>>,
}

impl ReadDenyMatcher {
    pub(crate) fn new(
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        cwd: &Path,
    ) -> Option<Self> {
        if !file_system_sandbox_policy.has_denied_read_restrictions() {
            return None;
        }

        let denied_candidates = file_system_sandbox_policy
            .get_unreadable_roots_with_cwd(cwd)
            .into_iter()
            .map(|path| normalized_and_canonical_candidates(path.as_path()))
            .collect();
        Some(Self { denied_candidates })
    }

    pub(crate) fn is_read_denied(&self, path: &Path) -> bool {
        let path_candidates = normalized_and_canonical_candidates(path);
        self.denied_candidates.iter().any(|denied_candidates| {
            path_candidates.iter().any(|candidate| {
                denied_candidates.iter().any(|denied_candidate| {
                    candidate == denied_candidate || candidate.starts_with(denied_candidate)
                })
            })
        })
    }
}

pub(crate) fn ensure_read_allowed(
    path: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> Result<(), FunctionCallError> {
    if ReadDenyMatcher::new(file_system_sandbox_policy, cwd)
        .is_some_and(|matcher| matcher.is_read_denied(path))
    {
        return Err(FunctionCallError::RespondToModel(format!(
            "{DENY_READ_POLICY_MESSAGE}: `{}`",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn is_read_denied(
    path: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> bool {
    ReadDenyMatcher::new(file_system_sandbox_policy, cwd)
        .is_some_and(|matcher| matcher.is_read_denied(path))
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
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::is_read_denied;
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
}
