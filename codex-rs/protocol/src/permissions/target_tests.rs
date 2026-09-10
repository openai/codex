//! Regression coverage for remote path matching and managed-denial enforcement.

use super::*;
use crate::permissions::FileSystemAccessMode;
use crate::permissions::FileSystemSandboxEntry;
use crate::permissions::PROTECTED_METADATA_PATH_NAMES;
use pretty_assertions::assert_eq;

fn uri(input: &str) -> PathUri {
    PathUri::parse(input).unwrap()
}

fn deny(path: FileSystemPath) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(path, FileSystemAccessMode::Deny)
}

#[test]
fn read_denials_use_target_paths_and_native_glob_semantics() {
    for base in [
        "file:///work/repo",
        "file:///Users/agent/repo",
        "file:///C:/work/repo",
        "file://server/share/repo",
    ] {
        let cwd = uri(base);
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: std::slice::from_ref(&cwd),
            user_home_dir: Some(&cwd),
            temporary_directories: Some(&[]),
        };
        let policy = FileSystemSandboxPolicy::restricted(vec![
            deny(FileSystemPath::Path {
                path: cwd.join("private").unwrap(),
            }),
            deny(FileSystemPath::GlobPattern {
                pattern: "~/secrets/*.key".into(),
            }),
            deny(FileSystemPath::GlobPattern {
                pattern: "nested/**/*.env".into(),
            }),
        ]);
        let matcher = ReadDenyMatcher::try_new_with_context(&policy, &context)
            .unwrap()
            .unwrap();
        for (path, expected) in [
            ("private", true),
            ("private/file", true),
            ("private-other/file", false),
            ("secrets/token.key", true),
            ("secrets/nested/token.key", false),
            ("nested/.env", true),
            ("nested/deep/token.env", true),
            ("public/file", false),
        ] {
            assert_eq!(
                matcher.is_read_denied_uri(&cwd.join(path).unwrap(), &context),
                expected,
                "{base}: {path}"
            );
        }
        assert_eq!(
            matcher.is_read_denied_uri(&cwd.join("PRIVATE/file").unwrap(), &context),
            cwd.infer_path_convention() == Some(PathConvention::Windows),
        );
        assert!(matcher.is_read_denied_uri(&uri("file:///C:/foreign%2Fpath"), &context));
    }
}

#[test]
fn managed_denials_reject_concrete_read_and_write_grants() {
    for base in [
        "file:///work/repo",
        "file:///C:/work/repo",
        "file://server/share/repo",
    ] {
        let cwd = uri(base);
        let private = cwd.join("private").unwrap();
        let required = FileSystemSandboxPolicy::restricted(vec![deny(FileSystemPath::Path {
            path: private.clone(),
        })]);
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: std::slice::from_ref(&cwd),
            user_home_dir: None,
            temporary_directories: Some(&[]),
        };
        let mut policy = FileSystemSandboxPolicy::workspace_write_with_path_uris(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );
        policy.entries.extend(required.entries.clone());
        assert_eq!(
            policy.validate_managed_deny_read(&required, &context),
            Ok(())
        );
        for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Write] {
            let mut carveout = policy.clone();
            carveout.entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::Path {
                    path: private.join("carveout").unwrap(),
                },
                access,
            ));
            assert_eq!(
                carveout.validate_managed_deny_read(&required, &context),
                Err(SHADOWED.into())
            );
        }
    }
}

