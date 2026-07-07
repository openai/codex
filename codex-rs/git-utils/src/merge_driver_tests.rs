use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::git_command::GitRunner;
use crate::git_config::GitConfigOrigin;
use crate::git_config::GitConfigScope;
use crate::guarded_config::GuardedGitConfig;
use crate::guarded_config::config_source_authorization_count;
use crate::guarded_config::merge_attribute_read_count;
use crate::guarded_config::merge_config_read_count;
use crate::guarded_config::merge_overlay_count;
use crate::guarded_config::reset_config_source_authorization_count;
use crate::guarded_config::reset_merge_policy_counts;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::path::Path;

#[test]
fn apply_allows_unused_global_merge_driver() {
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(
            &global_config,
            "[merge \"unused\"]\n\tdriver = git config --file .git/config codex.mergeran true && false\n",
        )
        .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_merge_test(
            "merge_driver::tests::apply_allows_unused_global_merge_driver",
            &[
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add file: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "file"]);
    assert_eq!(commit_code, 0, "commit file: {commit_err}");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: false,
    })
    .expect("allow unrelated global merge driver");
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt"))
            .expect("read file")
            .replace("\r\n", "\n"),
        "new\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "unused merge driver must not run");
}

#[test]
fn apply_rejects_global_merge_driver_before_three_way() {
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(
            &global_config,
            "[merge \"codex-test\"]\n\tdriver = git config --file .git/config codex.mergeran true && false\n",
        )
        .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_merge_test(
            "merge_driver::tests::apply_rejects_global_merge_driver_before_three_way",
            &[
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add base: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base: {commit_err}");
    let (base_code, base, base_err) = run(root, &["git", "rev-parse", "HEAD"]);
    assert_eq!(base_code, 0, "resolve base: {base_err}");

    std::fs::write(root.join("file.txt"), "theirs\n").expect("write theirs");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add theirs: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "theirs"]);
    assert_eq!(commit_code, 0, "commit theirs: {commit_err}");
    let (diff_code, diff, diff_err) = run(
        root,
        &[
            "git",
            "diff",
            "--full-index",
            base.trim(),
            "HEAD",
            "--",
            "file.txt",
        ],
    );
    assert_eq!(diff_code, 0, "create full-index patch: {diff_err}");

    let (checkout_code, _, checkout_err) =
        run(root, &["git", "checkout", "-b", "ours", base.trim()]);
    assert_eq!(checkout_code, 0, "checkout base: {checkout_err}");
    std::fs::write(root.join("file.txt"), "ours\n").expect("write ours");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-am", "ours"]);
    assert_eq!(commit_code, 0, "commit ours: {commit_err}");

    reset_config_source_authorization_count();
    reset_merge_policy_counts();
    let error = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect_err("reject global merge driver");
    assert_eq!(config_source_authorization_count(), 1);
    assert_eq!(merge_config_read_count(), 1);
    assert_eq!(merge_attribute_read_count(), 1);
    assert_eq!(merge_overlay_count(), 1);
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read ours"),
        "ours\n"
    );
    let (status_code, status, status_err) = status_with_apply_config(root);
    assert_eq!(status_code, 0, "status: {status_err}");
    assert!(status.is_empty(), "worktree/index changed: {status}");
}

#[test]
fn three_way_allows_trivial_selected_custom_path_when_peer_forces_fallback() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "clean.txt merge=codex-test\n")
        .expect("write attributes");
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "A\nB\nC\n").expect("write base");
    }
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    assert_eq!(run(root, &["git", "update-index", "--split-index"]).0, 0);

    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "A-patched\nB\nC\n").expect("write patch side");
    }
    let (_, diff, diff_err) = run(
        root,
        &["git", "diff", "--full-index", "--", "clean.txt", "peer.txt"],
    );
    assert!(diff_err.is_empty(), "create patch: {diff_err}");
    assert_eq!(
        run(root, &["git", "checkout", "--", "clean.txt", "peer.txt"]).0,
        0
    );
    std::fs::write(root.join("peer.txt"), "A\nB\nC-local\n").expect("write ours");
    assert_eq!(run(root, &["git", "add", "peer.txt"]).0, 0);
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "merge.codex-test.driver",
                "git config --file .git/config codex.mergeran true && false",
            ],
        )
        .0,
        0
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow base-equals-ours custom path");

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.cmd_for_log.contains("--3way"));
    assert_eq!(
        std::fs::read_to_string(root.join("clean.txt")).expect("read clean"),
        "A-patched\nB\nC\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("peer.txt")).expect("read peer"),
        "A-patched\nB\nC-local\n"
    );
    assert_ne!(
        run(root, &["git", "config", "--get", "codex.mergeran"]).0,
        0,
        "custom merge driver must not run"
    );
}

#[test]
fn three_way_allows_selected_custom_path_when_ours_equals_theirs() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "clean.txt merge=codex-test\n")
        .expect("write attributes");
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "A\nB\nC\n").expect("write base");
    }
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "A-patched\nB\nC\n").expect("write patch side");
    }
    let (_, diff, _) = run(
        root,
        &["git", "diff", "--full-index", "--", "clean.txt", "peer.txt"],
    );
    assert_eq!(run(root, &["git", "checkout", "--", "peer.txt"]).0, 0);
    std::fs::write(root.join("peer.txt"), "A\nB\nC-local\n").expect("write peer ours");
    assert_eq!(run(root, &["git", "add", "clean.txt", "peer.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow ours-equals-theirs custom path");
    assert_eq!(result.exit_code, 0, "{result:?}");
    assert_eq!(
        std::fs::read_to_string(root.join("clean.txt")).expect("read clean"),
        "A-patched\nB\nC\n"
    );
}

#[test]
fn three_way_allows_trivial_configured_namespaces_and_implicit_merge_values() {
    for (label, attributes, config) in [
        ("implicit default", "", "[merge]\n\tdefault\n"),
        (
            "unused implicit known key",
            "",
            "[merge \"unused\"]\n\tdriver\n",
        ),
        (
            "selected implicit driver",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tdriver\n",
        ),
        (
            "selected implicit unknown",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tunknown\n",
        ),
        (
            "selected name-only namespace",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tname = Demo\n",
        ),
        (
            "selected recursive-only namespace",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\trecursive = text\n",
        ),
        (
            "selected explicit empty driver",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tdriver =\n",
        ),
        (
            "selected dotted namespace",
            "file.txt merge=dotted.name\n",
            "[merge \"dotted.name\"]\n\tunknown = value\n",
        ),
        (
            "selected empty namespace",
            "file.txt merge=\n",
            "[merge \"\"]\n\tdriver = false\n",
        ),
    ] {
        let repo = init_repo();
        let root = repo.path();
        if !attributes.is_empty() {
            std::fs::write(root.join(".gitattributes"), attributes).expect("write attributes");
        }
        std::fs::write(root.join("file.txt"), "base\n").expect("write base");
        assert_eq!(run(root, &["git", "add", "."]).0, 0, "{label}");
        assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0, "{label}");
        std::fs::write(root.join("file.txt"), "patched\n").expect("write patched");
        let (_, diff, diff_err) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
        assert!(diff_err.is_empty(), "{label}: {diff_err}");
        assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0, "{label}");
        append_raw_merge_config(root, config);

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff,
            revert: false,
            preflight: false,
        })
        .unwrap_or_else(|error| panic!("{label}: trivial three-way must succeed: {error}"));
        assert_eq!(result.exit_code, 0, "{label}: {result:?}");
        assert!(result.cmd_for_log.contains("--3way"), "{label}");
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt")).expect("read file"),
            "patched\n",
            "{label}"
        );
    }
}

