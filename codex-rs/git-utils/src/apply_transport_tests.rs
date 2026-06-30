use super::ApplyGitRequest;
use super::apply_git_patch;
use pretty_assertions::assert_eq;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::process::Output;

#[test]
fn three_way_apply_does_not_lazy_fetch_promisor_objects() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let source = temp_dir.path().join("source");
    let partial = temp_dir.path().join("partial");
    fs::create_dir(&source).expect("create source repository");
    run_git(&source, &["init", "-q", "--initial-branch=main"]);
    run_git(&source, &["config", "uploadpack.allowFilter", "true"]);

    fs::write(source.join("data.txt"), "one\nbase\nthree\n").expect("write base blob");
    run_git(&source, &["add", "data.txt"]);
    commit_all(&source, "base");
    let base_blob = run_git_stdout(&source, &["rev-parse", "HEAD:data.txt"]);

    fs::write(source.join("data.txt"), "one\nbase\npatched\n").expect("write patch blob");
    let patch = String::from_utf8(run_git(&source, &["diff", "--full-index", "--binary"]).stdout)
        .expect("patch should be UTF-8");
    run_git(&source, &["checkout", "-q", "--", "data.txt"]);

    fs::write(source.join("data.txt"), "current\nbase\nthree\n").expect("write head blob");
    commit_all(&source, "head");

    let complete_result = apply_git_patch(&ApplyGitRequest {
        cwd: source.clone(),
        diff: patch.clone(),
        revert: false,
        preflight: false,
    })
    .expect("apply patch in complete clone");
    assert_eq!(
        (
            complete_result.exit_code,
            fs::read_to_string(source.join("data.txt")).expect("read complete result"),
        ),
        (0, "current\nbase\npatched\n".to_string()),
        "three-way apply should retain normal behavior when every object is local; stderr: {}",
        complete_result.stderr,
    );
    run_git(&source, &["reset", "-q", "--hard", "HEAD"]);

    let source_url = format!("file://{}", source.display());
    run_git(
        temp_dir.path(),
        &[
            "-c",
            "protocol.file.allow=always",
            "clone",
            "-q",
            "--no-local",
            "--filter=blob:none",
            "--no-checkout",
            &source_url,
            partial.to_str().expect("partial path"),
        ],
    );
    run_git(&partial, &["checkout", "-q", "main"]);
    let missing = run_git_stdout(
        &partial,
        &["rev-list", "--objects", "--all", "--missing=print"],
    );
    assert!(
        missing.lines().any(|line| line == format!("?{base_blob}")),
        "expected patch base blob {base_blob} to remain missing:\n{missing}",
    );

    let helper = temp_dir.path().join("transport-helper.sh");
    fs::write(&helper, "#!/bin/sh\nprintf ran >\"$0.ran\"\nexit 1\n")
        .expect("write transport helper");
    let mut permissions = fs::metadata(&helper)
        .expect("read transport helper metadata")
        .permissions();
    permissions.set_mode(/*mode*/ 0o755);
    fs::set_permissions(&helper, permissions).expect("make transport helper executable");
    let helper_url = format!("ext::{}", helper.display());
    run_git(&partial, &["config", "remote.origin.url", &helper_url]);
    run_git(&partial, &["config", "protocol.ext.allow", "always"]);

    let patch_path = temp_dir.path().join("change.patch");
    fs::write(&patch_path, &patch).expect("write attack reproduction patch");
    let unprotected = Command::new("git")
        .args(["apply", "--3way", patch_path.to_str().expect("patch path")])
        .env_remove("GIT_ALLOW_PROTOCOL")
        .env_remove("GIT_NO_LAZY_FETCH")
        .current_dir(&partial)
        .output()
        .expect("run unprotected Git apply");
    let helper_marker = helper.with_extension("sh.ran");
    assert_eq!(
        (unprotected.status.success(), helper_marker.exists()),
        (false, true),
        "unprotected three-way apply should reach the selected promisor transport",
    );
    fs::remove_file(&helper_marker).expect("remove reproduction marker");
    run_git(&partial, &["reset", "-q", "--hard", "HEAD"]);

    let partial_result = apply_git_patch(&ApplyGitRequest {
        cwd: partial,
        diff: patch,
        revert: false,
        preflight: false,
    })
    .expect("run fail-closed apply");

    assert_eq!(
        (partial_result.exit_code == 0, helper_marker.exists(),),
        (false, false),
        "local-only apply must fail without invoking the promisor transport",
    );
}

fn run_git(cwd: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|error| panic!("run git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn run_git_stdout(cwd: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&run_git(cwd, args).stdout)
        .trim()
        .to_string()
}

fn commit_all(cwd: &Path, message: &str) {
    run_git(
        cwd,
        &[
            "-c",
            "user.name=Codex Test",
            "-c",
            "user.email=codex@example.com",
            "commit",
            "-qam",
            message,
        ],
    );
}
