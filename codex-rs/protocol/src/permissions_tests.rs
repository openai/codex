#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[cfg(unix)]
const SYMLINKED_TMPDIR_TEST_ENV: &str = "CODEX_PROTOCOL_TEST_SYMLINKED_TMPDIR";

#[cfg(unix)]
fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn writable_roots_protect_gitdir_target_outside_alias_root() {
    let temp = TempDir::new().expect("tempdir");
    let base = temp.path().canonicalize().expect("canonical tempdir");
    let workspace = base.join("workspace");
    let writable = base.join("writable");
    let aliases = base.join("readonly-aliases");
    let gitdir = writable.join("gitdir");
    fs::create_dir(&workspace).expect("create workspace");
    fs::create_dir_all(&gitdir).expect("create gitdir");
    fs::create_dir(&aliases).expect("create aliases");
    let alias = aliases.join("repo");
    symlink_dir(&writable, &alias).expect("create alias");
    fs::write(
        workspace.join(".git"),
        format!("gitdir: {}\n", alias.join("gitdir").display()),
    )
    .expect("write Git pointer");
    let policy = FileSystemSandboxPolicy::restricted(
        [&workspace, &writable]
            .into_iter()
            .map(|path| {
                FileSystemSandboxEntry::new(
                    AbsolutePathBuf::from_absolute_path(path)
                        .expect("absolute root")
                        .into(),
                    FileSystemAccessMode::Write,
                )
            })
            .collect(),
    );
    let roots = if cfg!(target_os = "macos") {
        policy.get_writable_roots_with_cwd_preserving_mutable_paths(&workspace)
    } else {
        policy.get_writable_roots_with_cwd(&workspace)
    };
    let root = roots
        .iter()
        .find(|root| root.root.as_path() == writable)
        .expect("additional writable root");
    assert_eq!(
        root.read_only_subpaths,
        vec![AbsolutePathBuf::from_absolute_path(gitdir).expect("absolute gitdir")],
    );
}

#[test]
fn permission_paths_preserve_native_strings_across_path_conventions() {
    for path in [
        "/workspace/src",
        r"C:\workspace\src",
        r"\\server\share\src",
        r"\\localhost\share",
    ] {
        let expected = serde_json::json!({ "type": "path", "path": path });
        let actual = serde_json::from_value::<RawFileSystemPath>(expected.clone())
            .expect("valid raw permission path");
        assert_eq!(
            serde_json::to_value(actual).expect("lossless raw permission path"),
            expected
        );
    }
}

#[test]
fn writable_root_presence_matches_materialized_roots() {
    let cwd = TempDir::new().expect("tempdir");
    let writable_root = AbsolutePathBuf::resolve_path_against_base("work", cwd.path());
    let policies = [
        FileSystemSandboxPolicy::read_only(),
        FileSystemSandboxPolicy::unrestricted(),
        FileSystemSandboxPolicy::external_sandbox(),
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Write,
        )]),
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            writable_root.clone().into(),
            FileSystemAccessMode::Write,
        )]),
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(writable_root.clone().into(), FileSystemAccessMode::Write),
            FileSystemSandboxEntry::new(writable_root.into(), FileSystemAccessMode::Deny),
        ]),
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(
                    ":future_special_path",
                    /*subpath*/ None,
                ),
            },
            FileSystemAccessMode::Write,
        )]),
    ];

    for policy in policies {
        assert_eq!(
            policy.has_configured_writable_roots_with_cwd(cwd.path()),
            !policy.get_writable_roots_with_cwd(cwd.path()).is_empty()
        );
    }
}

#[cfg(windows)]
#[test]
fn permission_paths_preserve_native_slash_unc_strings() {
    for path in ["//server/share/src", r"/\server/share/src"] {
        let expected = serde_json::json!({ "type": "path", "path": path });
        let actual = serde_json::from_value::<RawFileSystemPath>(expected.clone())
            .expect("valid raw slash UNC permission path");
        assert_eq!(
            serde_json::to_value(actual).expect("lossless raw slash UNC permission path"),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn native_ambiguous_permission_paths_keep_deny_semantics() {
    let cwd = TempDir::new().expect("tempdir");
    for path in ["//server/share/secret", "/C:/secret"] {
        let denied_path = serde_json::from_value::<RawFileSystemPath>(serde_json::json!({
            "type": "path",
            "path": path,
        }))
        .expect("raw permission path")
        .try_into()
        .expect("runtime permission path");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(denied_path, FileSystemAccessMode::Deny),
        ]);

        assert!(
            !policy.can_read_local_path_with_cwd(Path::new(path), cwd.path()),
            "deny should apply to {path}"
        );
        assert!(
            policy.can_read_local_path_with_cwd(&cwd.path().join("ordinary"), cwd.path()),
            "opaque deny for {path} should not poison ordinary paths"
        );
    }
}

#[test]
fn uri_matcher_resolves_selected_executor_paths() {
    let path = |path| PathUri::parse(path).expect("valid path URI");
    let cwd = path("file:///C:/workspace");
    let workspace_roots = [cwd.clone()];
    let temporary_directories = [path("file:///C:/Temp")];
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: &workspace_roots,
        user_home_dir: None,
        temporary_directories: Some(&temporary_directories),
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
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            path("file:///C:/workspace/private").into(),
            FileSystemAccessMode::Deny,
        ),
        FileSystemSandboxEntry::new(
            path("file:///C:/workspace/private/public").into(),
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            FileSystemAccessMode::Write,
        ),
    ]);

    for (candidate, expected) in [
        (
            "file:///C:/workspace/src/main.rs",
            FileSystemAccessMode::Write,
        ),
        (
            "file:///c:/WORKSPACE/private/key",
            FileSystemAccessMode::Deny,
        ),
        (
            "file:///C:/workspace/private/public/ok",
            FileSystemAccessMode::Write,
        ),
        ("file:///C:/Temp/cache", FileSystemAccessMode::Write),
        ("file:///C:/outside", FileSystemAccessMode::Read),
        ("file:///tmp/cache", FileSystemAccessMode::Deny),
    ] {
        assert_eq!(
            policy.resolve_access(&path(candidate), &context),
            expected,
            "resolving {candidate}"
        );
    }
    assert!(!policy.can_write_path(&path("file:///c:/WORKSPACE/.git/config"), &context,));

    let scoped = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
        FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(Some("docs/../public".to_string())),
        },
        FileSystemAccessMode::Write,
    )]);
    assert_eq!(
        scoped.resolve_access(&path("file:///C:/workspace/public/file"), &context),
        FileSystemAccessMode::Write
    );
    assert_eq!(
        scoped.resolve_access(&path("file:///C:/outside/file"), &context),
        FileSystemAccessMode::Deny
    );
}

