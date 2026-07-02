use super::*;
use crate::guarded_config::config_source_authorization_count;
use crate::guarded_config::merge_attribute_read_count;
use crate::guarded_config::merge_config_read_count;
use crate::guarded_config::reset_config_source_authorization_count;
use crate::guarded_config::reset_merge_policy_counts;
use crate::safe_git::filter_policy_read_count;
use crate::safe_git::isolate_git_command_environment;
use crate::safe_git::reset_filter_policy_counts;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

#[derive(Clone, Copy, Debug)]
enum Topology {
    Delete,
    Rename,
}

fn run(cwd: &Path, args: &[&str], input: Option<&[u8]>) -> std::process::Output {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().expect("run Git");
    if let Some(input) = input {
        use std::io::Write;
        child
            .stdin
            .take()
            .expect("Git stdin")
            .write_all(input)
            .expect("write Git stdin");
    }
    child.wait_with_output().expect("read Git output")
}

fn run_success(cwd: &Path, args: &[&str]) -> Vec<u8> {
    let output = run(cwd, args, /*input*/ None);
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repository");
    let root = repo.path();
    run_success(root, &["init", "-q"]);
    run_success(root, &["config", "user.email", "codex@example.com"]);
    run_success(root, &["config", "user.name", "Codex"]);
    run_success(root, &["config", "core.autocrlf", "false"]);
    run_success(root, &["config", "core.filemode", "true"]);
    repo
}

fn read_file_normalized(path: &Path) -> String {
    std::fs::read_to_string(path)
        .expect("read file")
        .replace("\r\n", "\n")
}

fn run_success_with_apply_config(root: &Path, command_args: &[&str]) -> Vec<u8> {
    let mut configured_args = configured_git_config_parts();
    configured_args.extend(command_args.iter().map(ToString::to_string));
    let configured_args = configured_args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    run_success(root, &configured_args)
}

fn status_porcelain_with_apply_config(root: &Path) -> Vec<u8> {
    run_success_with_apply_config(root, &["status", "--porcelain"])
}

fn request(root: &Path, patch: &str) -> ApplyGitRequest {
    ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: patch.to_string(),
        revert: true,
        preflight: false,
    }
}

fn topology_fixture(topology: Topology) -> (tempfile::TempDir, String) {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("old.txt"), b"old\n").expect("base file");
    run_success(root, &["add", "old.txt"]);
    run_success(root, &["commit", "-qm", "base"]);
    match topology {
        Topology::Delete => {
            run_success(root, &["rm", "-q", "old.txt"]);
        }
        Topology::Rename => {
            run_success(root, &["mv", "old.txt", "new.txt"]);
        }
    }
    let patch = String::from_utf8(run_success(
        root,
        &["diff", "--cached", "--full-index", "--binary", "-M"],
    ))
    .expect("UTF-8 patch");
    run_success(root, &["reset", "--hard", "-q", "HEAD"]);
    (repo, patch)
}

fn index_snapshot(root: &Path) -> Vec<u8> {
    let mut snapshot = run_success(root, &["ls-files", "-v", "--stage", "-z"]);
    snapshot.extend(run_success(root, &["ls-files", "--debug"]));
    snapshot
}

#[derive(Debug, PartialEq, Eq)]
enum LeafSnapshot {
    Missing,
    File(Vec<u8>, u32),
    Symlink(std::path::PathBuf),
}

#[cfg(unix)]
fn file_mode(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode()
}

#[cfg(not(unix))]
fn file_mode(metadata: &std::fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}

fn leaf_snapshot(root: &Path, path: &str) -> LeafSnapshot {
    match std::fs::symlink_metadata(root.join(path)) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            LeafSnapshot::Symlink(std::fs::read_link(root.join(path)).expect("symlink target"))
        }
        Ok(metadata) => LeafSnapshot::File(
            std::fs::read(root.join(path)).expect("file bytes"),
            file_mode(&metadata),
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => LeafSnapshot::Missing,
        Err(error) => panic!("snapshot {path}: {error}"),
    }
}