#[test]
fn three_way_refuses_nontrivial_configured_namespaces_and_implicit_merge_values() {
    for (label, attributes, config) in [
        ("implicit default", "", "[merge]\n\tdefault\n"),
        (
            "unused implicit known key",
            "",
            "[merge \"unused\"]\n\tdriver\n",
        ),
        (
            "selected implicit driver",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tdriver\n",
        ),
        (
            "selected implicit unknown",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tunknown\n",
        ),
        (
            "selected name-only namespace",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tname = Demo\n",
        ),
        (
            "selected recursive-only namespace",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\trecursive = text\n",
        ),
        (
            "selected explicit empty driver",
            "file.txt merge=demo\n",
            "[merge \"demo\"]\n\tdriver =\n",
        ),
        (
            "selected dotted namespace",
            "file.txt merge=dotted.name\n",
            "[merge \"dotted.name\"]\n\tunknown = value\n",
        ),
        (
            "selected empty namespace",
            "file.txt merge=\n",
            "[merge \"\"]\n\tdriver = false\n",
        ),
    ] {
        let repo = init_repo();
        let root = repo.path();
        let diff = build_three_way_fixture(root);
        if !attributes.is_empty() {
            std::fs::write(root.join(".gitattributes"), attributes).expect("write attributes");
        }
        append_raw_merge_config(root, config);
        let before_index = std::fs::read(root.join(".git/index")).expect("read index");

        let error = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff,
            revert: false,
            preflight: false,
        })
        .expect_err("nontrivial merge policy must refuse before real mutation");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported, "{label}: {error}");
        assert_eq!(
            std::fs::read(root.join(".git/index")).expect("read index after refusal"),
            before_index,
            "{label}: refusal changed the real index"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt")).expect("read ours"),
            "ours\n",
            "{label}: refusal changed the worktree"
        );
    }
}

#[cfg(unix)]
#[test]
fn three_way_allows_selected_custom_path_when_theirs_equals_base() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "clean.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("clean.txt"), "base\n").expect("write clean base");
    std::fs::write(root.join("peer.txt"), "A\nB\nC\n").expect("write peer base");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    let (_, clean_oid, _) = run(root, &["git", "rev-parse", ":clean.txt"]);
    std::fs::write(root.join("peer.txt"), "A-patched\nB\nC\n").expect("write peer patch");
    let (_, peer_patch, _) = run(root, &["git", "diff", "--full-index", "--", "peer.txt"]);
    assert_eq!(run(root, &["git", "checkout", "--", "peer.txt"]).0, 0);
    let diff = format!(
        "diff --git a/clean.txt b/clean.txt\nold mode 100644\nnew mode 100755\nindex {oid}..{oid}\n{peer_patch}",
        oid = clean_oid.trim()
    );
    std::fs::write(root.join("clean.txt"), "ours\n").expect("write clean ours");
    std::fs::write(root.join("peer.txt"), "A\nB\nC-local\n").expect("write peer ours");
    assert_eq!(run(root, &["git", "add", "clean.txt", "peer.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow theirs-equals-base custom path");
    assert_eq!(result.exit_code, 0, "{result:?}");
    assert_eq!(
        std::fs::read_to_string(root.join("clean.txt")).expect("read clean"),
        "ours\n"
    );
    let (_, mode, _) = run(root, &["git", "ls-files", "--stage", "--", "clean.txt"]);
    assert!(mode.starts_with("100755 "), "{mode:?}");
}

#[test]
fn trivial_selected_custom_path_survives_noncustom_peer_conflict() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "clean.txt merge=codex-test\n")
        .expect("write attributes");
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "base\n").expect("write base");
    }
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "theirs\n").expect("write patch side");
    }
    let (_, diff, _) = run(
        root,
        &["git", "diff", "--full-index", "--", "clean.txt", "peer.txt"],
    );
    assert_eq!(
        run(root, &["git", "checkout", "--", "clean.txt", "peer.txt"]).0,
        0
    );
    std::fs::write(root.join("peer.txt"), "ours\n").expect("write conflicting ours");
    assert_eq!(run(root, &["git", "add", "peer.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow proven custom path beside noncustom conflict");

    assert_eq!(result.exit_code, 1, "{result:?}");
    assert_eq!(
        std::fs::read_to_string(root.join("clean.txt")).expect("read clean"),
        "theirs\n"
    );
    let (_, stages, stage_err) = run(root, &["git", "ls-files", "--stage", "--", "peer.txt"]);
    assert!(stage_err.is_empty(), "read conflict stages: {stage_err}");
    assert!(stages.lines().any(|line| line.contains(" 1\tpeer.txt")));
    assert!(stages.lines().any(|line| line.contains(" 2\tpeer.txt")));
    assert!(stages.lines().any(|line| line.contains(" 3\tpeer.txt")));
}

#[test]
fn reverse_three_way_models_staging_before_proving_trivial_custom_path() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "clean.txt merge=codex-test\n")
        .expect("write attributes");
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "A\nB\nC\n").expect("write base");
    }
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    for path in ["clean.txt", "peer.txt"] {
        std::fs::write(root.join(path), "A-patched\nB\nC\n").expect("write patched");
    }
    let (_, diff, _) = run(
        root,
        &["git", "diff", "--full-index", "--", "clean.txt", "peer.txt"],
    );
    assert_eq!(run(root, &["git", "add", "clean.txt", "peer.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "patched"]).0, 0);
    std::fs::write(root.join("peer.txt"), "A-patched\nB\nC-local\n").expect("write reverse ours");
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: true,
        preflight: false,
    })
    .expect("reverse with prospective scratch staging");

    assert_eq!(result.exit_code, 0, "{result:?}");
    assert!(result.cmd_for_log.contains("--3way"));
    assert!(result.cmd_for_log.contains("-R"));
    assert_eq!(
        std::fs::read_to_string(root.join("clean.txt")).expect("read clean"),
        "A\nB\nC\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("peer.txt")).expect("read peer"),
        "A\nB\nC-local\n"
    );
}