#[test]
fn uri_matcher_uses_validated_native_components_for_precedence() {
    let path = |path| PathUri::parse(path).expect("valid path URI");
    let cwd = path("file:///workspace");
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: std::slice::from_ref(&cwd),
        user_home_dir: None,
        temporary_directories: None,
    };
    let candidate = path("file:///workspace/private/secret/key");

    for (case, writable_path, denied_path) in [
        (
            "encoded component",
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            path("file:///workspace/%70rivate").into(),
        ),
        (
            "encoded separator",
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            path("file:///workspace/private%2Fsecret").into(),
        ),
        (
            "repeated separators",
            path("file:///workspace////").into(),
            path("file:///workspace/private").into(),
        ),
    ] {
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(writable_path, FileSystemAccessMode::Write),
            FileSystemSandboxEntry::new(denied_path, FileSystemAccessMode::Deny),
        ]);

        assert_eq!(
            policy.resolve_access(&candidate, &context),
            FileSystemAccessMode::Deny,
            "resolving {case}"
        );
    }
}

#[test]
fn uri_matcher_fails_closed_without_executor_roots_or_lexical_paths() {
    let path = |path| PathUri::parse(path).expect("valid path URI");
    let cwd = path("file://server/share/workspace");
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: &[],
        user_home_dir: None,
        temporary_directories: None,
    };
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            FileSystemAccessMode::Write,
        ),
    ]);

    assert_eq!(
        policy.resolve_access(&cwd, &context),
        FileSystemAccessMode::Deny
    );
    assert_eq!(
        policy.resolve_access(&path("file:///tmp/cache"), &context),
        FileSystemAccessMode::Deny
    );

    let opaque = path("file:///%00/bad/path/YQ");
    let opaque_context = FileSystemSandboxPolicyContext {
        cwd: &opaque,
        workspace_roots: std::slice::from_ref(&opaque),
        user_home_dir: None,
        temporary_directories: None,
    };
    let opaque_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
        opaque.clone().into(),
        FileSystemAccessMode::Write,
    )]);
    assert_eq!(
        opaque_policy.resolve_access(&opaque, &opaque_context),
        FileSystemAccessMode::Deny
    );
}

#[test]
fn uri_deny_and_metadata_matcher_use_executor_paths() {
    let path = |path| PathUri::parse(path).expect("valid path URI");
    let cwd = path("file:///C:/workspace");
    let roots = [cwd.clone()];
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: &roots,
        user_home_dir: None,
        temporary_directories: None,
    };
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(cwd.clone().into(), FileSystemAccessMode::Write),
        FileSystemSandboxEntry::new(path("file:///C:/").into(), FileSystemAccessMode::Deny),
        FileSystemSandboxEntry::new(
            path("file:///C:/workspace/.codex").into(),
            FileSystemAccessMode::Read,
        ),
        unreadable_glob_entry(r"C:\workspace\**\*.env".to_string()),
    ]);
    let prepared = policy
        .prepare_deny_read_matcher(&context, InvalidDenyReadGlobBehavior::ReturnError)
        .expect("remote deny matcher");

    assert!(prepared.denied_roots.is_empty());
    assert!(FileSystemSandboxPolicy::matches_prepared_read_deny(
        &path("file:///c:/WORKSPACE/app/.ENV"),
        &context,
        &prepared,
    ));
    for candidate in ["file:///%00/bad/path/YQ", "file:///C:/workspace/%2Fsecret"] {
        assert!(FileSystemSandboxPolicy::matches_prepared_read_deny(
            &path(candidate),
            &context,
            &prepared,
        ));
    }
    assert!(!policy.can_write_path(&path("file:///C:/workspace/.codex/config"), &context));
    assert_eq!(
        policy.metadata_write_denial(&path("file:///C:/workspace/.codex/config"), &context),
        Some(".codex"),
    );
}

#[test]
fn uri_deny_globs_use_executor_home() {
    for (cwd, home, pattern, candidate) in [
        (
            "file:///workspace",
            "file:///home/executor",
            "~/private/*.env",
            "file:///home/executor/private/secret.env",
        ),
        (
            "file:///workspace",
            "file:///home/executor",
            "~//private/*.env",
            "file:///home/executor/private/secret.env",
        ),
        (
            "file:///C:/workspace",
            "file:///C:/Users/executor",
            r"~\private\*.env",
            "file:///C:/Users/executor/private/secret.env",
        ),
        (
            "file:///C:/workspace",
            "file:///C:/Users/executor",
            r"~\\private\*.env",
            "file:///C:/Users/executor/private/secret.env",
        ),
    ] {
        let cwd = PathUri::parse(cwd).expect("executor cwd");
        let home = PathUri::parse(home).expect("executor home");
        let candidate = PathUri::parse(candidate).expect("denied path");
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: std::slice::from_ref(&cwd),
            user_home_dir: Some(&home),
            temporary_directories: None,
        };
        let policy =
            FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(pattern.to_string())]);
        let prepared = policy
            .prepare_deny_read_matcher(&context, InvalidDenyReadGlobBehavior::ReturnError)
            .expect("home-relative deny glob");

        assert!(FileSystemSandboxPolicy::matches_prepared_read_deny(
            &candidate, &context, &prepared,
        ));

        let without_home = FileSystemSandboxPolicyContext {
            user_home_dir: None,
            ..context
        };
        assert!(
            policy
                .prepare_deny_read_matcher(&without_home, InvalidDenyReadGlobBehavior::ReturnError,)
                .is_err()
        );
        let prepared = policy
            .prepare_deny_read_matcher(&without_home, InvalidDenyReadGlobBehavior::FailClosed)
            .expect("missing executor home fails closed");
        assert!(prepared.invalid_pattern);
    }
}