fn assert_refused_without_mutation(root: &Path, patch: &str) {
    let before_index = index_snapshot(root);
    let before_leaves = [
        leaf_snapshot(root, "old.txt"),
        leaf_snapshot(root, "new.txt"),
    ];
    let error = apply_git_patch(&request(root, patch)).expect_err("unsafe reverse must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert_eq!(index_snapshot(root), before_index);
    assert_eq!(
        [
            leaf_snapshot(root, "old.txt"),
            leaf_snapshot(root, "new.txt")
        ],
        before_leaves
    );
}

#[test]
fn reverse_delete_and_rename_preserve_divergent_staged_blobs() {
    for (topology, endpoint) in [
        (Topology::Delete, "old.txt"),
        (Topology::Rename, "old.txt"),
        (Topology::Rename, "new.txt"),
    ] {
        let (repo, patch) = topology_fixture(topology);
        let root = repo.path();
        if matches!(topology, Topology::Rename) {
            std::fs::remove_file(root.join("old.txt")).expect("remove old endpoint");
            std::fs::write(root.join("new.txt"), b"old\n").expect("new endpoint");
        }
        std::fs::write(root.join(endpoint), b"user-staged\n").expect("divergent staged blob");
        run_success(root, &["add", endpoint]);
        if endpoint == "old.txt" {
            std::fs::remove_file(root.join(endpoint)).expect("missing old endpoint");
        } else {
            std::fs::write(root.join(endpoint), b"old\n").expect("restore worktree endpoint");
        }
        assert_refused_without_mutation(root, &patch);
    }
}

#[test]
fn reverse_rename_accepts_each_partially_staged_form() {
    for staged_endpoints in ["old", "new", "both"] {
        let (repo, patch) = topology_fixture(Topology::Rename);
        let root = repo.path();
        std::fs::remove_file(root.join("old.txt")).expect("remove old endpoint");
        std::fs::write(root.join("new.txt"), b"old\n").expect("new endpoint");
        match staged_endpoints {
            "old" => {
                run_success(root, &["add", "old.txt"]);
            }
            "new" => {
                run_success(root, &["add", "new.txt"]);
            }
            "both" => {
                run_success(root, &["add", "old.txt", "new.txt"]);
            }
            _ => unreachable!(),
        }

        let result = apply_git_patch(&request(root, &patch)).expect("safe partial rename reverse");
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(result.applied_paths, vec!["old.txt"]);
        assert_eq!(read_file_normalized(&root.join("old.txt")), "old\n");
        assert!(!root.join("new.txt").exists());
        assert!(status_porcelain_with_apply_config(root).is_empty());
    }
}

#[test]
fn reverse_staged_delete_succeeds_but_recreated_worktree_is_refused() {
    let (repo, patch) = topology_fixture(Topology::Delete);
    let root = repo.path();
    run_success_with_apply_config(root, &["rm", "-q", "old.txt"]);
    let result = apply_git_patch(&request(root, &patch)).expect("already-staged reverse delete");
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(result.applied_paths, vec!["old.txt"]);
    assert!(status_porcelain_with_apply_config(root).is_empty());

    run_success_with_apply_config(root, &["rm", "-q", "old.txt"]);
    std::fs::write(root.join("old.txt"), b"recreated\n").unwrap();
    assert_refused_without_mutation(root, &patch);
}

#[test]
fn reverse_refuses_intent_to_add_conflicts_and_index_flags() {
    for condition in [
        "ita-old",
        "ita-new",
        "conflict-old",
        "conflict-new",
        "assume",
        "skip",
    ] {
        let topology = if matches!(condition, "ita-new" | "conflict-new") {
            Topology::Rename
        } else {
            Topology::Delete
        };
        let (repo, patch) = topology_fixture(topology);
        let root = repo.path();
        if matches!(topology, Topology::Rename) {
            std::fs::remove_file(root.join("old.txt")).unwrap();
            std::fs::write(root.join("new.txt"), b"old\n").unwrap();
        }
        match condition {
            "ita-old" => {
                run_success(root, &["rm", "--cached", "-q", "old.txt"]);
                run_success(root, &["add", "-N", "old.txt"]);
                std::fs::remove_file(root.join("old.txt")).unwrap();
            }
            "ita-new" => {
                run_success(root, &["add", "-N", "new.txt"]);
            }
            "conflict-old" | "conflict-new" => {
                let path = condition.strip_prefix("conflict-").unwrap();
                let path = format!("{path}.txt");
                if path == "old.txt" {
                    std::fs::remove_file(root.join(&path)).unwrap();
                }
                let oid =
                    String::from_utf8(run_success(root, &["rev-parse", "HEAD:old.txt"])).unwrap();
                let input = format!(
                    "0 0000000000000000000000000000000000000000\t{path}\n100644 {} 1\t{path}\n100644 {} 2\t{path}\n100644 {} 3\t{path}\n",
                    oid.trim(),
                    oid.trim(),
                    oid.trim()
                );
                let output = run(
                    root,
                    &["update-index", "--index-info"],
                    Some(input.as_bytes()),
                );
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            "assume" => {
                run_success(root, &["update-index", "--assume-unchanged", "old.txt"]);
                std::fs::remove_file(root.join("old.txt")).unwrap();
            }
            "skip" => {
                run_success(root, &["update-index", "--skip-worktree", "old.txt"]);
                std::fs::remove_file(root.join("old.txt")).unwrap();
            }
            _ => unreachable!(),
        }
        assert_refused_without_mutation(root, &patch);
    }
}

#[cfg(unix)]
#[test]
fn reverse_refuses_divergent_staged_modes_and_symlinks_at_every_endpoint() {
    use std::os::unix::fs::PermissionsExt;

    for (topology, endpoint, symlink) in [
        (Topology::Delete, "old.txt", false),
        (Topology::Delete, "old.txt", true),
        (Topology::Rename, "old.txt", false),
        (Topology::Rename, "old.txt", true),
        (Topology::Rename, "new.txt", false),
        (Topology::Rename, "new.txt", true),
    ] {
        let (repo, patch) = topology_fixture(topology);
        let root = repo.path();
        if matches!(topology, Topology::Rename) {
            std::fs::remove_file(root.join("old.txt")).unwrap();
            std::fs::write(root.join("new.txt"), b"old\n").unwrap();
        }
        let endpoint_path = root.join(endpoint);
        if symlink {
            let _ = std::fs::remove_file(&endpoint_path);
            std::os::unix::fs::symlink("staged-target", &endpoint_path).unwrap();
        } else {
            std::fs::write(&endpoint_path, b"old\n").unwrap();
            let mut permissions = std::fs::metadata(&endpoint_path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&endpoint_path, permissions).unwrap();
        }
        run_success(root, &["add", endpoint]);
        std::fs::remove_file(&endpoint_path).unwrap();
        if endpoint == "new.txt" {
            std::fs::write(&endpoint_path, b"old\n").unwrap();
        }
        if !symlink {
            run_success(root, &["config", "core.filemode", "false"]);
        }
        assert_refused_without_mutation(root, &patch);
    }
}

#[cfg(unix)]
#[test]
fn reverse_mode_only_change_succeeds_when_repository_ignores_filemode() {
    use std::os::unix::fs::PermissionsExt;

    for (base_mode, forward_mode, expected_index_mode) in
        [(0o644, 0o755, "100644 "), (0o755, 0o644, "100755 ")]
    {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("mode.txt"), b"unchanged\n").unwrap();
        let mut permissions = std::fs::metadata(root.join("mode.txt"))
            .unwrap()
            .permissions();
        permissions.set_mode(base_mode);
        std::fs::set_permissions(root.join("mode.txt"), permissions).unwrap();
        run_success(root, &["add", "mode.txt"]);
        run_success(root, &["commit", "-qm", "base"]);

        let mut permissions = std::fs::metadata(root.join("mode.txt"))
            .unwrap()
            .permissions();
        permissions.set_mode(forward_mode);
        std::fs::set_permissions(root.join("mode.txt"), permissions).unwrap();
        let patch = String::from_utf8(run_success(root, &["diff", "--full-index"])).unwrap();
        assert!(patch.contains("old mode "), "{patch}");
        assert!(patch.contains("new mode "), "{patch}");

        run_success(root, &["reset", "--hard", "-q", "HEAD"]);
        run_success(root, &["config", "core.filemode", "false"]);
        let mut permissions = std::fs::metadata(root.join("mode.txt"))
            .unwrap()
            .permissions();
        permissions.set_mode(forward_mode);
        std::fs::set_permissions(root.join("mode.txt"), permissions).unwrap();

        let result =
            apply_git_patch(&request(root, &patch)).expect("reverse executable-bit change");

        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(result.applied_paths, vec!["mode.txt"]);
        let index =
            String::from_utf8(run_success(root, &["ls-files", "--stage", "mode.txt"])).unwrap();
        assert!(index.starts_with(expected_index_mode), "{index}");
        assert_eq!(
            std::fs::metadata(root.join("mode.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            base_mode
        );
        assert!(status_porcelain_with_apply_config(root).is_empty());
    }
}

#[cfg(unix)]
#[test]
fn reverse_content_only_change_preserves_executable_index_mode_when_filemode_is_ignored() {
    use std::os::unix::fs::PermissionsExt;

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("script.sh"), b"old\n").unwrap();
    let mut permissions = std::fs::metadata(root.join("script.sh"))
        .unwrap()
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(root.join("script.sh"), permissions).unwrap();
    run_success(root, &["add", "script.sh"]);
    run_success(root, &["commit", "-qm", "base"]);

    std::fs::write(root.join("script.sh"), b"new\n").unwrap();
    let patch = String::from_utf8(run_success(root, &["diff", "--full-index"])).unwrap();
    assert!(!patch.contains("old mode "), "{patch}");
    assert!(!patch.contains("new mode "), "{patch}");

    run_success(root, &["reset", "--hard", "-q", "HEAD"]);
    run_success(root, &["config", "core.filemode", "false"]);
    let mut permissions = std::fs::metadata(root.join("script.sh"))
        .unwrap()
        .permissions();
    permissions.set_mode(0o644);
    std::fs::set_permissions(root.join("script.sh"), permissions).unwrap();
    std::fs::write(root.join("script.sh"), b"new\n").unwrap();

    let result = apply_git_patch(&request(root, &patch)).expect("reverse content-only change");

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(result.applied_paths, vec!["script.sh"]);
    assert_eq!(read_file_normalized(&root.join("script.sh")), "old\n");
    let index =
        String::from_utf8(run_success(root, &["ls-files", "--stage", "script.sh"])).unwrap();
    assert!(index.starts_with("100755 "), "{index}");
    assert!(status_porcelain_with_apply_config(root).is_empty());
}

#[test]
fn reverse_refuses_ignored_untracked_rename_endpoint_without_mutation() {
    let (repo, patch) = topology_fixture(Topology::Rename);
    let root = repo.path();
    std::fs::write(root.join(".gitignore"), b"new.txt\n").unwrap();
    std::fs::rename(root.join("old.txt"), root.join("new.txt")).unwrap();
    let before_index = index_snapshot(root);
    let before_leaves = [
        leaf_snapshot(root, "old.txt"),
        leaf_snapshot(root, "new.txt"),
    ];

    let error = apply_git_patch(&request(root, &patch)).expect_err("ignored endpoint");
    assert!(error.to_string().contains("ignored"), "{error}");
    assert_eq!(index_snapshot(root), before_index);
    assert_eq!(
        [
            leaf_snapshot(root, "old.txt"),
            leaf_snapshot(root, "new.txt")
        ],
        before_leaves
    );
}

#[test]
fn reverse_handles_unborn_empty_index_and_refuses_unborn_index_state() {
    const DELETE_PATCH: &str = "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-old\n";

    let clean = init_repo();
    let result = apply_git_patch(&request(clean.path(), DELETE_PATCH)).expect("unborn reverse add");
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(result.applied_paths, vec!["old.txt"]);
    assert_eq!(read_file_normalized(&clean.path().join("old.txt")), "old\n");

    for intent_to_add in [false, true] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("old.txt"), b"staged\n").unwrap();
        run_success(
            root,
            if intent_to_add {
                &["add", "-N", "old.txt"]
            } else {
                &["add", "old.txt"]
            },
        );
        std::fs::remove_file(root.join("old.txt")).unwrap();
        assert_refused_without_mutation(root, DELETE_PATCH);
    }
}