#[test]
fn custom_merge_driver_proof_is_consumed_before_final_spawn() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    std::fs::write(root.join("file.txt"), "patched\n").expect("write patched");
    let (_, diff, _) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(run(root, &["git", "checkout", "--", "file.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let patch_dir = tempfile::tempdir().expect("patch dir");
    let patch = patch_dir.path().join("change.patch");
    std::fs::write(&patch, diff).expect("write patch");
    let patch = patch.to_str().expect("UTF-8 patch");
    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut config = GuardedGitConfig::authorize(&git, root, Vec::new()).expect("config");
    config.freeze_apply_policy().expect("freeze apply policy");
    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("apply filter policy");
    let (_, gate) = config
        .run_apply_policy_gate(/*revert*/ false, patch)
        .expect("policy gate");
    assert!(gate.status.success());
    config
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect("merge policy");
    let scratch = config.create_three_way_scratch_storage().expect("scratch");
    config
        .prove_three_way_merge_policy_safety(&scratch, /*revert*/ false, patch)
        .expect("proof");

    let first = config
        .run_three_way_apply(/*revert*/ false, patch)
        .expect("first final apply");
    assert!(first.status.success());
    let second = config
        .run_three_way_apply(/*revert*/ false, patch)
        .expect_err("proof must not be reusable");
    assert_eq!(second.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn custom_merge_driver_proof_is_consumed_before_failed_index_revalidation() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    std::fs::write(root.join("file.txt"), "patched\n").expect("write patched");
    let (_, diff, _) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(run(root, &["git", "checkout", "--", "file.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let patch_dir = tempfile::tempdir().expect("patch dir");
    let patch = patch_dir.path().join("change.patch");
    std::fs::write(&patch, diff).expect("write patch");
    let patch = patch.to_str().expect("UTF-8 patch");
    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut config = GuardedGitConfig::authorize(&git, root, Vec::new()).expect("config");
    config.freeze_apply_policy().expect("freeze apply policy");
    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("apply filter policy");
    let (_, gate) = config
        .run_apply_policy_gate(/*revert*/ false, patch)
        .expect("policy gate");
    assert!(gate.status.success());
    config
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect("merge policy");
    let scratch = config.create_three_way_scratch_storage().expect("scratch");
    config
        .prove_three_way_merge_policy_safety(&scratch, /*revert*/ false, patch)
        .expect("proof");

    std::fs::write(root.join("file.txt"), "index-raced\n").expect("write raced index entry");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);

    let first = config
        .run_three_way_apply(/*revert*/ false, patch)
        .expect_err("changed live index must invalidate proof");
    assert_eq!(first.kind(), io::ErrorKind::PermissionDenied);
    assert!(first.to_string().contains("index changed"), "{first}");

    let second = config
        .run_three_way_apply(/*revert*/ false, patch)
        .expect_err("failed revalidation must still consume proof");
    assert_eq!(second.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        second.to_string().contains("unused scratch proof"),
        "{second}"
    );
}

#[test]
fn conditional_merge_config_proof_is_consumed_before_failed_index_revalidation() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    std::fs::write(root.join("file.txt"), "patched\n").expect("write patched");
    let (_, diff, _) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(run(root, &["git", "checkout", "--", "file.txt"]).0, 0);
    append_raw_merge_config(root, "[merge \"unused\"]\n\tdriver\n");

    let patch_dir = tempfile::tempdir().expect("patch dir");
    let patch = patch_dir.path().join("change.patch");
    std::fs::write(&patch, diff).expect("write patch");
    let patch = patch.to_str().expect("UTF-8 patch");
    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut config = GuardedGitConfig::authorize(&git, root, Vec::new()).expect("config");
    config.freeze_apply_policy().expect("freeze apply policy");
    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("apply filter policy");
    let (_, gate) = config
        .run_apply_policy_gate(/*revert*/ false, patch)
        .expect("policy gate");
    assert!(gate.status.success());
    config
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect("merge policy");
    assert!(
        config
            .three_way_requires_merge_policy_proof()
            .expect("proof requirement"),
        "implicit known merge config must require proof without a selected namespace"
    );
    let scratch = config.create_three_way_scratch_storage().expect("scratch");
    config
        .prove_three_way_merge_policy_safety(&scratch, /*revert*/ false, patch)
        .expect("conditional merge-config proof");

    std::fs::write(root.join("file.txt"), "index-raced\n").expect("write raced index entry");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);

    let first = config
        .run_three_way_apply(/*revert*/ false, patch)
        .expect_err("changed live index must invalidate proof");
    assert_eq!(first.kind(), io::ErrorKind::PermissionDenied);
    assert!(first.to_string().contains("index changed"), "{first}");
    let second = config
        .run_three_way_apply(/*revert*/ false, patch)
        .expect_err("failed revalidation must still consume proof");
    assert_eq!(second.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        second.to_string().contains("unused scratch proof"),
        "{second}"
    );
}

#[test]
fn merge_policy_rejects_repeated_selected_custom_destination_records() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );

    let repeated = ["file.txt".to_string(), "file.txt".to_string()];
    let error = install_merge_policy(root, &repeated)
        .expect_err("selected custom destinations must be unique patch records");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert!(
        error.to_string().contains("repeated patch records"),
        "{error}"
    );
}

#[test]
fn rejected_custom_merge_probe_does_not_pollute_real_object_store() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    std::fs::write(root.join("file.txt"), "uncommitted-theirs\n").expect("write theirs");
    let (_, post_oid, _) = run(root, &["git", "hash-object", "file.txt"]);
    let (_, diff, _) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(run(root, &["git", "checkout", "--", "file.txt"]).0, 0);
    std::fs::write(root.join("file.txt"), "ours\n").expect("write ours");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "merge.codex-test.driver", "false"]).0,
        0
    );
    assert_ne!(
        run(root, &["git", "cat-file", "-e", post_oid.trim()]).0,
        0,
        "fixture postimage unexpectedly already exists"
    );

    let error = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect_err("all-distinct custom merge must refuse");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert_ne!(
        run(root, &["git", "cat-file", "-e", post_oid.trim()]).0,
        0,
        "scratch probe wrote its generated blob into the real object store"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read ours"),
        "ours\n"
    );
}

#[test]
fn three_way_apply_allows_unrelated_local_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add base: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base: {commit_err}");

    std::fs::write(root.join("file.txt"), "base\npatched\n").expect("write patch side");
    let (diff_code, diff, diff_err) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(diff_code, 0, "create patch: {diff_err}");
    let (restore_code, _, restore_err) = run(root, &["git", "checkout", "--", "file.txt"]);
    assert_eq!(restore_code, 0, "restore base: {restore_err}");

    std::fs::write(root.join("file.txt"), "current\nbase\n").expect("write current side");
    let (stage_code, _, stage_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(stage_code, 0, "stage current side: {stage_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.unused.driver",
            "git config --file .git/config codex.mergeran true && false",
        ],
    );
    assert_eq!(
        config_code, 0,
        "configure unused local driver: {config_err}"
    );

    reset_config_source_authorization_count();
    reset_merge_policy_counts();
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow unrelated local merge driver during three-way fallback");
    assert_eq!(config_source_authorization_count(), 1);
    assert_eq!(merge_config_read_count(), 1);
    assert_eq!(merge_attribute_read_count(), 1);
    assert_eq!(merge_overlay_count(), 1);
    assert_eq!(result.exit_code, 0, "three-way apply: {}", result.stderr);
    assert!(result.cmd_for_log.contains("--3way"));
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt"))
            .expect("read merged file")
            .replace("\r\n", "\n"),
        "current\nbase\npatched\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "unused merge driver must not run");
}

#[test]
fn isolated_three_way_config_preserves_builtin_union_default() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "top\nbase\nbottom\n").expect("write base");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add base: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base: {commit_err}");
    let (base_code, base, base_err) = run(root, &["git", "rev-parse", "HEAD"]);
    assert_eq!(base_code, 0, "resolve base: {base_err}");

    std::fs::write(root.join("file.txt"), "top\ntheirs\nbottom\n").expect("write theirs");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add theirs: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "theirs"]);
    assert_eq!(commit_code, 0, "commit theirs: {commit_err}");
    let (diff_code, diff, diff_err) = run(
        root,
        &[
            "git",
            "diff",
            "--full-index",
            base.trim(),
            "HEAD",
            "--",
            "file.txt",
        ],
    );
    assert_eq!(diff_code, 0, "create full-index patch: {diff_err}");

    let (checkout_code, _, checkout_err) =
        run(root, &["git", "checkout", "-b", "ours", base.trim()]);
    assert_eq!(checkout_code, 0, "checkout base: {checkout_err}");
    std::fs::write(root.join("file.txt"), "top\nours\nbottom\n").expect("write ours");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-am", "ours"]);
    assert_eq!(commit_code, 0, "commit ours: {commit_err}");
    let (config_code, _, config_err) = run(root, &["git", "config", "merge.default", "union"]);
    assert_eq!(config_code, 0, "configure union default: {config_err}");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("apply with builtin union default");

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    let contents = std::fs::read_to_string(root.join("file.txt"))
        .expect("read union merge")
        .replace("\r\n", "\n");
    assert!(contents.contains("ours\n"), "{contents}");
    assert!(contents.contains("theirs\n"), "{contents}");
    assert!(!contents.contains("<<<<<<<"), "{contents}");
}

#[derive(Clone, Copy, Debug)]
enum MergeAttributeSource {
    Info,
    CoreAttributesFile,
}

#[derive(Clone, Copy, Debug)]
enum SafeMergeAttribute {
    Union,
    Binary,
}