#[test]
fn unknown_special_paths_are_ignored_by_legacy_bridge() -> std::io::Result<()> {
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(
                    ":future_special_path",
                    /*subpath*/ None,
                ),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    let sandbox_policy = policy.to_legacy_sandbox_policy(
        NetworkSandboxPolicy::Restricted,
        Path::new("/tmp/workspace"),
    )?;

    assert_eq!(
        sandbox_policy,
        SandboxPolicy::ReadOnly {
            network_access: false,
        }
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn workspace_write_ignores_slash_tmp_special_path_but_preserves_literal_path() {
    let cwd = TempDir::new().expect("tempdir");
    let slash_tmp = AbsolutePathBuf::from_absolute_path("/tmp").expect("absolute tmp path");
    let slash_tmp_only_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::SlashTmp,
            },
            FileSystemAccessMode::Write,
        ),
    ]);
    assert_eq!(
        slash_tmp_only_policy
            .to_legacy_sandbox_policy(NetworkSandboxPolicy::Restricted, cwd.path())
            .expect("legacy sandbox policy"),
        SandboxPolicy::ReadOnly {
            network_access: false,
        }
    );
    assert!(
        !slash_tmp_only_policy
            .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path())
    );

    for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Deny] {
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Write,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::SlashTmp,
                },
                access,
            ),
        ]);
        assert!(policy.has_full_disk_write_access());
        assert!(policy.has_full_disk_read_access());
    }

    let legacy_policy = SandboxPolicy::new_workspace_write_policy();
    assert_eq!(
        FileSystemSandboxPolicy::from(&legacy_policy)
            .to_legacy_sandbox_policy(NetworkSandboxPolicy::Restricted, cwd.path())
            .expect("legacy workspace-write policy"),
        legacy_policy
    );
    assert_eq!(
        FileSystemSandboxPolicy::from(&legacy_policy)
            .get_writable_roots_with_cwd(cwd.path())
            .into_iter()
            .map(|root| root.root)
            .collect::<Vec<_>>(),
        legacy_policy
            .get_writable_roots_with_cwd(cwd.path())
            .into_iter()
            .map(|root| normalize_effective_absolute_path(root.root))
            .collect::<Vec<_>>()
    );
    assert!(
        FileSystemSandboxPolicy::workspace_write(
            std::slice::from_ref(&slash_tmp),
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false,
        )
        .can_write_local_path_with_cwd(slash_tmp.as_path(), cwd.path())
    );
}

#[cfg(unix)]
#[test]
fn writable_roots_proactively_protect_missing_dot_codex() {
    let cwd = TempDir::new().expect("tempdir");
    let expected_root =
        AbsolutePathBuf::from_absolute_path(cwd.path().canonicalize().expect("canonicalize cwd"))
            .expect("absolute canonical root");
    let expected_dot_codex = expected_root.join(".codex");

    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_dot_codex)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn preserving_mutable_paths_normalizes_top_level_aliases_consistently() {
    let root = TempDir::new_in("/tmp").expect("tempdir under /tmp");
    let logical_root =
        AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute logical root");
    let canonical_root =
        AbsolutePathBuf::from_absolute_path(root.path().canonicalize().expect("canonicalize root"))
            .expect("absolute canonical root");
    let protected = canonical_root.join("protected");
    fs::create_dir(&protected).expect("create protected path");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(logical_root.into(), FileSystemAccessMode::Write),
        FileSystemSandboxEntry::new(protected.clone().into(), FileSystemAccessMode::Read),
    ]);

    let roots = policy.get_writable_roots_with_cwd_preserving_mutable_paths(root.path());

    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].root, canonical_root);
    assert!(roots[0].read_only_subpaths.contains(&protected));
}

#[cfg(unix)]
#[test]
fn preserving_writable_roots_cannot_be_rebound_during_projection() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::thread;

    let tmp = TempDir::new().expect("tempdir");
    let active_ancestor = tmp.path().join("active");
    let parked_ancestor = tmp.path().join("parked");
    let outside_ancestor = tmp.path().join("outside");
    let writable_root = active_ancestor.join("workspace");
    let outside_root = outside_ancestor.join("workspace");
    fs::create_dir_all(&writable_root).expect("create writable root");
    fs::create_dir_all(&outside_root).expect("create outside root");
    let writable_root =
        AbsolutePathBuf::from_absolute_path(writable_root).expect("absolute writable root");
    let outside_root =
        AbsolutePathBuf::from_absolute_path(outside_root).expect("absolute outside root");
    let expected_writable_root = writable_root
        .normalize_system_aliases()
        .expect("normalize writable root");
    let expected_outside_root = outside_root
        .normalize_system_aliases()
        .expect("normalize outside root");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
        writable_root.into(),
        FileSystemAccessMode::Write,
    )]);

    let stop = Arc::new(AtomicBool::new(false));
    let swaps = Arc::new(AtomicUsize::new(0));
    let racer_stop = Arc::clone(&stop);
    let racer_swaps = Arc::clone(&swaps);
    let racer = thread::spawn(move || {
        while !racer_stop.load(Ordering::Relaxed) {
            if fs::rename(&active_ancestor, &parked_ancestor).is_err() {
                thread::yield_now();
                continue;
            }
            if symlink_dir(&outside_ancestor, &active_ancestor).is_ok() {
                racer_swaps.fetch_add(1, Ordering::Relaxed);
                thread::yield_now();
                let _ = fs::remove_file(&active_ancestor);
            }
            fs::rename(&parked_ancestor, &active_ancestor).expect("restore writable ancestor");
        }
    });

    let mut rebound_root = None;
    for _ in 0..2_000 {
        let roots = policy.get_writable_roots_with_cwd_preserving_mutable_paths(tmp.path());
        if roots.len() != 1 || roots[0].root != expected_writable_root {
            rebound_root = roots.first().map(|root| root.root.clone());
            break;
        }
        assert_ne!(roots[0].root, expected_outside_root);
        thread::yield_now();
    }
    stop.store(true, Ordering::Relaxed);
    racer.join().expect("join path racer");

    assert!(swaps.load(Ordering::Relaxed) > 0, "racer did not run");
    assert_eq!(rebound_root, None);
}

