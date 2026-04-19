use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

struct GlobScanPlan {
    root: PathBuf,
    max_depth: Option<usize>,
}

/// Resolve split filesystem `None` read entries into concrete Windows ACL targets.
///
/// Windows ACLs do not understand Codex filesystem glob patterns directly. Exact
/// unreadable roots can be passed through as-is, including paths that do not
/// exist yet. Glob entries are snapshot-expanded to the files/directories that
/// already exist under their literal scan root; future exact paths are handled
/// later by materializing them before the deny ACE is applied.
pub fn resolve_windows_deny_read_paths(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> Result<Vec<AbsolutePathBuf>, String> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    for path in file_system_sandbox_policy.get_unreadable_roots_with_cwd(cwd.as_path()) {
        push_absolute_path(&mut paths, &mut seen, path.into_path_buf())?;
    }

    let unreadable_globs = file_system_sandbox_policy.get_unreadable_globs_with_cwd(cwd.as_path());
    if unreadable_globs.is_empty() {
        return Ok(paths);
    }

    let glob_policy = FileSystemSandboxPolicy::restricted(
        unreadable_globs
            .iter()
            .map(|pattern| FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: pattern.clone(),
                },
                access: FileSystemAccessMode::None,
            })
            .collect(),
    );
    let Some(matcher) = ReadDenyMatcher::new(&glob_policy, cwd.as_path()) else {
        return Ok(paths);
    };

    let mut seen_scan_dirs = HashSet::new();
    for pattern in unreadable_globs {
        let scan_plan = glob_scan_plan(&pattern);
        collect_existing_glob_matches(
            &scan_plan.root,
            &matcher,
            &mut paths,
            &mut seen,
            &mut seen_scan_dirs,
            scan_plan.max_depth,
            0,
        )?;
    }

    Ok(paths)
}

fn collect_existing_glob_matches(
    path: &Path,
    matcher: &ReadDenyMatcher,
    paths: &mut Vec<AbsolutePathBuf>,
    seen_paths: &mut HashSet<PathBuf>,
    seen_scan_dirs: &mut HashSet<PathBuf>,
    max_depth: Option<usize>,
    depth: usize,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    if matcher.is_read_denied(path) {
        push_absolute_path(paths, seen_paths, path.to_path_buf())?;
    }

    let Ok(metadata) = path.metadata() else {
        return Ok(());
    };
    if !metadata.is_dir() {
        return Ok(());
    }

    // Canonical directory keys keep recursive scans from following a symlink or
    // junction cycle forever while preserving the original matched path for the
    // ACL layer.
    let scan_key = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen_scan_dirs.insert(scan_key) {
        return Ok(());
    }

    if max_depth.is_some_and(|max_depth| depth >= max_depth) {
        return Ok(());
    }

    let Ok(entries) = std::fs::read_dir(path) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        collect_existing_glob_matches(
            &entry.path(),
            matcher,
            paths,
            seen_paths,
            seen_scan_dirs,
            max_depth,
            depth + 1,
        )?;
    }

    Ok(())
}

fn push_absolute_path(
    paths: &mut Vec<AbsolutePathBuf>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
) -> Result<(), String> {
    let absolute_path = AbsolutePathBuf::from_absolute_path(dunce::simplified(&path))
        .map_err(|err| err.to_string())?;
    if seen.insert(absolute_path.to_path_buf()) {
        paths.push(absolute_path);
    }
    Ok(())
}