#[test]
fn reverse_propagates_index_staging_failure_before_final_apply() {
    let (repo, patch) = topology_fixture(Topology::Delete);
    let root = repo.path();
    std::fs::remove_file(root.join("old.txt")).unwrap();
    let before = index_snapshot(root);
    std::fs::write(root.join(".git/index.lock"), b"locked").unwrap();

    let error = apply_git_patch(&request(root, &patch)).expect_err("index staging failure");
    assert!(
        error
            .to_string()
            .contains("failed to stage reverse patch paths")
    );
    std::fs::remove_file(root.join(".git/index.lock")).unwrap();
    assert_eq!(index_snapshot(root), before);
    assert!(
        !root.join("old.txt").exists(),
        "final reverse apply must not run"
    );
}

#[test]
fn direct_success_paths_are_machine_readable_in_each_orientation() {
    if std::env::var_os("CODEX_GIT_UTILS_REPORTING_LOCALE_CHILD").is_none() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        isolate_git_command_environment(&mut command);
        let output = command
            .arg("apply::reverse_apply_tests::direct_success_paths_are_machine_readable_in_each_orientation")
            .arg("--exact")
            .arg("--nocapture")
            .env("CODEX_GIT_UTILS_REPORTING_LOCALE_CHILD", "1")
            .env("LC_ALL", OsStr::new("fr_FR.UTF-8"))
            .env("RUST_TEST_THREADS", "1")
            .output()
            .expect("localized test child");
        assert!(output.status.success(), "{output:?}");
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    for path in ["first.txt", "second.txt"] {
        std::fs::write(root.join(path), b"old\n").unwrap();
    }
    run_success(root, &["add", "."]);
    run_success(root, &["commit", "-qm", "base"]);
    for path in ["first.txt", "second.txt"] {
        std::fs::write(root.join(path), b"new\n").unwrap();
    }
    run_success(root, &["add", "."]);
    let patch =
        String::from_utf8(run_success(root, &["diff", "--cached", "--full-index"])).unwrap();
    run_success(root, &["reset", "--hard", "-q", "HEAD"]);
    for revert in [false, true] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: patch.clone(),
            revert,
            preflight: false,
        })
        .unwrap();
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(result.applied_paths, vec!["first.txt", "second.txt"]);
        assert!(!result.cmd_for_log.contains("--verbose"));
    }

    run_success(root, &["mv", "first.txt", "renamed.txt"]);
    let rename = String::from_utf8(run_success(
        root,
        &["diff", "--cached", "--full-index", "-M"],
    ))
    .unwrap();
    run_success(root, &["reset", "--hard", "-q", "HEAD"]);
    for (revert, expected) in [(false, "renamed.txt"), (true, "first.txt")] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: rename.clone(),
            revert,
            preflight: false,
        })
        .unwrap();
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(result.applied_paths, vec![expected]);
    }
}