#[test]
fn legacy_workspace_write_projection_preserves_symbolic_project_root() {
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    assert_eq!(
        FileSystemSandboxPolicy::from(&policy),
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry::skip_missing_path(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some(".git".into())),
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::skip_missing_path(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some(".agents".into())),
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::skip_missing_path(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some(".codex".into())),
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::skip_missing_path(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some(".aws".into())),
                },
                FileSystemAccessMode::Read,
            ),
        ])
    );
}

#[test]
fn legacy_current_working_directory_special_path_deserializes_as_project_roots()
-> serde_json::Result<()> {
    let value = serde_json::json!({
        "kind": "current_working_directory",
    });

    let special_path = serde_json::from_value::<FileSystemSpecialPath>(value)?;
    assert_eq!(
        special_path,
        FileSystemSpecialPath::project_roots(/*subpath*/ None)
    );
    assert_eq!(
        serde_json::to_value(&special_path)?,
        serde_json::json!({
            "kind": "project_roots",
        })
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn writable_roots_skip_default_metadata_when_explicit_user_rule_exists() {
    for name in [".codex", ".aws"] {
        let cwd = TempDir::new().expect("tempdir");
        let expected_root = AbsolutePathBuf::from_absolute_path(
            cwd.path().canonicalize().expect("canonicalize cwd"),
        )
        .expect("absolute canonical root");
        let explicit_metadata = expected_root.join(name);
        fs::create_dir(&explicit_metadata).expect("create metadata directory");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: explicit_metadata.clone().into(),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        let workspace_root = writable_roots
            .iter()
            .find(|root| root.root == expected_root)
            .expect("workspace writable root");
        assert!(
            !workspace_root
                .protected_metadata_names
                .contains(&name.to_string()),
            "explicit {name} rule should remove the metadata-name protection"
        );
        assert!(
            !workspace_root
                .read_only_subpaths
                .contains(&explicit_metadata),
            "explicit {name} rule should win over the default protected carveout"
        );
        assert!(policy.can_write_local_path_with_cwd(
            explicit_metadata.join("config.toml").as_path(),
            cwd.path()
        ));
    }
}

#[test]
fn filesystem_policy_blocks_protected_metadata_path_writes_by_default() {
    let cwd = TempDir::new().expect("tempdir");
    let dot_git_config = cwd.path().join(".git").join("config");
    let dot_agents_config = cwd.path().join(".agents").join("config");
    let dot_codex_config = cwd.path().join(".codex").join("config.toml");
    let dot_aws_config = cwd.path().join(".aws").join("config");
    let root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_git_config, cwd.path()));
    assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_agents_config, cwd.path()));
    assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_codex_config, cwd.path()));
    assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_aws_config, cwd.path()));

    let writable_roots = file_system_policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(
        writable_roots[0].protected_metadata_names,
        vec![
            ".git".to_string(),
            ".agents".to_string(),
            ".codex".to_string(),
            ".aws".to_string(),
        ]
    );
    assert!(!writable_roots[0].is_path_writable(&dot_git_config));
    assert!(!writable_roots[0].is_path_writable(&dot_agents_config));
    assert!(!writable_roots[0].is_path_writable(&dot_codex_config));
    assert!(!writable_roots[0].is_path_writable(&dot_aws_config));
}

#[test]
fn legacy_workspace_write_projection_accepts_relative_cwd() {
    let relative_cwd = Path::new("workspace");
    let expected_root = AbsolutePathBuf::from_absolute_path(
        std::env::current_dir()
            .expect("current dir")
            .join(relative_cwd),
    )
    .expect("absolute root");
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let file_system_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&policy, relative_cwd);

    let mut expected_entries = vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ];
    expected_entries.extend(PROTECTED_METADATA_PATH_NAMES.iter().map(|name| {
        FileSystemSandboxEntry::skip_missing_path(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some((*name).into())),
            },
            FileSystemAccessMode::Read,
        )
    }));
    expected_entries.extend(
        default_read_only_subpaths_for_writable_root(
            &expected_root,
            /*protect_missing_dot_codex*/ true,
        )
        .into_iter()
        .map(|path| {
            FileSystemSandboxEntry::skip_missing_path(path.into(), FileSystemAccessMode::Read)
        }),
    );

    assert_eq!(
        file_system_policy,
        FileSystemSandboxPolicy::restricted(expected_entries)
    );
    assert_eq!(
        forbidden_agent_metadata_write(Path::new(".git/config"), relative_cwd, &file_system_policy,),
        Some(".git")
    );
    assert!(
        file_system_policy.can_write_local_path_with_cwd(Path::new("src/main.rs"), relative_cwd,)
    );
    assert!(
        !file_system_policy
            .can_write_local_path_with_cwd(Path::new(".codex/config.toml"), relative_cwd,)
    );
    assert!(
        !file_system_policy.can_write_local_path_with_cwd(
            Path::new(".agents/skills/example/SKILL.md"),
            relative_cwd,
        )
    );
}

