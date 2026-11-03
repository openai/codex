use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::permissions::ResolvedFileSystemPermissions;
use codex_protocol::permissions::windows_deny_read_glob_scan;
use codex_utils_absolute_path::AbsolutePathBuf;
use ignore::overrides::OverrideBuilder;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
struct GlobScanPlan {
    root: PathBuf,
    max_depth: Option<usize>,
    globs: Vec<String>,
}

/// Resolve split filesystem `None` read entries into concrete Windows ACL targets.
///
/// Windows ACLs do not understand Codex filesystem glob patterns directly. Exact
/// unreadable roots can be passed through as-is, including paths that do not
/// exist yet. Glob entries are snapshot-expanded to the files/directories that
/// already exist under their literal scan root. Directory links below that root
/// are not traversed; explicitly linked scan roots are followed. Future exact
/// paths are materialized later before the deny ACE is applied.
pub fn resolve_windows_deny_read_paths(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> Result<Vec<AbsolutePathBuf>, String> {
    expand_deny_read_paths(
        &file_system_sandbox_policy.get_unreadable_roots_with_cwd(cwd),
        &file_system_sandbox_policy.get_unreadable_globs_with_cwd(cwd),
        file_system_sandbox_policy.glob_scan_max_depth,
        cwd,
    )
}

/// Expand the denies from an already resolved filesystem policy into Windows ACL targets.
pub fn expand_windows_deny_read_paths(
    permissions: &ResolvedFileSystemPermissions,
    cwd: &AbsolutePathBuf,
) -> Result<Vec<AbsolutePathBuf>, String> {
    expand_deny_read_paths(
        &permissions.unreadable_roots,
        &permissions.unreadable_globs,
        permissions.glob_scan_max_depth,
        cwd,
    )
}

fn expand_deny_read_paths(
    unreadable_roots: &[AbsolutePathBuf],
    unreadable_globs: &[String],
    glob_scan_max_depth: Option<usize>,
    cwd: &AbsolutePathBuf,
) -> Result<Vec<AbsolutePathBuf>, String> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    for path in unreadable_roots {
        push_absolute_path(&mut paths, &mut seen, path.as_path())?;
    }

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
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            })
            .collect(),
    );
    let Some(matcher) = ReadDenyMatcher::try_new_for_local_paths(&glob_policy, cwd.as_path())?
    else {
        return Ok(paths);
    };

    let scan_plans = glob_scan_plans(unreadable_globs, glob_scan_max_depth)?;

    for scan_plan in scan_plans {
        if !scan_plan.root.exists() {
            continue;
        }

        // Snapshot matching entries without following directory links below the scan root.
        let mut filter = OverrideBuilder::new(&scan_plan.root);
        filter.allow_unclosed_class(/*yes*/ true);
        filter
            .case_insensitive(/*yes*/ true)
            .map_err(|err| err.to_string())?;
        for glob in &scan_plan.globs {
            filter.add(glob).map_err(|err| err.to_string())?;
        }
        let filter = filter.build().map_err(|err| err.to_string())?;

        for entry in ignore::WalkBuilder::new(&scan_plan.root)
            .standard_filters(/*yes*/ false)
            .follow_links(/*yes*/ false)
            .max_depth(scan_plan.max_depth)
            .overrides(filter)
            .build()
        {
            let entry = entry.and_then(|entry| {
                if entry.path_is_symlink() && matcher.is_local_path_read_denied(entry.path()) {
                    std::fs::metadata(entry.path()).map_err(ignore::Error::Io)?;
                }
                Ok(entry)
            });
            let entry = match entry {
                Ok(entry) => entry,
                Err(err)
                    if err.io_error().is_some_and(|err| {
                        matches!(
                            err.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                        )
                    }) =>
                {
                    continue;
                }
                Err(err) => {
                    return Err(format!(
                        "failed to enumerate unreadable glob paths under {}: {err}",
                        scan_plan.root.display()
                    ));
                }
            };
            if matcher.is_local_path_read_denied(entry.path()) {
                push_absolute_path(&mut paths, &mut seen, entry.into_path())?;
            }
        }
    }

    Ok(paths)
}