fn configure_attribute_source(
    root: &Path,
    source: MergeAttributeSource,
    storage: &tempfile::TempDir,
    contents: &str,
) {
    match source {
        MergeAttributeSource::Info => {
            std::fs::write(root.join(".git/info/attributes"), contents)
                .expect("write info attributes");
        }
        MergeAttributeSource::CoreAttributesFile => {
            let attributes = storage.path().join("attributes");
            std::fs::write(&attributes, contents).expect("write core attributes");
            let configured = run(
                root,
                &[
                    "git",
                    "config",
                    "core.attributesFile",
                    attributes.to_str().expect("UTF-8 attributes path"),
                ],
            );
            assert_eq!(configured.0, 0, "configure attributes: {}", configured.2);
        }
    }
}

#[test]
fn isolated_three_way_config_projects_safe_external_merge_attributes() {
    for source in [
        MergeAttributeSource::Info,
        MergeAttributeSource::CoreAttributesFile,
    ] {
        for attribute in [SafeMergeAttribute::Union, SafeMergeAttribute::Binary] {
            let repo = init_repo();
            let root = repo.path();
            let diff = build_three_way_fixture(root);
            let attribute_storage = tempfile::tempdir().expect("attribute storage");
            let contents = match attribute {
                SafeMergeAttribute::Union => "file.txt merge=union\n",
                SafeMergeAttribute::Binary => "file.txt -merge\n",
            };
            match source {
                MergeAttributeSource::Info => {
                    std::fs::write(root.join(".git/info/attributes"), contents)
                        .expect("write info attributes");
                }
                MergeAttributeSource::CoreAttributesFile => {
                    let attributes = attribute_storage.path().join("attributes");
                    std::fs::write(&attributes, contents).expect("write core attributes file");
                    let configured = run(
                        root,
                        &[
                            "git",
                            "config",
                            "core.attributesFile",
                            attributes.to_str().expect("UTF-8 attributes path"),
                        ],
                    );
                    assert_eq!(configured.0, 0, "configure attributes: {}", configured.2);
                }
            }

            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff,
                revert: false,
                preflight: false,
            })
            .expect("run projected built-in merge");

            assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
            let merged = std::fs::read_to_string(root.join("file.txt"))
                .expect("read merge result")
                .replace("\r\n", "\n");
            match attribute {
                SafeMergeAttribute::Union => {
                    assert_eq!(result.exit_code, 0, "{source:?}: {}", result.stderr);
                    assert!(merged.contains("ours\n"), "{source:?}: {merged}");
                    assert!(merged.contains("theirs\n"), "{source:?}: {merged}");
                    assert!(!merged.contains("<<<<<<<"), "{source:?}: {merged}");
                }
                SafeMergeAttribute::Binary => {
                    assert_ne!(result.exit_code, 0, "{source:?}: {merged}");
                    assert_eq!(merged, "ours\n", "{source:?}");
                    assert!(!merged.contains("<<<<<<<"), "{source:?}: {merged}");
                }
            }
        }
    }
}

#[test]
fn isolated_three_way_projects_external_conversion_and_marker_attributes() {
    for source in [
        MergeAttributeSource::Info,
        MergeAttributeSource::CoreAttributesFile,
    ] {
        let repo = init_repo();
        let root = repo.path();
        let diff = build_clean_three_way_addition(root, "$Id$\n");
        let storage = tempfile::tempdir().expect("attribute storage");
        configure_attribute_source(root, source, &storage, "file.txt text eol=crlf ident\n");

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff,
            revert: false,
            preflight: false,
        })
        .expect("run projected conversion apply");

        assert_eq!(result.exit_code, 0, "{source:?}: {result:#?}");
        assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
        let worktree = std::fs::read(root.join("file.txt")).expect("read converted worktree");
        assert!(
            worktree.windows(2).any(|window| window == b"\r\n"),
            "{source:?}: expected CRLF bytes: {worktree:?}"
        );
        let worktree = String::from_utf8(worktree).expect("UTF-8 converted worktree");
        assert!(worktree.contains("$Id: "), "{source:?}: {worktree:?}");
        let canonical = run(root, &["git", "show", ":file.txt"]);
        assert_eq!(canonical.0, 0, "read canonical blob: {}", canonical.2);
        assert_eq!(canonical.1, "current\nbase\n$Id$\n", "{source:?}");

        let repo = init_repo();
        let root = repo.path();
        let diff = build_three_way_fixture(root);
        let storage = tempfile::tempdir().expect("marker attribute storage");
        configure_attribute_source(root, source, &storage, "file.txt conflict-marker-size=12\n");
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff,
            revert: false,
            preflight: false,
        })
        .expect("run projected marker-size apply");
        assert_ne!(result.exit_code, 0, "{source:?}: expected conflict");
        let conflict = std::fs::read_to_string(root.join("file.txt"))
            .expect("read conflict")
            .replace("\r\n", "\n");
        assert!(
            conflict.contains("<<<<<<<<<<<< ours"),
            "{source:?}: {conflict}"
        );
        assert!(
            conflict.contains("============\n"),
            "{source:?}: {conflict}"
        );
        assert!(
            conflict.contains(">>>>>>>>>>>> theirs"),
            "{source:?}: {conflict}"
        );
    }
}

#[test]
fn isolated_three_way_filter_reset_masks_lower_executable_selection() {
    let repo = init_repo();
    let root = repo.path();
    let diff = build_clean_three_way_addition(root, "patched\n");
    std::fs::write(root.join(".gitattributes"), "file.txt filter=evil\n")
        .expect("write lower filter attribute");
    std::fs::write(root.join(".git/info/attributes"), "file.txt !filter\n")
        .expect("mask lower filter attribute");
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "filter.evil.smudge",
                "git config --file .git/config codex.filterran true && cat",
            ],
        )
        .0,
        0
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("run with masked executable filter");
    assert_eq!(result.exit_code, 0, "{result:#?}");
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt"))
            .expect("read helper-free result")
            .replace("\r\n", "\n"),
        "current\nbase\npatched\n"
    );
    assert_ne!(
        run(root, &["git", "config", "--get", "codex.filterran"]).0,
        0,
        "masked filter helper ran"
    );
}

#[test]
fn linked_worktree_projects_common_info_conversion_attributes() {
    let repo = init_repo();
    let primary = repo.path();
    std::fs::write(primary.join("seed.txt"), "seed\n").expect("write seed");
    assert_eq!(run(primary, &["git", "add", "seed.txt"]).0, 0);
    assert_eq!(run(primary, &["git", "commit", "-m", "seed"]).0, 0);
    let linked_parent = tempfile::tempdir().expect("linked worktree parent");
    let linked = linked_parent.path().join("linked");
    let added = run(
        primary,
        &[
            "git",
            "worktree",
            "add",
            "-b",
            "attribute-linked",
            linked.to_str().expect("UTF-8 linked path"),
        ],
    );
    assert_eq!(added.0, 0, "add linked worktree: {}", added.2);
    let diff = build_clean_three_way_addition(&linked, "patched\n");
    std::fs::write(
        primary.join(".git/info/attributes"),
        "file.txt text eol=crlf\n",
    )
    .expect("write common info attributes");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: linked.clone(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("run linked projected conversion apply");
    assert_eq!(result.exit_code, 0, "{result:#?}");
    let worktree = std::fs::read(linked.join("file.txt")).expect("read linked worktree");
    assert!(
        worktree.windows(2).any(|window| window == b"\r\n"),
        "expected projected CRLF bytes: {worktree:?}"
    );
    assert_eq!(
        run(&linked, &["git", "show", ":file.txt"]).1,
        "current\nbase\npatched\n"
    );
}

#[test]
fn isolated_three_way_config_preserves_direct_sha256_repository_format() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let root = repo.path();
    let initialized = run(root, &["git", "init", "-q", "--object-format=sha256"]);
    assert_eq!(
        initialized.0, 0,
        "initialize SHA-256 repo: {}",
        initialized.2
    );
    assert_eq!(
        run(root, &["git", "config", "user.email", "codex@example.com"]).0,
        0
    );
    assert_eq!(run(root, &["git", "config", "user.name", "Codex"]).0, 0);
    let diff = build_three_way_fixture(root);

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("run isolated SHA-256 three-way apply");

    assert_ne!(result.exit_code, 0, "text conflict unexpectedly merged");
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    let contents = std::fs::read_to_string(root.join("file.txt"))
        .expect("read SHA-256 conflict")
        .replace("\r\n", "\n");
    assert!(contents.contains("<<<<<<<"), "{contents}");
    let object_format = run(root, &["git", "rev-parse", "--show-object-format"]);
    assert_eq!(
        object_format.0, 0,
        "resolve object format: {}",
        object_format.2
    );
    assert_eq!(object_format.1.trim(), "sha256");
}