#[cfg(unix)]
#[test]
fn effective_runtime_roots_preserve_symlinked_paths() {
    let cwd = TempDir::new().expect("tempdir");
    let real_root = cwd.path().join("real");
    let link_root = cwd.path().join("link");
    let blocked = real_root.join("blocked");
    let codex_dir = real_root.join(".codex");

    fs::create_dir_all(&blocked).expect("create blocked");
    fs::create_dir_all(&codex_dir).expect("create .codex");
    symlink_dir(&real_root, &link_root).expect("create symlinked root");

    let link_root =
        AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
    let link_blocked = link_root.join("blocked");
    let expected_root = link_root.clone();
    let expected_blocked = link_blocked.clone();
    let expected_codex = link_root.join(".codex");

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: link_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_blocked.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    assert_eq!(
        policy.get_unreadable_roots_with_cwd(cwd.path()),
        vec![expected_blocked.clone()]
    );

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_blocked)
    );
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_codex)
    );
}

#[cfg(unix)]
#[test]
fn project_roots_special_path_preserves_symlinked_root() {
    let cwd = TempDir::new().expect("tempdir");
    let real_root = cwd.path().join("real");
    let link_root = cwd.path().join("link");
    let blocked = real_root.join("blocked");
    let agents_dir = real_root.join(".agents");
    let codex_dir = real_root.join(".codex");

    fs::create_dir_all(&blocked).expect("create blocked");
    fs::create_dir_all(&agents_dir).expect("create .agents");
    fs::create_dir_all(&codex_dir).expect("create .codex");
    symlink_dir(&real_root, &link_root).expect("create symlinked cwd");

    let link_blocked =
        AbsolutePathBuf::from_absolute_path(link_root.join("blocked")).expect("link blocked");
    let expected_root =
        AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
    let expected_blocked = link_blocked.clone();
    let expected_agents = expected_root.join(".agents");
    let expected_codex = expected_root.join(".codex");

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_blocked.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    assert_eq!(
        policy.get_readable_roots_with_cwd(&link_root),
        vec![expected_root.clone()]
    );
    assert_eq!(
        policy.get_unreadable_roots_with_cwd(&link_root),
        vec![expected_blocked.clone()]
    );

    let writable_roots = policy.get_writable_roots_with_cwd(&link_root);
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_blocked)
    );
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_agents)
    );
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_codex)
    );
}

#[cfg(unix)]
#[test]
fn writable_roots_preserve_symlinked_protected_subpaths() {
    let cwd = TempDir::new().expect("tempdir");
    let root = cwd.path().join("root");
    let decoy = root.join("decoy-codex");
    let dot_codex = root.join(".codex");
    fs::create_dir_all(&decoy).expect("create decoy");
    symlink_dir(&decoy, &dot_codex).expect("create .codex symlink");

    let root = AbsolutePathBuf::from_absolute_path(&root).expect("absolute root");
    let expected_dot_codex = AbsolutePathBuf::from_absolute_path(
        root.as_path()
            .canonicalize()
            .expect("canonicalize root")
            .join(".codex"),
    )
    .expect("absolute .codex symlink");
    let unexpected_decoy =
        AbsolutePathBuf::from_absolute_path(decoy.canonicalize().expect("canonicalize decoy"))
            .expect("absolute canonical decoy");

    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: root.into(),
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(
        writable_roots[0].read_only_subpaths,
        vec![expected_dot_codex]
    );
    assert!(
        !writable_roots[0]
            .read_only_subpaths
            .contains(&unexpected_decoy)
    );
}

#[cfg(unix)]
#[test]
fn writable_roots_preserve_explicit_symlinked_carveouts_under_symlinked_roots() {
    let cwd = TempDir::new().expect("tempdir");
    let real_root = cwd.path().join("real");
    let link_root = cwd.path().join("link");
    let decoy = real_root.join("decoy-private");
    let linked_private = real_root.join("linked-private");
    fs::create_dir_all(&decoy).expect("create decoy");
    symlink_dir(&real_root, &link_root).expect("create symlinked root");
    symlink_dir(&decoy, &linked_private).expect("create linked-private symlink");

    let link_root =
        AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
    let link_private = link_root.join("linked-private");
    let expected_root = link_root.clone();
    let expected_linked_private = link_private.clone();
    let unexpected_decoy =
        AbsolutePathBuf::from_absolute_path(decoy.canonicalize().expect("canonicalize decoy"))
            .expect("absolute canonical decoy");

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: link_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_private.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert_eq!(
        writable_roots[0].read_only_subpaths,
        vec![expected_linked_private]
    );
    assert!(
        !writable_roots[0]
            .read_only_subpaths
            .contains(&unexpected_decoy)
    );
}

#[cfg(unix)]
#[test]
fn writable_roots_preserve_explicit_symlinked_carveouts_that_escape_root() {
    let cwd = TempDir::new().expect("tempdir");
    let real_root = cwd.path().join("real");
    let link_root = cwd.path().join("link");
    let decoy = cwd.path().join("outside-private");
    let linked_private = real_root.join("linked-private");
    fs::create_dir_all(&decoy).expect("create decoy");
    fs::create_dir_all(&real_root).expect("create real root");
    symlink_dir(&real_root, &link_root).expect("create symlinked root");
    symlink_dir(&decoy, &linked_private).expect("create linked-private symlink");

    let link_root =
        AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
    let link_private = link_root.join("linked-private");
    let expected_root = link_root.clone();
    let expected_linked_private = link_private.clone();
    let unexpected_decoy =
        AbsolutePathBuf::from_absolute_path(decoy.canonicalize().expect("canonicalize decoy"))
            .expect("absolute canonical decoy");

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: link_root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_private.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert_eq!(
        writable_roots[0].read_only_subpaths,
        vec![expected_linked_private]
    );
    assert!(
        !writable_roots[0]
            .read_only_subpaths
            .contains(&unexpected_decoy)
    );
}