#[test]
fn managed_denials_preserve_native_metadata_read_rejections() {
    for base in [
        "file:///executor/home",
        "file:///C:/Users/executor",
        "file://server/share/executor",
    ] {
        let extra_root = uri(base);
        let cwd = extra_root.join("workspace").unwrap();
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: std::slice::from_ref(&cwd),
            user_home_dir: Some(&extra_root),
            temporary_directories: Some(&[]),
        };
        let legacy = FileSystemSandboxPolicy::workspace_write_with_path_uris(
            std::slice::from_ref(&extra_root),
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );
        for name in [".codex", ".git"] {
            let denied_root = extra_root.join(name).unwrap();
            let child = denied_root.join("reopened").unwrap();
            for (path, expected_root_access) in [
                (
                    FileSystemPath::Path {
                        path: denied_root.clone(),
                    },
                    FileSystemAccessMode::Deny,
                ),
                (
                    FileSystemPath::GlobPattern {
                        pattern: denied_root.join("*").unwrap().inferred_native_path_string(),
                    },
                    FileSystemAccessMode::Read,
                ),
            ] {
                let required = FileSystemSandboxPolicy::restricted(vec![deny(path)]);
                let mut policy = legacy.clone();
                policy.preserve_deny_read_restrictions_from(&required);
                assert_eq!(
                    (
                        policy.resolve_access(&denied_root, &context),
                        policy.validate_managed_deny_read(&required, &context),
                    ),
                    (
                        expected_root_access,
                        if expected_root_access == FileSystemAccessMode::Deny {
                            Err(SHADOWED.into())
                        } else {
                            Ok(())
                        },
                    ),
                    "{base}: {name}",
                );
                let matcher = ReadDenyMatcher::try_new_with_context(&required, &context)
                    .unwrap()
                    .unwrap();
                assert!(matcher.is_read_denied_uri(&child, &context));
                for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Write] {
                    let mut reopened = policy.clone();
                    reopened.entries.push(FileSystemSandboxEntry::new(
                        FileSystemPath::Path {
                            path: child.clone(),
                        },
                        access,
                    ));
                    assert_eq!(
                        reopened.validate_managed_deny_read(&required, &context),
                        Err(SHADOWED.into()),
                        "{base}: {name}: {access}",
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_or_unresolvable_denials_fail_closed() {
    let cwd = uri("file:///work/repo");
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: &[],
        user_home_dir: None,
        temporary_directories: Some(&[]),
    };
    for pattern in ["~/secrets/**", "secrets/[z-a]"] {
        let policy = FileSystemSandboxPolicy::restricted(vec![deny(FileSystemPath::GlobPattern {
            pattern: pattern.into(),
        })]);
        assert!(ReadDenyMatcher::try_new_with_context(&policy, &context).is_err());
    }
    let policy = FileSystemSandboxPolicy::restricted(vec![deny(FileSystemPath::Path {
        path: uri("file:///C:/private"),
    })]);
    assert!(ReadDenyMatcher::try_new_with_context(&policy, &context).is_err());
}

#[test]
fn slash_tmp_denials_follow_the_executor_convention() {
    let policy = FileSystemSandboxPolicy::restricted(vec![deny(FileSystemPath::Special {
        value: FileSystemSpecialPath::SlashTmp,
    })]);
    for (base, active) in [("file:///work", true), ("file:///C:/work", false)] {
        let cwd = uri(base);
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: &[],
            user_home_dir: None,
            temporary_directories: None,
        };
        let matcher = ReadDenyMatcher::try_new_with_context(&policy, &context).unwrap();
        assert_eq!(matcher.is_some(), active);
        if let Some(matcher) = matcher {
            assert!(matcher.is_read_denied_uri(&cwd.join("/tmp/file").unwrap(), &context));
        }
    }
}

#[test]
fn remote_workspace_write_keeps_metadata_protected() {
    let cwd = uri("file:///C:/repo");
    let extra = uri("file:///D:/extra");
    let policy = FileSystemSandboxPolicy::workspace_write_with_path_uris(
        std::slice::from_ref(&extra),
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: std::slice::from_ref(&cwd),
        user_home_dir: None,
        temporary_directories: Some(&[]),
    };
    for name in PROTECTED_METADATA_PATH_NAMES {
        assert!(
            policy
                .entries
                .contains(&FileSystemSandboxEntry::skip_missing_path(
                    FileSystemPath::Path {
                        path: extra.join(name).unwrap()
                    },
                    FileSystemAccessMode::Read,
                ))
        );
    }
    for root in [cwd.clone(), extra] {
        assert!(policy.can_write_path(&root.join("file").unwrap(), &context));
        for name in PROTECTED_METADATA_PATH_NAMES {
            let path = root.join(name).unwrap().join("file").unwrap();
            assert!(policy.can_read_path(&path, &context));
            assert!(!policy.can_write_path(&path, &context));
        }
    }
}

#[test]
fn read_globs_reject_metacharacters_in_home_and_cwd_facts() {
    for prefix in ["file:///home", "file:///C:/Users"] {
        let parent = uri(prefix);
        let directory = parent.join("sam[1]").unwrap();
        for (pattern, cwd, home) in [
            ("~/private/*.key", &parent, Some(&directory)),
            ("private/*.key", &directory, None),
        ] {
            let policy =
                FileSystemSandboxPolicy::restricted(vec![deny(FileSystemPath::GlobPattern {
                    pattern: pattern.into(),
                })]);
            let context = FileSystemSandboxPolicyContext {
                cwd,
                workspace_roots: std::slice::from_ref(&directory),
                user_home_dir: home,
                temporary_directories: None,
            };
            assert!(ReadDenyMatcher::try_new_with_context(&policy, &context).is_err());
            let matcher = ReadDenyMatcher::from_context(&policy, &context).unwrap();
            assert!(
                matcher.is_read_denied_uri(&directory.join("private/a.key").unwrap(), &context)
            );
        }
    }
}

#[test]
fn workspace_globs_fail_closed_on_literal_root_metacharacters_on_every_host() {
    for prefix in ["file:///home", "file:///C:/Users"] {
        let root = uri(prefix).join("sam[1]").unwrap();
        let policy = FileSystemSandboxPolicy::restricted(vec![deny(FileSystemPath::GlobPattern {
            pattern: crate::permissions::project_roots_glob_pattern(std::path::Path::new(
                "private/*.key",
            )),
        })]);
        let materialized = policy
            .clone()
            .materialize_project_roots_with_path_uris(std::slice::from_ref(&root));
        assert_eq!(
            materialized,
            FileSystemSandboxPolicy::restricted(vec![deny(root.clone().into())]),
        );
        if let Ok(native_root) = root.to_abs_path() {
            assert_eq!(
                policy.materialize_project_roots_with_workspace_roots(&[native_root]),
                materialized,
            );
        }
    }
}

#[test]
fn materializing_legacy_home_relative_workspace_denials_removes_all_grants() {
    for (root, home) in [
        ("file:///work/repo", "file:///home/sam"),
        ("file:///C:/work/repo", "file:///D:/Users/sam"),
    ] {
        let root = uri(root);
        let home = uri(home);
        let convention = root.infer_path_convention().unwrap();
        for subpath in ["~", "~/private/*.env", r"~\private", r"~\private\*.env"] {
            if convention.home_relative_suffix(subpath).is_none() {
                continue;
            }
            let denied_path = if subpath.contains('*') {
                FileSystemPath::GlobPattern {
                    pattern: crate::permissions::project_roots_glob_pattern(std::path::Path::new(
                        subpath,
                    )),
                }
            } else {
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots {
                        subpath: Some(subpath.into()),
                    },
                }
            };
            let policy = FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    FileSystemAccessMode::Read,
                ),
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::ProjectRoots { subpath: None },
                    },
                    FileSystemAccessMode::Write,
                ),
                FileSystemSandboxEntry::new(home.clone().into(), FileSystemAccessMode::Write),
                deny(denied_path),
            ]);
            let mut materialized = vec![
                policy
                    .clone()
                    .materialize_project_roots_with_path_uris(std::slice::from_ref(&root)),
                policy
                    .clone()
                    .with_materialized_project_roots_for_path_uris(std::slice::from_ref(&root)),
            ];
            if let Ok(native_root) = root.to_abs_path() {
                materialized.push(
                    policy
                        .clone()
                        .materialize_project_roots_with_workspace_roots(std::slice::from_ref(
                            &native_root,
                        )),
                );
                materialized.push(policy.with_materialized_project_roots_for_workspace_roots(
                    std::slice::from_ref(&native_root),
                ));
            }
            for policy in materialized {
                assert_eq!(policy, FileSystemSandboxPolicy::restricted(Vec::new()));
            }
        }
    }
}