#[test]
fn isolated_three_way_config_reads_separate_common_directory() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("worktree");
    let common = fixture.path().join("common.git");
    let initialized = run(
        fixture.path(),
        &[
            "git",
            "init",
            "-q",
            "--separate-git-dir",
            common.to_str().expect("UTF-8 common directory"),
            root.to_str().expect("UTF-8 worktree"),
        ],
    );
    assert_eq!(
        initialized.0, 0,
        "initialize separate repo: {}",
        initialized.2
    );
    assert_eq!(
        run(&root, &["git", "config", "user.email", "codex@example.com"]).0,
        0
    );
    assert_eq!(run(&root, &["git", "config", "user.name", "Codex"]).0, 0);
    let diff = build_three_way_fixture(&root);

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.clone(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("run separate-common three-way apply");

    assert_ne!(result.exit_code, 0, "text conflict unexpectedly merged");
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    let contents = std::fs::read_to_string(root.join("file.txt"))
        .expect("read separate-common conflict")
        .replace("\r\n", "\n");
    assert!(contents.contains("<<<<<<<"), "{contents}");
}

#[test]
fn malformed_shared_repository_refuses_before_reverse_index_staging() {
    let repo = init_repo();
    let root = repo.path();
    let diff = build_three_way_fixture(root);
    let index_path = root.join(".git/index");
    let before_index = std::fs::read(&index_path).expect("read index before refusal");
    let unused_filter = run(root, &["git", "config", "filter.lfs.clean", "cat"]);
    assert_eq!(
        unused_filter.0, 0,
        "configure unused filter: {}",
        unused_filter.2
    );
    let configured = run(
        root,
        &["git", "config", "core.sharedRepository", "not-a-permission"],
    );
    assert_eq!(
        configured.0, 0,
        "configure malformed value: {}",
        configured.2
    );

    let error = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: true,
        preflight: false,
    })
    .expect_err("reject malformed shared repository policy");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{error:?}");
    assert_eq!(
        std::fs::read(index_path).expect("read index after refusal"),
        before_index,
        "reverse staging ran before shared-repository validation"
    );
}

#[cfg(unix)]
#[test]
fn isolated_three_way_config_preserves_shared_repository_permissions() {
    const TEST_NAME: &str =
        "merge_driver::tests::isolated_three_way_config_preserves_shared_repository_permissions";
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_ENV_CHILD").is_none() {
        run_isolated_merge_test(TEST_NAME, &[]);
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let repo = init_repo();
    let root = repo.path();
    let diff = build_three_way_fixture(root);
    std::fs::write(root.join(".git/info/attributes"), "file.txt merge=union\n")
        .expect("write union attributes");
    let configured = run(root, &["git", "config", "core.sharedRepository", "0660"]);
    assert_eq!(
        configured.0, 0,
        "configure shared repository: {}",
        configured.2
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("run shared-repository three-way apply");
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let index_mode = std::fs::metadata(root.join(".git/index"))
        .expect("index metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(index_mode, 0o660, "index mode {index_mode:o}");
    let stage = run(root, &["git", "ls-files", "--stage", "--", "file.txt"]);
    assert_eq!(stage.0, 0, "read merged index: {}", stage.2);
    let object_id = stage
        .1
        .split_ascii_whitespace()
        .nth(1)
        .expect("merged object id");
    let object = root
        .join(".git/objects")
        .join(&object_id[..2])
        .join(&object_id[2..]);
    let object_mode = std::fs::metadata(&object)
        .expect("new merged loose object")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(object_mode, 0o440, "object mode {object_mode:o}");
}

#[cfg(unix)]
#[test]
fn isolated_three_way_preserves_git_init_shared_all_in_main_and_linked_worktrees() {
    const TEST_NAME: &str = "merge_driver::tests::isolated_three_way_preserves_git_init_shared_all_in_main_and_linked_worktrees";
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_ENV_CHILD").is_none() {
        run_isolated_merge_test_with_umask_077(TEST_NAME);
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    fn init_shared_all_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("shared-all repo tempdir");
        let root = repo.path();
        let initialized = run(root, &["git", "init", "--shared=all"]);
        assert_eq!(
            initialized.0, 0,
            "initialize shared repo: {}",
            initialized.2
        );
        assert_eq!(
            run(root, &["git", "config", "user.email", "codex@example.com"]).0,
            0
        );
        assert_eq!(run(root, &["git", "config", "user.name", "Codex"]).0, 0);
        let configured = run(root, &["git", "config", "--get", "core.sharedRepository"]);
        assert_eq!(configured.0, 0, "read shared config: {}", configured.2);
        assert_eq!(
            configured.1.trim(),
            "2",
            "Git must serialize --shared=all as 2"
        );
        repo
    }

    fn assert_shared_all_apply_modes(root: &Path, index: &Path, objects: &Path) {
        let index_mode = std::fs::metadata(index)
            .expect("index metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(index_mode, 0o664, "index mode {index_mode:o}");

        let stage = run(root, &["git", "ls-files", "--stage", "--", "file.txt"]);
        assert_eq!(stage.0, 0, "read merged index: {}", stage.2);
        let object_id = stage
            .1
            .split_ascii_whitespace()
            .nth(1)
            .expect("merged object id");
        let object = objects.join(&object_id[..2]).join(&object_id[2..]);
        let object_mode = std::fs::metadata(&object)
            .expect("new merged loose object metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(object_mode, 0o444, "object mode {object_mode:o}");
    }

    let main_repo = init_shared_all_repo();
    let main = main_repo.path();
    let patch = build_three_way_fixture(main);
    std::fs::write(main.join(".git/info/attributes"), "file.txt merge=union\n")
        .expect("write main union attributes");
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: main.to_path_buf(),
        diff: patch,
        revert: false,
        preflight: false,
    })
    .expect("run main shared-all three-way apply");
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    assert_shared_all_apply_modes(main, &main.join(".git/index"), &main.join(".git/objects"));

    let linked_repo = init_shared_all_repo();
    let primary = linked_repo.path();
    std::fs::write(primary.join("seed.txt"), "seed\n").expect("write linked seed");
    assert_eq!(run(primary, &["git", "add", "seed.txt"]).0, 0);
    assert_eq!(run(primary, &["git", "commit", "-m", "seed"]).0, 0);
    let linked_parent = tempfile::tempdir().expect("linked worktree parent");
    let linked = linked_parent.path().join("linked");
    let added = run(
        primary,
        &[
            "git",
            "worktree",
            "add",
            "-b",
            "shared-all-linked",
            linked.to_str().expect("UTF-8 linked path"),
        ],
    );
    assert_eq!(added.0, 0, "add linked worktree: {}", added.2);
    let patch = build_three_way_fixture(&linked);
    std::fs::write(
        primary.join(".git/info/attributes"),
        "file.txt merge=union\n",
    )
    .expect("write linked union attributes");
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: linked.clone(),
        diff: patch,
        revert: false,
        preflight: false,
    })
    .expect("run linked shared-all three-way apply");
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    let linked_git_dir = run(&linked, &["git", "rev-parse", "--absolute-git-dir"]);
    assert_eq!(
        linked_git_dir.0, 0,
        "resolve linked Git directory: {}",
        linked_git_dir.2
    );
    assert_shared_all_apply_modes(
        &linked,
        &std::path::PathBuf::from(linked_git_dir.1.trim()).join("index"),
        &primary.join(".git/objects"),
    );
}