#[cfg(unix)]
#[test]
fn writable_roots_preserve_explicit_symlinked_carveouts_that_alias_root() {
    let cwd = TempDir::new().expect("tempdir");
    let root = cwd.path().join("root");
    let alias = root.join("alias-root");
    fs::create_dir_all(&root).expect("create root");
    symlink_dir(&root, &alias).expect("create alias symlink");

    let root = AbsolutePathBuf::from_absolute_path(&root).expect("absolute root");
    let alias = root.join("alias-root");
    let expected_root = AbsolutePathBuf::from_absolute_path(
        root.as_path().canonicalize().expect("canonicalize root"),
    )
    .expect("absolute canonical root");
    let expected_alias = expected_root.join("alias-root");

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: alias.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert_eq!(writable_roots[0].read_only_subpaths, vec![expected_alias]);
}

#[cfg(unix)]
#[test]
fn tmpdir_special_path_preserves_symlinked_tmpdir() {
    if std::env::var_os(SYMLINKED_TMPDIR_TEST_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .env(SYMLINKED_TMPDIR_TEST_ENV, "1")
            .arg("--exact")
            .arg("permissions::tests::tmpdir_special_path_preserves_symlinked_tmpdir")
            .output()
            .expect("run tmpdir subprocess test");

        assert!(
            output.status.success(),
            "tmpdir subprocess test failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let cwd = TempDir::new().expect("tempdir");
    let real_tmpdir = cwd.path().join("real-tmpdir");
    let link_tmpdir = cwd.path().join("link-tmpdir");
    let blocked = real_tmpdir.join("blocked");
    let codex_dir = real_tmpdir.join(".codex");

    fs::create_dir_all(&blocked).expect("create blocked");
    fs::create_dir_all(&codex_dir).expect("create .codex");
    symlink_dir(&real_tmpdir, &link_tmpdir).expect("create symlinked tmpdir");

    let link_blocked =
        AbsolutePathBuf::from_absolute_path(link_tmpdir.join("blocked")).expect("link blocked");
    let expected_root =
        AbsolutePathBuf::from_absolute_path(&link_tmpdir).expect("absolute symlinked tmpdir");
    let expected_blocked = link_blocked.clone();
    let expected_codex = expected_root.join(".codex");

    unsafe {
        std::env::set_var("TMPDIR", &link_tmpdir);
    }

    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: link_blocked.into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    assert_eq!(
        policy.get_unreadable_roots_with_cwd(cwd.path()),
        vec![expected_blocked.clone()]
    );

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
    assert_eq!(writable_roots.len(), 1);
    assert_eq!(writable_roots[0].root, expected_root);
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_blocked)
    );
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_codex)
    );
}

#[test]
fn resolve_access_for_local_path_with_cwd_uses_most_specific_entry() {
    let cwd = TempDir::new().expect("tempdir");
    let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
    let docs_private = AbsolutePathBuf::resolve_path_against_base("docs/private", cwd.path());
    let docs_private_public =
        AbsolutePathBuf::resolve_path_against_base("docs/private/public", cwd.path());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: docs.clone().into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: docs_private.clone().into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: docs_private_public.clone().into(),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(cwd.path(), cwd.path()),
        FileSystemAccessMode::Write
    );
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
        FileSystemAccessMode::Read
    );
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(docs_private.as_path(), cwd.path()),
        FileSystemAccessMode::Deny
    );
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(docs_private_public.as_path(), cwd.path()),
        FileSystemAccessMode::Write
    );
}

#[test]
fn split_only_nested_carveouts_need_direct_runtime_enforcement() {
    let cwd = TempDir::new().expect("tempdir");
    let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: docs.into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);

    assert!(policy.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path(),));

    let legacy_workspace_write = legacy_runtime_file_system_policy_for_cwd(
        &SandboxPolicy::new_workspace_write_policy(),
        cwd.path(),
    );
    assert!(
        legacy_workspace_write
            .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path(),),
        "metadata-name protections must stay in the direct enforcement path even when legacy concrete read-only paths match"
    );
}

#[test]
fn legacy_projection_runtime_enforcement_ignores_entry_order() {
    let cwd = TempDir::new().expect("tempdir");
    let legacy_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let legacy_order = legacy_runtime_file_system_policy_for_cwd(&legacy_policy, cwd.path());
    let mut reordered_entries = legacy_order.entries.clone();
    reordered_entries.reverse();
    let reordered = FileSystemSandboxPolicy::restricted(reordered_entries);

    assert!(
        legacy_order.is_semantically_equivalent_to(&reordered, cwd.path()),
        "entry order should not affect filesystem semantics"
    );
    assert_eq!(
        legacy_order.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
        reordered.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
        "entry order should not affect direct-enforcement classification"
    );
}

#[test]
fn missing_symbolic_metadata_carveouts_need_direct_runtime_enforcement() {
    let cwd = TempDir::new().expect("tempdir");
    let legacy_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let profile_projection =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&legacy_policy, cwd.path());
    assert!(
        profile_projection
            .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
        "symbolic .git/.agents carveouts protect missing paths that legacy sandboxes cannot represent"
    );

    let legacy_runtime_projection =
        legacy_runtime_file_system_policy_for_cwd(&legacy_policy, cwd.path());
    assert!(
        legacy_runtime_projection
            .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
        "metadata-name protections are outside the legacy SandboxPolicy writable-root contract"
    );
}

#[test]
fn root_write_with_read_only_child_is_not_full_disk_write() {
    let cwd = TempDir::new().expect("tempdir");
    let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: docs.clone().into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);

    assert!(!policy.has_full_disk_write_access());
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
        FileSystemAccessMode::Read
    );
    assert!(policy.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path(),));
    assert!(
        policy
            .to_legacy_sandbox_policy(NetworkSandboxPolicy::Restricted, cwd.path())
            .is_err()
    );
}

#[test]
fn root_deny_does_not_materialize_as_unreadable_root() {
    let cwd = TempDir::new().expect("tempdir");
    let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
    let expected_docs = AbsolutePathBuf::from_absolute_path(
        canonicalize_preserving_symlinks(cwd.path())
            .expect("canonicalize cwd")
            .join("docs"),
    )
    .expect("canonical docs");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: docs.clone().into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
    ]);

    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
        FileSystemAccessMode::Read
    );
    assert_eq!(
        policy.get_readable_roots_with_cwd(cwd.path()),
        vec![expected_docs]
    );
    assert!(policy.get_unreadable_roots_with_cwd(cwd.path()).is_empty());
}