#[test]
fn reverse_three_way_with_staging_candidate_uses_one_authorization() {
    let child_marker = "CODEX_GIT_UTILS_REVERSE_THREE_WAY_TRACE_CHILD";
    if std::env::var_os(child_marker).is_none() {
        let environment = tempfile::tempdir().expect("isolated Git environment");
        let global_config = environment.path().join("global.gitconfig");
        let system_config = environment.path().join("system.gitconfig");
        std::fs::write(&global_config, "").expect("empty global config");
        std::fs::write(&system_config, "").expect("empty system config");
        let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
        isolate_git_command_environment(&mut command);
        let output = command
            .arg("apply::reverse_apply_tests::reverse_three_way_with_staging_candidate_uses_one_authorization")
            .arg("--exact")
            .arg("--nocapture")
            .env(child_marker, "1")
            .env("GIT_CONFIG_GLOBAL", &global_config)
            .env("GIT_CONFIG_SYSTEM", &system_config)
            .env("RUST_TEST_THREADS", "1")
            .output()
            .expect("run isolated reverse three-way test");
        assert!(
            output.status.success(),
            "isolated reverse three-way test failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    let base_contents = "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\nbase\n12\n13\n14\n15\n";
    let theirs_contents = "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\ntheirs\n12\n13\n14\n15\n";
    let independently_edited =
        "01\n02\n03\n04\n05\n06\n07\nEIGHT\n09\n10\ntheirs\n12\n13\n14\n15\n";
    let expected = "01\n02\n03\n04\n05\n06\n07\nEIGHT\n09\n10\nbase\n12\n13\n14\n15\n";
    std::fs::write(root.join("file.txt"), base_contents).expect("write base");
    run_success(root, &["add", "file.txt"]);
    run_success(root, &["commit", "-qm", "base"]);
    let base = String::from_utf8(run_success(root, &["rev-parse", "HEAD"])).expect("base OID");
    std::fs::write(root.join("file.txt"), theirs_contents).expect("write theirs");
    run_success(root, &["add", "file.txt"]);
    run_success(root, &["commit", "-qm", "theirs"]);
    let patch = String::from_utf8(run_success(
        root,
        &[
            "diff",
            "--full-index",
            base.trim(),
            "HEAD",
            "--",
            "file.txt",
        ],
    ))
    .expect("patch");
    // Change a context line so a plain reverse check fails, while a three-way
    // reverse can preserve the independent edit after exact staging.
    std::fs::write(root.join("file.txt"), independently_edited).expect("write independent edit");

    reset_config_source_authorization_count();
    reset_filter_policy_counts();
    reset_merge_policy_counts();
    let result = apply_git_patch(&request(root, &patch)).expect("reverse three-way apply");

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.cmd_for_log.contains("--3way"));
    assert_eq!(config_source_authorization_count(), 1);
    assert_eq!(filter_policy_read_count(), 2);
    assert_eq!(merge_config_read_count(), 1);
    assert_eq!(merge_attribute_read_count(), 1);
    assert_eq!(read_file_normalized(&root.join("file.txt")), expected);
}