fn push_absolute_path(
    paths: &mut Vec<AbsolutePathBuf>,
    seen: &mut HashSet<PathBuf>,
    path: impl AsRef<Path>,
) -> Result<(), String> {
    let absolute_path = AbsolutePathBuf::from_absolute_path(dunce::simplified(path.as_ref()))
        .map_err(|err| err.to_string())?;
    if seen.insert(absolute_path.to_path_buf()) {
        paths.push(absolute_path);
    }
    Ok(())
}

fn glob_scan_plans(
    patterns: &[String],
    configured_max_depth: Option<usize>,
) -> Result<Vec<GlobScanPlan>, String> {
    let mut scan_plans: Vec<GlobScanPlan> = Vec::new();

    for pattern in patterns {
        let mut scan_plan = glob_scan_plan(pattern, configured_max_depth);
        if scan_plan.max_depth.is_none() && scan_plan.root.parent().is_none() {
            return Err(format!(
                "unreadable glob `{pattern}` cannot be safely expanded from a filesystem root without `glob_scan_max_depth`; configure `glob_scan_max_depth` or use a non-root directory prefix"
            ));
        }

        if let Some(existing) = scan_plans
            .iter_mut()
            .find(|existing| existing.root == scan_plan.root)
        {
            existing.max_depth = match (existing.max_depth, scan_plan.max_depth) {
                (Some(existing_depth), Some(new_depth)) => Some(existing_depth.max(new_depth)),
                _ => None,
            };
            existing.globs.append(&mut scan_plan.globs);
        } else {
            scan_plans.push(scan_plan);
        }
    }

    Ok(scan_plans)
}

fn glob_scan_plan(pattern: &str, configured_max_depth: Option<usize>) -> GlobScanPlan {
    let scan = windows_deny_read_glob_scan(pattern, configured_max_depth);
    GlobScanPlan {
        root: PathBuf::from(scan.root),
        max_depth: scan.max_depth,
        globs: vec![file_filter_glob(scan.pattern_suffix)],
    }
}

fn file_filter_glob(pattern: &str) -> String {
    let pattern = pattern.replace('\\', "/");
    if pattern.starts_with("**/") {
        pattern
    } else {
        format!("**/{pattern}")
    }
}

#[cfg(test)]
#[path = "deny_read_resolver_access_tests.rs"]
mod access_tests;