#[test]
fn duplicate_root_deny_prevents_full_disk_write_access() {
    let cwd = TempDir::new().expect("tempdir");
    let root = AbsolutePathBuf::from_absolute_path(cwd.path())
        .map(|cwd| absolute_root_path_for_cwd(&cwd))
        .expect("resolve filesystem root");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    assert!(!policy.has_full_disk_write_access());
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(root.as_path(), cwd.path()),
        FileSystemAccessMode::Deny
    );
}

#[test]
fn same_specificity_write_override_keeps_full_disk_write_access() {
    let cwd = TempDir::new().expect("tempdir");
    let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: docs.clone().into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: docs.clone().into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);

    assert!(policy.has_full_disk_write_access());
    assert_eq!(
        policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
        FileSystemAccessMode::Write
    );
}

#[test]
fn with_additional_readable_roots_skips_existing_effective_access() {
    let cwd = TempDir::new().expect("tempdir");
    let cwd_root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Read,
        missing_path_behavior: None,
    }]);

    let actual = policy
        .clone()
        .with_additional_readable_roots(cwd.path(), std::slice::from_ref(&cwd_root));

    assert_eq!(actual, policy);
}

#[test]
fn with_additional_writable_roots_skips_existing_effective_access() {
    let cwd = TempDir::new().expect("tempdir");
    let cwd_root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let actual = policy
        .clone()
        .with_additional_writable_roots(cwd.path(), std::slice::from_ref(&cwd_root));

    assert_eq!(actual, policy);
}

#[test]
fn with_additional_writable_roots_adds_new_root() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cwd = temp_dir.path().join("workspace");
    let extra = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("extra"))
        .expect("resolve extra root");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let actual = policy.with_additional_writable_roots(&cwd, std::slice::from_ref(&extra));

    assert_eq!(
        actual,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: extra.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ])
    );
}

#[test]
fn materialize_project_roots_with_workspace_roots_expands_exact_and_glob_entries() {
    let temp_dir = TempDir::new().expect("tempdir");
    let first = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("first"))
        .expect("resolve first root");
    let second = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("second"))
        .expect("resolve second root");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".git".into())),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: project_roots_glob_pattern(Path::new("**/*.env")),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);

    let actual =
        policy.materialize_project_roots_with_workspace_roots(&[first.clone(), second.clone()]);

    assert_eq!(
        actual,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: first.clone().into(),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: second.clone().into(),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: first.join(".git").into(),
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: second.join(".git").into(),
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: AbsolutePathBuf::resolve_path_against_base(
                        "**/*.env",
                        first.as_path(),
                    )
                    .to_string_lossy()
                    .into_owned(),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: AbsolutePathBuf::resolve_path_against_base(
                        "**/*.env",
                        second.as_path(),
                    )
                    .to_string_lossy()
                    .into_owned(),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ])
    );
}

#[test]
fn with_additional_legacy_workspace_writable_roots_protects_metadata() {
    let temp_dir = TempDir::new().expect("tempdir");
    let extra = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("extra"))
        .expect("resolve extra root");
    std::fs::create_dir_all(extra.join(".git")).expect("create .git dir");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
        },
        access: FileSystemAccessMode::Write,
        missing_path_behavior: None,
    }]);

    let actual =
        policy.with_additional_legacy_workspace_writable_roots(std::slice::from_ref(&extra));

    assert_eq!(
        actual,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: extra.clone().into()
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry::skip_missing_path(
                FileSystemPath::Path {
                    path: extra.join(".git").into()
                },
                FileSystemAccessMode::Read,
            ),
        ])
    );
}

#[test]
fn file_system_access_mode_orders_by_conflict_precedence() {
    assert!(FileSystemAccessMode::Write > FileSystemAccessMode::Read);
    assert!(FileSystemAccessMode::Deny > FileSystemAccessMode::Write);
}

#[test]
fn legacy_bridge_preserves_explicit_deny_entries() {
    let denied = AbsolutePathBuf::try_from("/tmp/private").expect("absolute path");
    let existing = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Path {
            path: denied.clone().into(),
        },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    }]);

    let rebuilt = FileSystemSandboxPolicy::from_legacy_sandbox_policy_preserving_deny_entries(
        &SandboxPolicy::new_workspace_write_policy(),
        Path::new("/tmp/workspace"),
        &existing,
    );

    assert!(
        rebuilt.entries.iter().any(|entry| {
            entry.path
                == FileSystemPath::Path {
                    path: denied.clone().into(),
                }
                && entry.access == FileSystemAccessMode::Deny
        }),
        "expected explicit deny entry to be preserved"
    );
}

#[test]
fn preserving_deny_entries_keeps_unrestricted_policy_enforceable() {
    let deny_entry = unreadable_glob_entry("/tmp/project/**/*.env".to_string());
    let mut existing = FileSystemSandboxPolicy::restricted(vec![deny_entry.clone()]);
    existing.glob_scan_max_depth = Some(2);
    let mut replacement = FileSystemSandboxPolicy::unrestricted();

    replacement.preserve_deny_read_restrictions_from(&existing);

    let mut expected = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        deny_entry,
    ]);
    expected.glob_scan_max_depth = Some(2);
    assert_eq!(replacement, expected);
}

fn deny_policy(path: &Path) -> FileSystemSandboxPolicy {
    FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Path {
            path: AbsolutePathBuf::try_from(path)
                .expect("absolute deny path")
                .into(),
        },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    }])
}

fn unreadable_glob_entry(pattern: String) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern { pattern },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    }
}

fn default_policy_with_unreadable_glob(pattern: String) -> FileSystemSandboxPolicy {
    let mut policy = FileSystemSandboxPolicy::default();
    policy.entries.push(unreadable_glob_entry(pattern));
    policy
}

