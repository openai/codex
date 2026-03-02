use std::path::Path;
use std::path::PathBuf;

use codex_config::FilesystemDenyReadPattern;
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

pub(crate) fn expand_deny_read_patterns(
    patterns: &[FilesystemDenyReadPattern],
) -> Vec<AbsolutePathBuf> {
    let mut expanded = Vec::new();
    for pattern in patterns {
        if pattern.contains_glob() {
            expand_glob_pattern(pattern, &mut expanded);
        } else if let Ok(path) = AbsolutePathBuf::try_from(pattern.as_str()) {
            push_unique_absolute(&mut expanded, path);
        }
    }
    expanded
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

fn push_unique_absolute(candidates: &mut Vec<AbsolutePathBuf>, candidate: AbsolutePathBuf) {
    if !candidates
        .iter()
        .any(|existing| existing.as_path() == candidate.as_path())
    {
        candidates.push(candidate);
    }
}

fn expand_glob_pattern(pattern: &FilesystemDenyReadPattern, expanded: &mut Vec<AbsolutePathBuf>) {
    let (root, components) = split_glob_pattern(pattern.as_str());
    let Ok(root) = AbsolutePathBuf::try_from(root) else {
        return;
    };
    expand_glob_components(root.as_path(), &components, expanded);
}

fn split_glob_pattern(pattern: &str) -> (&str, Vec<&str>) {
    let Some(first_glob) = pattern.find('*') else {
        return (pattern, Vec::new());
    };
    let separator_index = pattern[..first_glob]
        .char_indices()
        .rev()
        .find(|(_, ch)| is_path_separator(*ch))
        .map(|(index, _)| index);
    let (root, suffix) = match separator_index {
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
    };
    let components = suffix
        .split(is_path_separator)
        .filter(|component| !component.is_empty())
        .collect();
    (root, components)
}

fn expand_glob_components(current: &Path, remaining: &[&str], expanded: &mut Vec<AbsolutePathBuf>) {
    if remaining.is_empty() {
        if let Ok(path) = AbsolutePathBuf::try_from(current) {
            push_unique_absolute(expanded, path);
        }
        return;
    }

    let component = remaining[0];
    let rest = &remaining[1..];

    if component == "**" {
        expand_glob_components(current, rest, expanded);

        let Ok(entries) = std::fs::read_dir(current) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if rest.is_empty()
                && let Ok(path) = AbsolutePathBuf::try_from(path.as_path())
            {
                push_unique_absolute(expanded, path);
            }
            if path.is_dir() {
                expand_glob_components(&path, remaining, expanded);
            }
        }
        return;
    }

    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !segment_matches(component, name) {
            continue;
        }
        if rest.is_empty() {
            if let Ok(path) = AbsolutePathBuf::try_from(path.as_path()) {
                push_unique_absolute(expanded, path);
            }
        } else if path.is_dir() {
            expand_glob_components(&path, rest, expanded);
        }
    }
}

fn segment_matches(pattern: &str, candidate: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == candidate;
    }

    let pattern_chars: Vec<char> = pattern.chars().collect();
    let candidate_chars: Vec<char> = candidate.chars().collect();
    let mut pattern_index = 0;
    let mut candidate_index = 0;
    let mut star_index = None;
    let mut star_candidate_index = 0;

    while candidate_index < candidate_chars.len() {
        if pattern_index < pattern_chars.len()
            && pattern_chars[pattern_index] == candidate_chars[candidate_index]
        {
            pattern_index += 1;
            candidate_index += 1;
            continue;
        }

        if pattern_index < pattern_chars.len() && pattern_chars[pattern_index] == '*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_candidate_index = candidate_index;
            continue;
        }

        if let Some(star_index) = star_index {
            pattern_index = star_index + 1;
            star_candidate_index += 1;
            candidate_index = star_candidate_index;
            continue;
        }

        return false;
    }

    while pattern_index < pattern_chars.len() && pattern_chars[pattern_index] == '*' {
        pattern_index += 1;
    }

    pattern_index == pattern_chars.len()
}

fn is_path_separator(ch: char) -> bool {
    if cfg!(windows) {
        ch == '/' || ch == '\\'
    } else {
        ch == '/'
    }
}

#[cfg(test)]
mod tests {
    use super::expand_deny_read_patterns;
    use super::is_read_denied;
    use super::overlaps_deny_read;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use codex_config::FilesystemDenyReadPattern;
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

    #[test]
    fn expand_deny_read_patterns_supports_star_and_globstar() {
        let temp = tempdir().expect("temp dir");
        let secrets = temp.path().join("secrets");
        let nested = secrets.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested");
        let top = secrets.join("top.txt");
        let deep = nested.join("deep.txt");
        let ignored = secrets.join("top.log");
        std::fs::write(&top, "top").expect("write top");
        std::fs::write(&deep, "deep").expect("write deep");
        std::fs::write(&ignored, "ignored").expect("write ignored");

        let top_pattern = FilesystemDenyReadPattern::from_input(&format!(
            "{}/secrets/*.txt",
            temp.path().display()
        ))
        .expect("normalize pattern");
        let deep_pattern = FilesystemDenyReadPattern::from_input(&format!(
            "{}/secrets/**/*.txt",
            temp.path().display()
        ))
        .expect("normalize pattern");

        let expanded = expand_deny_read_patterns(&[top_pattern, deep_pattern]);

        assert_eq!(
            expanded,
            vec![
                AbsolutePathBuf::try_from(top).expect("absolute path"),
                AbsolutePathBuf::try_from(deep).expect("absolute path"),
            ]
        );
    }
}