#[cfg(test)]
mod tests {
    use super::GlobScanPlan;
    use super::file_filter_glob;
    use super::glob_scan_plan;
    use super::glob_scan_plans;
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

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn unreadable_glob_entry(pattern: String) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern { pattern },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        }
    }

    fn unreadable_path_entry(path: PathBuf) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::from_absolute_path(path)
                    .expect("absolute path")
                    .into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        }
    }

    #[test]
    fn scan_root_uses_literal_prefix_before_glob() {
        assert_eq!(
            glob_scan_plan("/tmp/work/**/*.env", /*configured_max_depth*/ None).root,
            PathBuf::from("/tmp/work")
        );
        assert_eq!(
            glob_scan_plan(
                r"C:\Users\dev\repo\**\*.env",
                /*configured_max_depth*/ None,
            )
            .root,
            PathBuf::from(r"C:\Users\dev\repo")
        );
        assert_eq!(
            glob_scan_plan(r"C:\*.env", /*configured_max_depth*/ None).root,
            PathBuf::from(r"C:\")
        );
    }

    #[test]
    fn scan_depth_is_bounded_for_non_recursive_globs() {
        assert_eq!(
            glob_scan_plan("/tmp/work/*.env", /*configured_max_depth*/ None).max_depth,
            Some(1)
        );
        assert_eq!(
            glob_scan_plan("/tmp/work/*/*.env", /*configured_max_depth*/ None).max_depth,
            Some(2)
        );
        assert_eq!(
            glob_scan_plan("/tmp/work/**/*.env", /*configured_max_depth*/ None).max_depth,
            None
        );
    }

    #[test]
    fn configured_depth_caps_recursive_glob_scans() {
        assert_eq!(
            glob_scan_plan("/tmp/work/**/*.env", Some(2)).max_depth,
            Some(2)
        );
        assert_eq!(
            glob_scan_plan("/tmp/work/*/*.env", Some(1)).max_depth,
            Some(1)
        );
    }

    #[test]
    fn glob_patterns_with_the_same_root_and_depth_share_one_scan() {
        assert_eq!(
            glob_scan_plans(
                &[
                    "/tmp/work/**/*.env".to_string(),
                    "/tmp/work/**/*.pem".to_string(),
                    "/tmp/work/**/*.secret".to_string(),
                ],
                /*configured_max_depth*/ Some(3),
            )
            .expect("combined scan plans"),
            vec![GlobScanPlan {
                root: PathBuf::from("/tmp/work"),
                max_depth: Some(3),
                globs: vec![
                    "**/*.env".to_string(),
                    "**/*.pem".to_string(),
                    "**/*.secret".to_string(),
                ],
            }]
        );
    }

    #[test]
    fn bounded_globs_with_the_same_root_share_the_deepest_scan() {
        assert_eq!(
            glob_scan_plans(
                &[
                    "/tmp/work/*.env".to_string(),
                    "/tmp/work/*/*.pem".to_string(),
                ],
                /*configured_max_depth*/ None,
            )
            .expect("combined scan plans"),
            vec![GlobScanPlan {
                root: PathBuf::from("/tmp/work"),
                max_depth: Some(2),
                globs: vec!["**/*.env".to_string(), "**/*/*.pem".to_string()],
            }]
        );
    }

    #[test]
    fn recursive_and_bounded_globs_with_the_same_root_share_one_scan() {
        assert_eq!(
            glob_scan_plans(
                &[
                    "/tmp/work/*.env".to_string(),
                    "/tmp/work/**/*.pem".to_string(),
                ],
                /*configured_max_depth*/ None,
            )
            .expect("combined scan plans"),
            vec![GlobScanPlan {
                root: PathBuf::from("/tmp/work"),
                max_depth: None,
                globs: vec!["**/*.env".to_string(), "**/*.pem".to_string()],
            }]
        );
    }

    #[test]
    fn project_recursive_globs_without_depth_expand_existing_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let secret = tmp.path().join("nested").join("secret.env");
        std::fs::create_dir_all(secret.parent().expect("secret parent")).expect("create parent");
        std::fs::write(&secret, "secret").expect("write secret");
        let pattern = tmp.path().join("**").join("*.env").display().to_string();
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(pattern)]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("unbounded project glob"),
            vec![AbsolutePathBuf::from_absolute_path(secret).expect("absolute secret")]
        );
    }

    #[test]
    fn relative_recursive_globs_without_depth_expand_existing_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let secret = tmp.path().join("nested").join("secret.env");
        std::fs::create_dir_all(secret.parent().expect("secret parent")).expect("create parent");
        std::fs::write(&secret, "secret").expect("write secret");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
            "**/*.env".to_string(),
        )]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd)
                .expect("unbounded relative project glob"),
            vec![AbsolutePathBuf::from_absolute_path(secret).expect("absolute secret")]
        );
    }

    #[test]
    fn root_recursive_globs_without_depth_fail_before_expansion() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let root = cwd.as_path().ancestors().last().expect("filesystem root");
        let pattern = root.join("**").join("*.env").display().to_string();
        let policy =
            FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(pattern.clone())]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect_err("unbounded root glob"),
            format!(
                "unreadable glob `{pattern}` cannot be safely expanded from a filesystem root without `glob_scan_max_depth`; configure `glob_scan_max_depth` or use a non-root directory prefix"
            )
        );
    }

    #[test]
    fn configured_depth_bounds_root_recursive_glob_scans() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let root = cwd.as_path().ancestors().last().expect("filesystem root");
        let pattern = root.join("**").join("*.env").display().to_string();
        let scan_plan = glob_scan_plan(&pattern, Some(2));

        assert_eq!(scan_plan.root, root);
        assert_eq!(scan_plan.max_depth, Some(2));
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
        let uppercase_env = tmp.path().join("app").join("SECRET.ENV");
        let notes = tmp.path().join("app").join("notes.txt");
        std::fs::create_dir_all(notes.parent().expect("parent")).expect("create parent");
        std::fs::write(&root_env, "secret").expect("write root env");
        std::fs::write(&nested_env, "secret").expect("write nested env");
        std::fs::write(&uppercase_env, "secret").expect("write uppercase env");
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
        let mut expected = [root_env, nested_env].into_iter().collect::<HashSet<_>>();
        if cfg!(windows) {
            expected.insert(uppercase_env);
        }

        assert_eq!(actual, expected);
    }

    #[test]
    fn recursive_globs_include_hidden_and_ignored_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let hidden_secret = tmp.path().join("nested").join(".env");
        let ignored_secret = tmp.path().join("nested").join("ignored.env");
        std::fs::create_dir_all(hidden_secret.parent().expect("secret parent"))
            .expect("create parent");
        std::fs::write(tmp.path().join(".ignore"), "ignored.env\n").expect("write ignore file");
        std::fs::write(&hidden_secret, "secret").expect("write hidden secret");
        std::fs::write(&ignored_secret, "secret").expect("write ignored secret");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
            "**/*.env".to_string(),
        )]);

        let actual: HashSet<PathBuf> = resolve_windows_deny_read_paths(&policy, &cwd)
            .expect("resolve hidden and ignored matches")
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf)
            .collect();

        assert_eq!(
            actual,
            [hidden_secret, ignored_secret].into_iter().collect()
        );
    }

    #[test]
    fn recursive_globs_preserve_matching_empty_directories() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let root_secret = tmp.path().join("root.env");
        let nested_secret = tmp.path().join("nested").join("empty.env");
        std::fs::create_dir_all(&root_secret).expect("create root secret directory");
        std::fs::create_dir_all(&nested_secret).expect("create nested secret directory");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
            "**/*.env".to_string(),
        )]);

        let actual: HashSet<PathBuf> = resolve_windows_deny_read_paths(&policy, &cwd)
            .expect("resolve matching directories")
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf)
            .collect();

        assert_eq!(actual, [root_secret, nested_secret].into_iter().collect());
    }

    #[test]
    fn configured_depth_excludes_deeper_recursive_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let shallow_secret = tmp.path().join("shallow.env");
        let deep_secret = tmp.path().join("nested").join("deep.env");
        std::fs::create_dir_all(deep_secret.parent().expect("secret parent"))
            .expect("create parent");
        std::fs::write(&shallow_secret, "secret").expect("write shallow secret");
        std::fs::write(&deep_secret, "secret").expect("write deep secret");
        let mut policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
            "**/*.env".to_string(),
        )]);
        policy.glob_scan_max_depth = Some(1);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("resolve depth-limited glob"),
            vec![AbsolutePathBuf::from_absolute_path(shallow_secret).expect("absolute secret")]
        );
    }

    #[test]
    fn recursive_globs_rescan_for_new_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        std::fs::write(tmp.path().join("notes.txt"), "notes").expect("write notes");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
            "**/*.env".to_string(),
        )]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("resolve unmatched glob"),
            Vec::<AbsolutePathBuf>::new()
        );
        let secret = tmp.path().join("new.env");
        std::fs::write(&secret, "new secret").expect("create matching file");
        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("fresh resolution"),
            vec![AbsolutePathBuf::from_absolute_path(secret).expect("secret path")]
        );
    }

    #[test]
    fn file_filter_globs_normalize_windows_separators_and_unclosed_classes() {
        assert_eq!(file_filter_glob(r"**\*.env"), "**/*.env");
        assert_eq!(file_filter_glob("nested/*.env"), "**/nested/*.env");
        assert_eq!(file_filter_glob("[*.env"), "**/[*.env");

        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("cwd");
        for (name, pattern) in [
            ("[]secret.env", "**/[]*.env"),
            ("[[secret.env", "**/[[*.env"),
        ] {
            let secret = cwd.join(name);
            std::fs::write(&secret, "secret").expect("write literal bracket name");
            let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
                pattern.to_owned(),
            )]);
            assert_eq!(
                resolve_windows_deny_read_paths(&policy, &cwd).expect("valid literal bracket glob"),
                vec![secret],
            );
        }
    }

    #[test]
    fn shared_scan_roots_preserve_matches_at_each_required_depth() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let env = tmp.path().join(".env");
        let pem = tmp.path().join("nested").join("credentials.pem");
        std::fs::create_dir_all(pem.parent().expect("pem parent")).expect("create pem parent");
        std::fs::write(&env, "secret").expect("write env");
        std::fs::write(&pem, "secret").expect("write pem");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            unreadable_glob_entry(format!("{}/*.env", tmp.path().display())),
            unreadable_glob_entry(format!("{}/*/*.pem", tmp.path().display())),
        ]);

        let actual: HashSet<PathBuf> = resolve_windows_deny_read_paths(&policy, &cwd)
            .expect("resolve shared root")
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf)
            .collect();
        let expected = [env, pem].into_iter().collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_glob_patterns_fail_before_expansion() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(format!(
            "{}/**/[z-a]",
            tmp.path().display()
        ))]);

        let err = resolve_windows_deny_read_paths(&policy, &cwd).expect_err("invalid glob");
        assert!(
            err.contains("invalid deny-read glob pattern"),
            "unexpected error: {err}"
        );
        assert!(err.contains("invalid range"), "unexpected error: {err}");
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

    #[cfg(unix)]
    #[test]
    fn aliased_glob_roots_each_preserve_their_lexical_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let target = tmp.path().join("target");
        let alias_a = tmp.path().join("alias-a");
        let alias_b = tmp.path().join("alias-b");
        let secret = target.join("secret.env");
        std::fs::create_dir_all(&target).expect("create target");
        std::fs::write(&secret, "secret").expect("write secret");
        symlink(&target, &alias_a).expect("create alias a");
        symlink(&target, &alias_b).expect("create alias b");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            unreadable_glob_entry(format!("{}/**/*.env", alias_a.display())),
            unreadable_glob_entry(format!("{}/**/*.env", alias_b.display())),
        ]);

        let actual: HashSet<PathBuf> = resolve_windows_deny_read_paths(&policy, &cwd)
            .expect("resolve")
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf)
            .collect();
        let expected = [alias_a.join("secret.env"), alias_b.join("secret.env")]
            .into_iter()
            .collect();

        assert_eq!(actual, expected);
    }

    #[cfg(unix)]
    #[test]
    fn recursive_globs_preserve_symlinked_file_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let cwd = AbsolutePathBuf::from_absolute_path(tmp.path()).expect("absolute cwd");
        let target = tmp.path().join("target.txt");
        let alias = tmp.path().join("linked.env");
        std::fs::write(&target, "secret").expect("write symlink target");
        symlink(&target, &alias).expect("create symlink");
        let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
            "**/*.env".to_string(),
        )]);

        assert_eq!(
            resolve_windows_deny_read_paths(&policy, &cwd).expect("resolve symlinked secret"),
            vec![AbsolutePathBuf::from_absolute_path(alias).expect("absolute symlink")]
        );
    }
}