fn is_read_denied(
    path: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> bool {
    ReadDenyMatcher::try_new_for_local_paths(file_system_sandbox_policy, cwd)
        .expect("valid deny-read globs")
        .is_some_and(|matcher| matcher.is_local_path_read_denied(path))
}

#[test]
fn exact_path_and_descendants_are_denied() {
    let temp = TempDir::new().expect("tempdir");
    let denied_dir = temp.path().join("denied");
    let nested = denied_dir.join("nested.txt");
    std::fs::create_dir_all(&denied_dir).expect("create denied dir");
    std::fs::write(&nested, "secret").expect("write secret");

    let policy = deny_policy(&denied_dir);
    assert!(is_read_denied(&denied_dir, &policy, temp.path()));
    assert!(is_read_denied(&nested, &policy, temp.path()));
    assert!(!is_read_denied(
        &temp.path().join("other.txt"),
        &policy,
        temp.path()
    ));
}

#[cfg(unix)]
#[test]
fn native_read_deny_matching_uses_uri_matcher_semantics() {
    let temp = TempDir::new().expect("tempdir");
    let real_dir = temp.path().join("real");
    let alias_dir = temp.path().join("alias");
    std::fs::create_dir_all(&real_dir).expect("create real dir");
    symlink_dir(&real_dir, &alias_dir).expect("symlink alias");

    let secret = real_dir.join("secret.txt");
    std::fs::write(&secret, "secret").expect("write secret");
    let alias_secret = alias_dir.join("secret.txt");

    for (denied_root, candidate) in [(&real_dir, &alias_secret), (&alias_dir, &secret)] {
        let policy = deny_policy(denied_root);
        let denied_root_uri = PathUri::from_host_native_path(denied_root).expect("deny root URI");
        let candidate_uri = PathUri::from_host_native_path(candidate).expect("candidate URI");

        assert_eq!(
            is_read_denied(candidate, &policy, temp.path()),
            candidate_uri.starts_with(&denied_root_uri)
        );
    }
}

#[test]
fn literal_patterns_and_globs_are_denied() {
    let temp = TempDir::new().expect("tempdir");
    let literal = temp.path().join("private");
    let other = temp.path().join("notes.txt");
    std::fs::create_dir_all(&literal).expect("create literal dir");
    std::fs::write(&other, "notes").expect("write notes");

    let mut policy = deny_policy(&literal);
    policy.entries.push(unreadable_glob_entry(format!(
        "{}/**/*.txt",
        temp.path().display()
    )));

    assert!(is_read_denied(&literal, &policy, temp.path()));
    assert!(is_read_denied(&other, &policy, temp.path()));
}

#[test]
fn glob_patterns_deny_matching_paths() {
    let temp = TempDir::new().expect("tempdir");
    let denied = temp.path().join("private").join("secret1.txt");
    std::fs::create_dir_all(denied.parent().expect("parent")).expect("create parent");
    std::fs::write(&denied, "secret").expect("write secret");

    let policy = default_policy_with_unreadable_glob(format!(
        "{}/private/secret?.txt",
        temp.path().display()
    ));

    assert!(is_read_denied(&denied, &policy, temp.path()));

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        let non_utf8 = denied
            .parent()
            .expect("parent")
            .join(OsStr::from_bytes(b"secret\xff.txt"));
        assert!(is_read_denied(&non_utf8, &policy, temp.path()));
    }
}

#[test]
fn glob_patterns_do_not_cross_path_separators() {
    let temp = TempDir::new().expect("tempdir");
    let matching = temp.path().join("app").join("file42.txt");
    let nested = temp.path().join("app").join("nested").join("file42.txt");
    let short = temp.path().join("app").join("file4.txt");
    let letters = temp.path().join("app").join("fileab.txt");
    std::fs::create_dir_all(nested.parent().expect("parent")).expect("create parent");
    std::fs::write(&matching, "secret").expect("write matching");
    std::fs::write(&nested, "secret").expect("write nested");
    std::fs::write(&short, "secret").expect("write short");
    std::fs::write(&letters, "secret").expect("write letters");

    let policy =
        default_policy_with_unreadable_glob(format!("{}/*/file[0-9]?.txt", temp.path().display()));

    assert!(is_read_denied(&matching, &policy, temp.path()));
    assert!(!is_read_denied(&nested, &policy, temp.path()));
    assert!(!is_read_denied(&short, &policy, temp.path()));
    assert!(!is_read_denied(&letters, &policy, temp.path()));
}

#[test]
fn globstar_patterns_deny_root_and_nested_matches() {
    let temp = TempDir::new().expect("tempdir");
    let root_env = temp.path().join(".env");
    let nested_env = temp.path().join("app").join(".env");
    let other = temp.path().join("app").join("notes.txt");
    std::fs::create_dir_all(nested_env.parent().expect("parent")).expect("create parent");
    std::fs::write(&root_env, "secret").expect("write root env");
    std::fs::write(&nested_env, "secret").expect("write nested env");
    std::fs::write(&other, "notes").expect("write notes");

    let policy = default_policy_with_unreadable_glob(format!("{}/**/*.env", temp.path().display()));

    assert!(is_read_denied(&root_env, &policy, temp.path()));
    assert!(is_read_denied(&nested_env, &policy, temp.path()));
    assert!(!is_read_denied(&other, &policy, temp.path()));
}

#[test]
fn unclosed_character_classes_match_literal_brackets() {
    let temp = TempDir::new().expect("tempdir");
    let bracket_file = temp.path().join("[");
    let other = temp.path().join("notes.txt");
    std::fs::write(&bracket_file, "secret").expect("write bracket file");
    std::fs::write(&other, "notes").expect("write notes");
    let policy = default_policy_with_unreadable_glob(format!("{}/[", temp.path().display()));

    assert!(is_read_denied(&bracket_file, &policy, temp.path()));
    assert!(!is_read_denied(&other, &policy, temp.path()));
}