#[test]
fn isolated_three_way_add_add_conflict_preserves_markers_and_index_stages() {
    let repo = init_repo();
    let root = repo.path();
    let target = root.join("conflict.txt");
    std::fs::write(&target, "existing\n").expect("write existing file");
    assert_eq!(run(root, &["git", "add", "conflict.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "existing"]).0, 0);

    let incoming = root.join("incoming.txt");
    std::fs::write(&incoming, "incoming\n").expect("write incoming file");
    let (hash_code, object_id, hash_error) =
        run(root, &["git", "hash-object", "-w", "incoming.txt"]);
    assert_eq!(hash_code, 0, "hash incoming blob: {hash_error}");
    std::fs::remove_file(incoming).expect("remove incoming fixture");
    let object_id = object_id.trim();
    let null_object_id = "0".repeat(object_id.len());
    let patch = format!(
        "diff --git a/conflict.txt b/conflict.txt\nnew file mode 100644\nindex {null_object_id}..{object_id}\n--- /dev/null\n+++ b/conflict.txt\n@@ -0,0 +1 @@\n+incoming\n"
    );

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: patch,
        revert: false,
        preflight: false,
    })
    .expect("run isolated add/add three-way apply");

    assert_ne!(result.exit_code, 0, "add/add apply unexpectedly succeeded");
    assert!(
        result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>")
            && result.cmd_for_log.contains("--3way"),
        "unexpected final command: {}",
        result.cmd_for_log
    );
    let contents = std::fs::read_to_string(&target).expect("read conflicted file");
    assert!(
        contents.contains("<<<<<<<")
            && contents.contains("=======")
            && contents.contains(">>>>>>>"),
        "three-way add/add apply did not leave conflict markers:\nresult: {result:#?}\ncontents:\n{contents}"
    );
    let (stage_code, stages, stage_error) =
        run(root, &["git", "ls-files", "--stage", "--", "conflict.txt"]);
    assert_eq!(stage_code, 0, "read conflict stages: {stage_error}");
    let actual_stages = stages
        .lines()
        .filter_map(|line| line.split_ascii_whitespace().nth(2))
        .collect::<Vec<_>>();
    assert_eq!(actual_stages, vec!["2", "3"], "{result:#?}");
}

#[test]
fn isolated_three_way_config_preserves_core_whitespace_error_classification() {
    let repo = init_repo();
    let root = repo.path();
    let diff = build_clean_three_way_addition(root, "patched \n");
    assert_eq!(
        run(root, &["git", "config", "apply.whitespace", "error"]).0,
        0
    );
    assert_eq!(
        run(
            root,
            &["git", "config", "core.whitespace", "-trailing-space"]
        )
        .0,
        0
    );
    configure_unused_marker_driver(root);

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("run forced three-way apply with whitespace error policy");

    assert_eq!(result.exit_code, 0, "{result:?}");
    assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
    let contents = std::fs::read_to_string(root.join("file.txt"))
        .expect("read whitespace result")
        .replace("\r\n", "\n");
    assert_eq!(contents, "current\nbase\npatched \n");
    let marker = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker.0, 0, "unused merge driver ran");
}

#[test]
fn isolated_three_way_config_preserves_core_whitespace_fix_output() {
    for (classifier, expected_line) in [
        ("-indent-with-non-tab", "        patched\n"),
        ("indent-with-non-tab", "\tpatched\n"),
    ] {
        let repo = init_repo();
        let root = repo.path();
        let diff = build_clean_three_way_addition(root, "        patched\n");
        assert_eq!(
            run(root, &["git", "config", "apply.whitespace", "fix"]).0,
            0
        );
        assert_eq!(
            run(root, &["git", "config", "core.whitespace", classifier]).0,
            0
        );
        configure_unused_marker_driver(root);

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff,
            revert: false,
            preflight: false,
        })
        .expect("run forced three-way apply with whitespace fix policy");

        assert_eq!(result.exit_code, 0, "{classifier}: {result:?}");
        assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
        let contents = std::fs::read_to_string(root.join("file.txt"))
            .expect("read fixed whitespace result")
            .replace("\r\n", "\n");
        assert_eq!(contents, format!("current\nbase\n{expected_line}"));
        let marker = run(root, &["git", "config", "--get", "codex.mergeran"]);
        assert_ne!(marker.0, 0, "unused merge driver ran for {classifier}");
    }
}

#[test]
fn isolated_three_way_config_projects_external_whitespace_attributes() {
    for source in [
        MergeAttributeSource::Info,
        MergeAttributeSource::CoreAttributesFile,
    ] {
        // `warn`: an external unset must keep the isolated child from
        // re-reading a stricter lower/default source and inventing a warning.
        {
            let repo = init_repo();
            let root = repo.path();
            let diff = build_clean_three_way_addition(root, "patched \n");
            assert_eq!(
                run(root, &["git", "config", "apply.whitespace", "warn"]).0,
                0
            );
            assert_eq!(
                run(
                    root,
                    &["git", "config", "core.whitespace", "trailing-space"]
                )
                .0,
                0
            );
            let storage = tempfile::tempdir().expect("attribute storage");
            configure_attribute_source(root, source, &storage, "file.txt -whitespace\n");
            if matches!(source, MergeAttributeSource::Info) {
                std::fs::write(root.join(".gitattributes"), "file.txt whitespace\n")
                    .expect("write lower worktree attribute");
            }

            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff,
                revert: false,
                preflight: false,
            })
            .expect("run projected warn apply");

            assert_eq!(result.exit_code, 0, "{source:?}: {result:?}");
            assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
            assert!(
                !result.stderr.contains("trailing whitespace"),
                "{source:?}: {}",
                result.stderr
            );
            assert_eq!(
                std::fs::read_to_string(root.join("file.txt"))
                    .expect("read warn result")
                    .replace("\r\n", "\n"),
                "current\nbase\npatched \n"
            );
        }

        // `fix`: a string classifier must survive source removal and retain
        // its exact byte-correcting behavior in the isolated child.
        {
            let repo = init_repo();
            let root = repo.path();
            let diff = build_clean_three_way_addition(root, "        patched\n");
            assert_eq!(
                run(root, &["git", "config", "apply.whitespace", "fix"]).0,
                0
            );
            assert_eq!(
                run(
                    root,
                    &["git", "config", "core.whitespace", "-indent-with-non-tab"]
                )
                .0,
                0
            );
            let storage = tempfile::tempdir().expect("attribute storage");
            configure_attribute_source(
                root,
                source,
                &storage,
                "file.txt whitespace=indent-with-non-tab,tabwidth=8\n",
            );
            if matches!(source, MergeAttributeSource::Info) {
                std::fs::write(root.join(".gitattributes"), "file.txt -whitespace\n")
                    .expect("write lower worktree attribute");
            }

            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff,
                revert: false,
                preflight: false,
            })
            .expect("run projected fix apply");

            assert_eq!(result.exit_code, 0, "{source:?}: {result:?}");
            assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
            assert!(
                !result.cmd_for_log.contains("--whitespace=nowarn"),
                "fix must retain correction behavior: {}",
                result.cmd_for_log
            );
            assert_eq!(
                std::fs::read_to_string(root.join("file.txt"))
                    .expect("read fix result")
                    .replace("\r\n", "\n"),
                "current\nbase\n\tpatched\n"
            );
        }

        // `error`: the real-policy gate is authoritative, and the isolated
        // final child cannot reclassify a gate-approved patch after staging.
        {
            let repo = init_repo();
            let root = repo.path();
            let diff = build_clean_three_way_addition(root, "patched \n");
            assert_eq!(
                run(root, &["git", "config", "apply.whitespace", "error"]).0,
                0
            );
            let storage = tempfile::tempdir().expect("attribute storage");
            configure_attribute_source(root, source, &storage, "file.txt -whitespace\n");
            if matches!(source, MergeAttributeSource::Info) {
                std::fs::write(root.join(".gitattributes"), "file.txt whitespace\n")
                    .expect("write lower worktree attribute");
            }

            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff,
                revert: false,
                preflight: false,
            })
            .expect("run projected fatal-mode apply");

            assert_eq!(result.exit_code, 0, "{source:?}: {result:?}");
            assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
            assert!(result.cmd_for_log.contains("--whitespace=nowarn"));
            assert_eq!(
                std::fs::read_to_string(root.join("file.txt"))
                    .expect("read error result")
                    .replace("\r\n", "\n"),
                "current\nbase\npatched \n"
            );
        }
    }
}