fn glob_scan_plan(pattern: &str) -> GlobScanPlan {
    // Start scanning at the deepest literal directory prefix before the first
    // glob metacharacter. For example, `C:\repo\**\*.env` only scans `C:\repo`
    // instead of the current directory or drive root.
    let first_glob = pattern
        .char_indices()
        .find(|(_, ch)| matches!(ch, '*' | '?' | '['))
        .map(|(index, _)| index)
        .unwrap_or(pattern.len());
    let literal_prefix = &pattern[..first_glob];
    let Some(separator_index) = literal_prefix.rfind(['/', '\\']) else {
        return GlobScanPlan {
            root: PathBuf::from("."),
            max_depth: glob_scan_max_depth(pattern),
        };
    };
    let pattern_suffix = &pattern[separator_index + 1..];
    let is_drive_root_separator = separator_index > 0
        && literal_prefix
            .as_bytes()
            .get(separator_index - 1)
            .is_some_and(|ch| *ch == b':');
    if separator_index == 0 || is_drive_root_separator {
        return GlobScanPlan {
            root: PathBuf::from(&literal_prefix[..=separator_index]),
            max_depth: glob_scan_max_depth(pattern_suffix),
        };
    }
    GlobScanPlan {
        root: PathBuf::from(literal_prefix[..separator_index].to_string()),
        max_depth: glob_scan_max_depth(pattern_suffix),
    }
}

fn glob_scan_max_depth(pattern_suffix: &str) -> Option<usize> {
    let components = pattern_suffix
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    if components.contains(&"**") {
        return None;
    }
    Some(components.len())
}

#[cfg(test)]
mod tests {
    use super::glob_scan_plan;
    use super::resolve_windows_deny_read_paths;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn unreadable_glob_entry(pattern: String) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern { pattern },
            access: FileSystemAccessMode::None,
        }
    }

    fn unreadable_path_entry(path: PathBuf) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::from_absolute_path(path).expect("absolute path"),
            },
            access: FileSystemAccessMode::None,
        }
    }

    #[test]
    fn scan_root_uses_literal_prefix_before_glob() {
        assert_eq!(
            glob_scan_plan("/tmp/work/**/*.env").root,
            PathBuf::from("/tmp/work")
        );
        assert_eq!(
            glob_scan_plan(r"C:\Users\dev\repo\**\*.env").root,
            PathBuf::from(r"C:\Users\dev\repo")
        );
        assert_eq!(glob_scan_plan(r"C:\*.env").root, PathBuf::from(r"C:\"));
    }

    #[test]
    fn scan_depth_is_bounded_for_non_recursive_globs() {
        assert_eq!(glob_scan_plan("/tmp/work/*.env").max_depth, Some(1));
        assert_eq!(glob_scan_plan("/tmp/work/*/*.env").max_depth, Some(2));
        assert_eq!(glob_scan_plan("/tmp/work/**/*.env").max_depth, None);
    }

    #[test]
    fn exact_missing_paths_are_preserved() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let missing = tmp.path().join("missing.env");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_path_entry(missing)]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("resolve"),
            vec![
                AbsolutePathBuf::from_absolute_path(
                    dunce::canonicalize(tmp.path())
                        .expect("canonical tempdir")
                        .join("missing.env")
                )
                .expect("absolute missing")
            ]
        );
    }

    #[test]
    fn glob_patterns_expand_to_existing_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let root_env = tmp.path().join(".env");
        let nested_env = tmp.path().join("app").join(".env");
        let notes = tmp.path().join("app").join("notes.txt");
        std::fs::create_dir_all(notes.parent().expect("parent")).expect("create parent");
        std::fs::write(&root_env, "secret").expect("write root env");
        std::fs::write(&nested_env, "secret").expect("write nested env");
        std::fs::write(&notes, "notes").expect("write notes");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(format!(
            "{}/**/*.env",
            tmp.path().display()
        ))]);

        let actual: HashSet<PathBuf> = resolve_windows_deny_read_paths(&policy, &cwd)
            .expect("resolve")
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf)
            .collect();
        let expected = [root_env, nested_env].into_iter().collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn non_recursive_globs_do_not_expand_nested_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let root_env = tmp.path().join(".env");
        let nested_env = tmp.path().join("app").join(".env");
        std::fs::create_dir_all(nested_env.parent().expect("parent")).expect("create parent");
        std::fs::write(&root_env, "secret").expect("write root env");
        std::fs::write(&nested_env, "secret").expect("write nested env");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(format!(
            "{}/*.env",
            tmp.path().display()
        ))]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("resolve"),
            vec![AbsolutePathBuf::from_absolute_path(root_env).expect("absolute root env")]
        );
    }
}
