use super::*;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::path::Path;

const PATCH: &str =
    "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-orig\n+next\n";
const FILTER_COMMAND: &str = "git config codex.filterran true && git hash-object --stdin";

fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run Git");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_success(cwd: &Path, args: &[&str]) {
    let (code, _, stderr) = run(cwd, args);
    assert_eq!(code, 0, "git {args:?}: {stderr}");
}

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("tempdir");
    let root = repo.path();
    run_success(root, &["init"]);
    run_success(root, &["config", "user.email", "codex@example.com"]);
    run_success(root, &["config", "user.name", "Codex"]);
    std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
    run_success(root, &["add", "file.txt"]);
    run_success(root, &["commit", "-m", "seed"]);
    repo
}

fn configure_filter(root: &Path, driver: &str) {
    run_success(
        root,
        &["config", &format!("filter.{driver}.clean"), FILTER_COMMAND],
    );
}

fn configured_filter_ran(root: &Path) -> bool {
    run(root, &["config", "--get", "codex.filterran"]).0 == 0
}

fn request(root: &Path, revert: bool, preflight: bool) -> ApplyGitRequest {
    ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: PATCH.to_string(),
        revert,
        preflight,
    }
}

fn run_isolated_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_APPLY_FILTER_ENV_CHILD", "1")
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

#[test]
fn reverse_staging_uses_command_scoped_filter_override() {
    if std::env::var_os("CODEX_GIT_UTILS_APPLY_FILTER_ENV_CHILD").is_none() {
        run_isolated_test(
            "apply::filter_tests::reverse_staging_uses_command_scoped_filter_override",
            &[("CODEX_APPLY_GIT_CFG", OsStr::new("filter.demo.clean="))],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt filter=demo\n")
        .expect("write attributes");
    run_success(root, &["add", ".gitattributes"]);
    run_success(root, &["commit", "-m", "attributes"]);
    configure_filter(root, "demo");

    let forward = apply_git_patch(&request(
        root, /*revert*/ false, /*preflight*/ false,
    ))
    .expect("apply with disabled command-scoped filter");
    assert_eq!(forward.exit_code, 0);
    let reverse = apply_git_patch(&request(
        root, /*revert*/ true, /*preflight*/ false,
    ))
    .expect("reverse with disabled command-scoped filter");
    assert_eq!(reverse.exit_code, 0);
    assert!(!configured_filter_ran(root));
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "orig\n"
    );
}

#[test]
fn apply_treats_empty_name_filter_as_selected_only() {
    for command in ["clean", "smudge", "process"] {
        let repo = init_repo();
        let root = repo.path();
        run_success(
            root,
            &["config", &format!("filter..{command}"), FILTER_COMMAND],
        );

        std::fs::write(root.join(".gitattributes"), "file.txt filter=other\n")
            .expect("write unrelated attributes");
        let unused = apply_git_patch(&request(
            root, /*revert*/ false, /*preflight*/ true,
        ))
        .expect("allow unused empty-name filter");
        assert_eq!(unused.exit_code, 0, "{command}");
        assert!(!configured_filter_ran(root), "{command}");

        std::fs::write(root.join(".gitattributes"), "file.txt filter=\n")
            .expect("select empty-name filter");
        let selected = apply_git_patch(&request(
            root, /*revert*/ false, /*preflight*/ true,
        ))
        .expect_err("reject selected empty-name filter");
        assert_eq!(selected.kind(), io::ErrorKind::Unsupported, "{command}");
        assert!(!configured_filter_ran(root), "{command}");
    }
}

#[test]
fn apply_distinguishes_filter_sentinels_from_literal_driver_names() {
    for (driver, sentinel_attribute) in
        [("set", "filter"), ("unset", "-filter"), ("unspecified", "")]
    {
        let repo = init_repo();
        let root = repo.path();
        configure_filter(root, driver);
        run_success(
            root,
            &["config", &format!("filter.{driver}.smudge"), FILTER_COMMAND],
        );
        run_success(
            root,
            &[
                "config",
                &format!("filter.{driver}.process"),
                FILTER_COMMAND,
            ],
        );

        let attributes = if sentinel_attribute.is_empty() {
            String::new()
        } else {
            format!("file.txt {sentinel_attribute}\n")
        };
        std::fs::write(root.join(".gitattributes"), attributes).expect("write sentinel attribute");
        let sentinel = apply_git_patch(&request(
            root, /*revert*/ false, /*preflight*/ true,
        ))
        .expect("allow attribute sentinel");
        assert_eq!(sentinel.exit_code, 0, "{driver}");
        assert!(!configured_filter_ran(root), "{driver}");

        std::fs::write(
            root.join(".gitattributes"),
            format!("file.txt filter={driver}\n"),
        )
        .expect("write literal driver attribute");
        let selected = apply_git_patch(&request(
            root, /*revert*/ false, /*preflight*/ true,
        ))
        .expect_err("reject literal sentinel-named driver");
        assert_eq!(selected.kind(), io::ErrorKind::Unsupported, "{driver}");
        assert!(!configured_filter_ran(root), "{driver}");
    }
}

#[test]
fn apply_allows_effectively_empty_empty_name_filter() {
    if std::env::var_os("CODEX_GIT_UTILS_APPLY_FILTER_ENV_CHILD").is_none() {
        run_isolated_test(
            "apply::filter_tests::apply_allows_effectively_empty_empty_name_filter",
            &[(
                "CODEX_APPLY_GIT_CFG",
                OsStr::new("filter..clean=,filter..smudge=,filter..process="),
            )],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt filter=\n")
        .expect("select empty-name filter");
    for command in ["clean", "smudge", "process"] {
        run_success(
            root,
            &["config", &format!("filter..{command}"), FILTER_COMMAND],
        );
    }
    run_success(root, &["config", "filter.unused.clean", "false"]);

    let result = apply_git_patch(&request(
        root, /*revert*/ false, /*preflight*/ true,
    ))
    .expect("allow effectively empty empty-name filter");
    assert_eq!(result.exit_code, 0);
    assert!(!configured_filter_ran(root));
}