#[test]
fn apply_allows_clean_patch_with_selected_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.codex-test.driver",
            "git config --file .git/config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");

    reset_merge_policy_counts();
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: false,
    })
    .expect("allow clean patch with selected merge driver");

    assert_eq!(result.exit_code, 0);
    assert_eq!(merge_config_read_count(), 0);
    assert_eq!(merge_attribute_read_count(), 0);
    assert_eq!(merge_overlay_count(), 0);
    assert!(!result.cmd_for_log.contains("--3way"));
    let contents = std::fs::read_to_string(root.join("file.txt")).expect("read file");
    assert!(
        matches!(contents.as_str(), "new\n" | "new\r\n"),
        "expected the patched contents with a platform line ending, got {contents:?}"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    let (status_code, status, status_err) = status_with_apply_config(root);
    assert_eq!(status_code, 0, "status: {status_err}");
    assert_eq!(
        status.trim(),
        "M  file.txt",
        "patch should update the index"
    );
}

#[test]
fn reverse_apply_allows_clean_patch_with_selected_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.codex-test.driver",
            "git config --file .git/config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");
    let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let patch_dir = tempfile::tempdir().expect("patch tempdir");
    let patch = patch_dir.path().join("change.diff");
    std::fs::write(&patch, diff).expect("write patch");
    let (apply_code, _, apply_err) = run(
        root,
        &[
            "git",
            "apply",
            "--index",
            patch.to_str().expect("patch path"),
        ],
    );
    assert_eq!(apply_code, 0, "prepare forward apply: {apply_err}");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: diff.to_string(),
        revert: true,
        preflight: false,
    })
    .expect("allow clean reverse patch with selected merge driver");

    assert_eq!(result.exit_code, 0);
    assert!(!result.cmd_for_log.contains("--3way"));
    let contents = std::fs::read_to_string(root.join("file.txt")).expect("read file");
    assert!(
        matches!(contents.as_str(), "old\n" | "old\r\n"),
        "expected the reverted contents with a platform line ending, got {contents:?}"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    let (status_code, status, status_err) = status_with_apply_config(root);
    assert_eq!(status_code, 0, "status: {status_err}");
    assert!(
        status.is_empty(),
        "reverse apply should restore HEAD: {status}"
    );
}

#[test]
fn preflight_does_not_probe_or_run_selected_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.codex-test.driver",
            "git config --file .git/config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");

    reset_merge_policy_counts();
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: true,
    })
    .expect("preflight must not probe merge drivers");
    assert_eq!(result.exit_code, 0);
    assert_eq!(merge_config_read_count(), 0);
    assert_eq!(merge_attribute_read_count(), 0);
    assert_eq!(merge_overlay_count(), 0);
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "old\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "preflight must not run merge driver");
}

#[test]
fn merge_override_rejects_process_temp_directory_inside_worktree() {
    let root_name = "CODEX_GIT_UTILS_MERGE_WORKTREE_TMP_ROOT";
    if let Some(root) = std::env::var_os(root_name) {
        let root = std::path::PathBuf::from(root);
        let patch = std::fs::read_to_string(root.join("fixture.patch")).expect("read patch");
        let before_tree = run(&root, &["git", "write-tree"]).1;
        let before_contents = std::fs::read_to_string(root.join("file.txt")).expect("read ours");
        let error = apply_git_patch(&ApplyGitRequest {
            cwd: root.clone(),
            diff: patch,
            revert: false,
            preflight: false,
        })
        .expect_err("reject worktree-owned merge override");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(run(&root, &["git", "write-tree"]).1, before_tree);
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt")).expect("read after"),
            before_contents
        );
        let (marker_code, _, _) = run(&root, &["git", "config", "--get", "codex.mergeran"]);
        assert_ne!(marker_code, 0, "merge driver must not run");
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    let patch = build_three_way_fixture(root);
    std::fs::write(root.join("fixture.patch"), patch).expect("write fixture patch");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.unused.driver",
            "git config --file .git/config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");
    let worktree_temp = root.join("process-temp");
    std::fs::create_dir(&worktree_temp).expect("worktree temp directory");
    run_isolated_merge_test(
        "merge_driver::tests::merge_override_rejects_process_temp_directory_inside_worktree",
        &[
            (root_name, root.as_os_str()),
            ("TMPDIR", worktree_temp.as_os_str()),
            #[cfg(windows)]
            ("TEMP", worktree_temp.as_os_str()),
            #[cfg(windows)]
            ("TMP", worktree_temp.as_os_str()),
        ],
    );
}

#[test]
fn empty_name_driver_follows_effective_git_selection() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "base\n").expect("write file");
    std::fs::write(root.join(".gitattributes"), "file.txt merge=other\n")
        .expect("write attributes");
    let (config_code, _, config_err) = run(
        root,
        &["git", "config", "merge..driver", "empty-name helper"],
    );
    assert_eq!(config_code, 0, "configure empty-name driver: {config_err}");
    let paths = ["file.txt".to_string()];

    install_merge_policy(root, &paths).expect("allow unused empty-name driver");

    std::fs::write(root.join(".gitattributes"), "file.txt merge=\n")
        .expect("select empty-name driver");
    install_merge_policy(root, &paths).expect("quarantine selected empty-name driver");

    std::fs::write(root.join(".gitattributes"), "").expect("clear attributes");
    let (default_code, _, default_err) = run(root, &["git", "config", "merge.default", ""]);
    assert_eq!(
        default_code, 0,
        "configure empty merge.default: {default_err}"
    );
    install_merge_policy(root, &paths)
        .expect("quarantine empty-name driver selected by empty default");

    let (empty_code, _, empty_err) = run(root, &["git", "config", "merge..driver", ""]);
    assert_eq!(empty_code, 0, "empty effective driver: {empty_err}");
    install_merge_policy(root, &paths)
        .expect("quarantine selected empty command through empty default");
    std::fs::write(root.join(".gitattributes"), "file.txt merge=\n")
        .expect("select empty-name driver");
    install_merge_policy(root, &paths).expect("quarantine explicitly selected empty command");
}

#[test]
fn selected_driver_policy_allows_unused_global_and_rejects_selected() {
    let mut entries = config_entries([
        ("merge.unused.driver", "true"),
        ("merge.selected.driver", "helper %A %B"),
        ("merge..driver", "empty-name helper"),
        ("merge.default", ""),
    ]);
    let unused = BTreeMap::from([("file.txt".to_string(), "other".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &unused).expect("unused selection"),
        None
    );

    let selected = BTreeMap::from([("file.txt".to_string(), "selected".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &selected).expect("selected driver"),
        Some(("selected".to_string(), "file.txt".to_string()))
    );

    let selected_empty_name = BTreeMap::from([("file.txt".to_string(), String::new())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_empty_name)
            .expect("selected empty-name driver"),
        Some((String::new(), "file.txt".to_string()))
    );

    let selected_by_empty_default =
        BTreeMap::from([("file.txt".to_string(), "unspecified".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_by_empty_default)
            .expect("empty default selection"),
        Some((String::new(), "file.txt".to_string()))
    );

    entries
        .get_mut("merge..driver")
        .expect("empty-name driver")
        .value
        .clear();
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_empty_name).expect("empty selected command"),
        Some((String::new(), "file.txt".to_string()))
    );
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_by_empty_default)
            .expect("empty defaulted command"),
        Some((String::new(), "file.txt".to_string()))
    );
}

#[test]
fn selected_driver_policy_treats_every_configured_namespace_as_custom() {
    let entries = config_entries([
        ("merge.named.name", "display name"),
        ("merge.recursive.recursive", "text"),
        ("merge.unknown.unrecognized", "value"),
        ("merge.empty.driver", ""),
        ("merge.dotted.name.recursive", "binary"),
        ("merge..name", "empty subsection"),
        ("merge.text.name", "shadows builtin"),
    ]);
    for name in [
        "named",
        "recursive",
        "unknown",
        "empty",
        "dotted.name",
        "",
        "text",
    ] {
        let attributes = BTreeMap::from([("file.txt".to_string(), name.to_string())]);
        assert_eq!(
            untrusted_driver_selection(&entries, &attributes).expect("namespace selection"),
            Some((name.to_string(), "file.txt".to_string())),
            "{name:?}"
        );
    }

    let missing = BTreeMap::from([("file.txt".to_string(), "missing".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &missing).expect("missing namespace"),
        None
    );
}

#[test]
fn selected_driver_policy_handles_default_and_sentinel_ambiguity() {
    let mut entries = config_entries([
        ("merge.default", "defaulted"),
        ("merge.defaulted.driver", "helper"),
        ("merge.set.driver", "set helper"),
        ("merge.unset.driver", "unset helper"),
        ("merge.unspecified.driver", "unspecified helper"),
    ]);
    for value in ["set", "unset", "unspecified"] {
        let attributes = BTreeMap::from([("file.txt".to_string(), value.to_string())]);
        assert!(
            untrusted_driver_selection(&entries, &attributes)
                .expect("sentinel selection")
                .is_some(),
            "{value}"
        );
    }

    entries
        .get_mut("merge.unspecified.driver")
        .expect("unspecified driver")
        .value
        .clear();
    let attributes = BTreeMap::from([("file.txt".to_string(), "unspecified".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &attributes).expect("merge.default selection"),
        Some(("defaulted".to_string(), "file.txt".to_string()))
    );
}

#[test]
fn selected_driver_policy_ignores_scope_until_driver_is_selected() {
    for scope in [
        GitConfigScope::System,
        GitConfigScope::Global,
        GitConfigScope::Local,
        GitConfigScope::Worktree,
        GitConfigScope::Command,
    ] {
        let mut entries = config_entries([("merge.scoped.driver", "helper")]);
        entries
            .get_mut("merge.scoped.driver")
            .expect("scoped driver")
            .scope = scope;
        let attributes = BTreeMap::from([("file.txt".to_string(), "other".to_string())]);
        assert_eq!(
            untrusted_driver_selection(&entries, &attributes).expect("unused scoped driver"),
            None,
            "{scope:?}"
        );
        let attributes = BTreeMap::from([("file.txt".to_string(), "scoped".to_string())]);
        assert_eq!(
            untrusted_driver_selection(&entries, &attributes).expect("selected scoped driver"),
            Some(("scoped".to_string(), "file.txt".to_string())),
            "{scope:?}"
        );
    }
}

#[test]
fn merge_attribute_parser_is_strict() {
    let paths = vec!["a.txt".to_string(), "b.txt".to_string()];
    let parsed = parse_merge_attributes(b"a.txt\0merge\0unspecified\0b.txt\0merge\0\0", &paths)
        .expect("parse attributes");
    assert_eq!(parsed.get("a.txt").map(String::as_str), Some("unspecified"));
    assert_eq!(parsed.get("b.txt").map(String::as_str), Some(""));

    for malformed in [
        b"a.txt\0merge\0unspecified".as_slice(),
        b"a.txt\0filter\0unspecified\0b.txt\0merge\0custom\0".as_slice(),
        b"a.txt\0merge\0unspecified\0".as_slice(),
        b"a.txt\0merge\0unspecified\0a.txt\0merge\0custom\0".as_slice(),
    ] {
        assert!(
            parse_merge_attributes(malformed, &paths).is_err(),
            "{malformed:?}"
        );
    }
}

fn append_raw_merge_config(root: &Path, config: &str) {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(root.join(".git/config"))
        .expect("open repository config");
    write!(file, "\n{config}").expect("append merge config");
}

fn config_entries<const N: usize>(values: [(&str, &str); N]) -> BTreeMap<String, GitConfigEntry> {
    values
        .into_iter()
        .map(|(key, value)| {
            (
                key.to_string(),
                GitConfigEntry {
                    scope: GitConfigScope::Global,
                    origin: GitConfigOrigin::File("/tmp/global.gitconfig".into()),
                    key: key.to_string(),
                    value: value.to_string(),
                },
            )
        })
        .collect()
}

fn install_merge_policy(root: &Path, paths: &[String]) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(root)?;
    let mut config = GuardedGitConfig::authorize(&git, root, Vec::new())?;
    config.authorize_filter_paths(paths)?;
    config.install_three_way_merge_policy(paths)
}

fn build_three_way_fixture(root: &Path) -> String {
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    let base = run(root, &["git", "rev-parse", "HEAD"]).1;
    std::fs::write(root.join("file.txt"), "theirs\n").expect("write theirs");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "theirs"]).0, 0);
    let patch = run(
        root,
        &[
            "git",
            "diff",
            "--full-index",
            base.trim(),
            "HEAD",
            "--",
            "file.txt",
        ],
    );
    assert_eq!(patch.0, 0, "create patch: {}", patch.2);
    assert_eq!(
        run(root, &["git", "checkout", "-b", "ours", base.trim()]).0,
        0
    );
    std::fs::write(root.join("file.txt"), "ours\n").expect("write ours");
    assert_eq!(run(root, &["git", "commit", "-am", "ours"]).0, 0);
    patch.1
}

fn build_clean_three_way_addition(root: &Path, added_line: &str) -> String {
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    std::fs::write(root.join("file.txt"), format!("base\n{added_line}")).expect("write patch side");
    let patch = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(patch.0, 0, "create patch: {}", patch.2);
    assert_eq!(run(root, &["git", "checkout", "--", "file.txt"]).0, 0);
    std::fs::write(root.join("file.txt"), "current\nbase\n").expect("write current side");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    patch.1
}

fn configure_unused_marker_driver(root: &Path) {
    let configured = run(
        root,
        &[
            "git",
            "config",
            "merge.unused.driver",
            "git config --file .git/config codex.mergeran true && false",
        ],
    );
    assert_eq!(configured.0, 0, "configure unused driver: {}", configured.2);
}

fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut command = std::process::Command::new(args[0]);
    isolate_git_command_environment(&mut command);
    let output = command
        .args(&args[1..])
        .current_dir(cwd)
        .output()
        .expect("run command");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn status_with_apply_config(cwd: &Path) -> (i32, String, String) {
    let mut args = vec!["git".to_string()];
    args.extend(crate::apply::configured_git_config_parts());
    args.extend(["status".to_string(), "--porcelain".to_string()]);
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run(cwd, &args)
}

fn run_isolated_merge_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_MERGE_ENV_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn run_isolated_merge_test_with_umask_077(test_name: &str) {
    let mut command = std::process::Command::new("/bin/sh");
    isolate_git_command_environment(&mut command);
    command
        .args(["-c", "umask 077; exec \"$@\"", "sh"])
        .arg(std::env::current_exe().expect("test binary"))
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_MERGE_ENV_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
    let output = command.output().expect("run umask-isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let root = repo.path();
    let _ = run(root, &["git", "init"]);
    let _ = run(root, &["git", "config", "user.email", "codex@example.com"]);
    let _ = run(root, &["git", "config", "user.name", "Codex"]);
    let _ = run(root, &["git", "config", "core.autocrlf", "false"]);
    repo
}
