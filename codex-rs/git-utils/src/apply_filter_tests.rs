use super::*;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::fs::File;
use std::fs::FileTimes;
use std::path::Path;
use std::thread;
#[cfg(unix)]
use std::time::Duration;
use std::time::UNIX_EPOCH;

const PATCH: &str =
    "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-orig\n+next\n";
const FILTER_COMMAND: &str = "git config codex.filterran true && git hash-object --stdin";
const PROCESS_FILTER_COMMAND: &str =
    "git config codex.filterran true && git rev-parse --verify refs/codex-filter-must-not-run";
const COMMAND_SCOPED_FILTER_CONFIG: &str =
    "filter.demo.clean=git config codex.filterran true && git hash-object --stdin";

#[derive(Clone, Copy, Debug)]
enum PatchKind {
    Direct,
    ThreeWay,
}

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
    run_success(root, &["config", "core.autocrlf", "false"]);
    std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
    run_success(root, &["add", "file.txt"]);
    run_success(root, &["commit", "-m", "seed"]);
    repo
}

fn configure_filter(root: &Path, driver: &str) {
    configure_filter_command(root, driver, "clean", FILTER_COMMAND);
}

fn configure_filter_command(root: &Path, driver: &str, name: &str, command: &str) {
    run_success(
        root,
        &["config", &format!("filter.{driver}.{name}"), command],
    );
}

fn make_index_racy(root: &Path) {
    let index = File::options()
        .write(true)
        .open(root.join(".git/index"))
        .expect("open index");
    index
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH))
        .expect("make index entries racy");
}

fn build_patch(root: &Path, kind: PatchKind, paths: &[&str]) -> String {
    if matches!(kind, PatchKind::Direct) && paths == ["file.txt"] {
        return PATCH.to_string();
    }
    std::fs::write(root.join("file.txt"), "next\n").expect("write target postimage");
    let mut args = vec!["diff"];
    if matches!(kind, PatchKind::ThreeWay) {
        args.extend(["--full-index", "--binary"]);
    }
    args.push("--");
    args.extend_from_slice(paths);
    let (code, stdout, stderr) = run(root, &args);
    assert_eq!(code, 0, "generate patch: {stderr}");
    let mut checkout = vec!["checkout", "-q", "--"];
    checkout.extend_from_slice(paths);
    run_success(root, &checkout);
    stdout
}

fn index_contents(root: &Path, path: &str) -> String {
    let (code, stdout, stderr) = run(root, &["show", &format!(":{path}")]);
    assert_eq!(code, 0, "read index {path}: {stderr}");
    stdout
}

fn setup_racy_offpath_filter(
    driver: &str,
    name: &str,
    command: &str,
    kind: PatchKind,
) -> (tempfile::TempDir, String, String) {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("outside.txt"), "outside\n").expect("write off-path file");
    std::fs::write(
        root.join(".gitattributes"),
        format!("outside.txt filter={driver}\n"),
    )
    .expect("write off-path attributes");
    run_success(root, &["add", ".gitattributes", "outside.txt"]);
    run_success(root, &["commit", "-m", "off-path filter target"]);
    let patch = build_patch(root, kind, &["file.txt"]);
    configure_filter_command(root, driver, name, command);
    run_success(
        root,
        &["config", &format!("filter.{driver}.required"), "true"],
    );
    let outside_index = index_contents(root, "outside.txt");
    make_index_racy(root);
    (repo, patch, outside_index)
}

fn assert_guarded_round_trip(
    repo: &tempfile::TempDir,
    patch: String,
    outside_index: &str,
    context: &str,
) {
    let root = repo.path();
    let forward = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: patch.clone(),
        revert: false,
        preflight: false,
    })
    .expect("guard forward apply");
    assert_eq!(forward.exit_code, 0, "{context}: {}", forward.stderr);
    assert!(!configured_filter_ran(root), "{context}: forward helper");
    assert_eq!(index_contents(root, "file.txt"), "next\n", "{context}");
    assert_eq!(
        index_contents(root, "outside.txt"),
        outside_index,
        "{context}"
    );

    make_index_racy(root);
    let reverse = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: patch,
        revert: true,
        preflight: false,
    })
    .expect("guard reverse apply");
    assert_eq!(reverse.exit_code, 0, "{context}: {}", reverse.stderr);
    assert!(!configured_filter_ran(root), "{context}: reverse helper");
    assert_eq!(index_contents(root, "file.txt"), "orig\n", "{context}");
    assert_eq!(
        index_contents(root, "outside.txt"),
        outside_index,
        "{context}"
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

#[cfg(unix)]
#[test]
fn apply_rejects_worktree_primary_config_fifo_before_any_git_launch() {
    use std::io::Write as _;

    const TEST_NAME: &str =
        "apply::filter_tests::apply_rejects_worktree_primary_config_fifo_before_any_git_launch";
    if std::env::var_os("CODEX_GIT_UTILS_APPLY_FILTER_ENV_CHILD").is_none() {
        let repo = init_repo();
        let primary_config = repo.path().join("worktree-global.fifo");
        let status = std::process::Command::new("mkfifo")
            .arg(&primary_config)
            .status()
            .expect("run mkfifo");
        assert!(status.success(), "mkfifo failed: {status}");
        let trace = tempfile::NamedTempFile::new().expect("trace file");
        run_isolated_test(
            TEST_NAME,
            &[
                ("CODEX_APPLY_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("GIT_CONFIG_GLOBAL", primary_config.as_os_str()),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                ("GIT_TRACE2_EVENT", trace.path().as_os_str()),
            ],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_APPLY_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    let trace = PathBuf::from(std::env::var_os("GIT_TRACE2_EVENT").expect("trace path"));
    let primary_config =
        PathBuf::from(std::env::var_os("GIT_CONFIG_GLOBAL").expect("worktree primary config FIFO"));
    // Keep the FIFO readable and prefill a valid config so a regression starts
    // and finishes instead of hanging the test process while opening it.
    let mut fifo_guard = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(primary_config)
        .expect("open worktree primary config FIFO");
    fifo_guard
        .write_all(b"[user]\n\tname = worktree-controlled\n")
        .expect("prefill worktree primary config FIFO");
    let release_fifo = thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        drop(fifo_guard);
    });
    let error = apply_git_patch(&request(
        &root, /*revert*/ false, /*preflight*/ true,
    ))
    .expect_err("reject worktree-controlled primary config");
    release_fifo.join().expect("release config FIFO");

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("worktree-controlled"), "{error}");
    assert_eq!(
        std::fs::read_to_string(trace).expect("read Git trace"),
        "",
        "config authorization must reject before rev-parse or numstat starts Git",
    );
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read worktree file"),
        "orig\n"
    );
}

#[test]
fn apply_rejects_unseen_driver_added_through_worktree_include_after_source_probe() {
    const TEST_NAME: &str = "apply::filter_tests::apply_rejects_unseen_driver_added_through_worktree_include_after_source_probe";
    if std::env::var_os("CODEX_GIT_UTILS_APPLY_FILTER_ENV_CHILD").is_none() {
        let trace = tempfile::NamedTempFile::new().expect("trace file");
        run_isolated_test(TEST_NAME, &[("GIT_TRACE2_EVENT", trace.path().as_os_str())]);
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt filter=fresh\n")
        .expect("write filter attributes");
    run_success(root, &["add", ".gitattributes"]);
    run_success(root, &["commit", "-m", "filter target"]);
    let included = root.join("driver-config");
    std::fs::write(&included, "").expect("write initially empty included config");
    run_success(root, &["config", "include.path", "../driver-config"]);

    let trace = PathBuf::from(std::env::var_os("GIT_TRACE2_EVENT").expect("trace path"));
    std::fs::write(&trace, "").expect("clear fixture trace");
    let phase = crate::git_config_sources::install_blocking_no_includes_query_phase();
    let watcher = thread::spawn(move || {
        phase.run(|| {
            std::fs::write(
                included,
                format!("[filter \"fresh\"]\n\tclean = {FILTER_COMMAND}\n\trequired = true\n"),
            )
            .expect("add previously unseen filter driver");
        })
    });

    let error = apply_git_patch(&request(
        root, /*revert*/ false, /*preflight*/ false,
    ))
    .expect_err("reject worktree config source before final apply");
    crate::git_config_sources::clear_blocking_no_includes_query_phase();
    assert!(watcher.join().expect("config watcher"));

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(!configured_filter_ran(root));
    assert_eq!(index_contents(root, "file.txt"), "orig\n");
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read worktree file"),
        "orig\n"
    );
    assert!(
        !std::fs::read_to_string(trace)
            .expect("read completed trace")
            .contains("--3way")
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

#[test]
fn apply_neutralizes_racy_offpath_clean_and_process_filters() {
    for (name, command) in [
        ("clean", FILTER_COMMAND),
        ("process", PROCESS_FILTER_COMMAND),
    ] {
        for kind in [PatchKind::Direct, PatchKind::ThreeWay] {
            let (repo, patch, outside_index) =
                setup_racy_offpath_filter("demo", name, command, kind);
            assert_guarded_round_trip(&repo, patch, &outside_index, &format!("{name} {kind:?}"));
        }
    }
}

#[test]
fn apply_neutralizes_empty_and_equals_filter_driver_names() {
    for (driver, name, command, kind) in [
        ("", "clean", FILTER_COMMAND, PatchKind::Direct),
        (
            "x=y",
            "process",
            PROCESS_FILTER_COMMAND,
            PatchKind::ThreeWay,
        ),
    ] {
        let (repo, patch, outside_index) = setup_racy_offpath_filter(driver, name, command, kind);
        assert_guarded_round_trip(
            &repo,
            patch,
            &outside_index,
            &format!("driver {driver:?} {name}"),
        );
    }
}

#[test]
fn command_scoped_offpath_filter_is_neutralized_after_local_config() {
    if std::env::var_os("CODEX_GIT_UTILS_APPLY_FILTER_ENV_CHILD").is_none() {
        run_isolated_test(
            "apply::filter_tests::command_scoped_offpath_filter_is_neutralized_after_local_config",
            &[(
                "CODEX_APPLY_GIT_CFG",
                OsStr::new(COMMAND_SCOPED_FILTER_CONFIG),
            )],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("outside.txt"), "outside\n").expect("write off-path file");
    std::fs::write(root.join(".gitattributes"), "outside.txt filter=demo\n")
        .expect("write off-path attributes");
    run_success(root, &["add", ".gitattributes", "outside.txt"]);
    run_success(root, &["commit", "-m", "off-path filter target"]);
    run_success(root, &["config", "filter.demo.clean", ""]);
    run_success(root, &["config", "filter.demo.required", "true"]);
    let outside_index = index_contents(root, "outside.txt");
    make_index_racy(root);
    assert_guarded_round_trip(
        &repo,
        PATCH.to_string(),
        &outside_index,
        "command-scoped clean",
    );
}

#[test]
fn filter_override_rejects_process_temp_directory_inside_worktree() {
    let root_name = "CODEX_GIT_UTILS_WORKTREE_TMP_ROOT";
    if let Some(root) = std::env::var_os(root_name) {
        let root = std::path::PathBuf::from(root);
        let error = apply_git_patch(&request(
            &root, /*revert*/ false, /*preflight*/ true,
        ))
        .expect_err("reject worktree-owned filter override");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(!configured_filter_ran(&root));
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    configure_filter(root, "demo");
    let worktree_temp = root.join("process-temp");
    std::fs::create_dir(&worktree_temp).expect("worktree temp directory");
    run_isolated_test(
        "apply::filter_tests::filter_override_rejects_process_temp_directory_inside_worktree",
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
fn apply_neutralizes_filter_activated_for_racy_offpath_by_same_patch() {
    for (name, command) in [
        ("clean", FILTER_COMMAND),
        ("process", PROCESS_FILTER_COMMAND),
    ] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("outside.txt"), "outside\n").expect("write off-path file");
        std::fs::write(root.join(".gitattributes"), "outside.txt -filter\n")
            .expect("write preimage attributes");
        run_success(root, &["add", ".gitattributes", "outside.txt"]);
        run_success(root, &["commit", "-m", "off-path filter preimage"]);

        std::fs::write(root.join(".gitattributes"), "outside.txt filter=demo\n")
            .expect("write postimage attributes");
        let patch = build_patch(root, PatchKind::ThreeWay, &[".gitattributes", "file.txt"]);
        configure_filter_command(root, "demo", name, command);
        run_success(root, &["config", "filter.demo.required", "true"]);
        let outside_index = index_contents(root, "outside.txt");
        make_index_racy(root);

        assert_guarded_round_trip(
            &repo,
            patch,
            &outside_index,
            &format!("dynamic off-path {name}"),
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".gitattributes")).expect("read restored attributes"),
            "outside.txt -filter\n"
        );
    }
}

#[test]
fn apply_still_refuses_clean_and_process_filters_selected_for_target() {
    for (name, command) in [
        ("clean", FILTER_COMMAND),
        ("process", PROCESS_FILTER_COMMAND),
    ] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join(".gitattributes"), "file.txt filter=demo\n")
            .expect("write target attributes");
        run_success(root, &["add", ".gitattributes"]);
        run_success(root, &["commit", "-m", "target filter"]);
        configure_filter_command(root, "demo", name, command);
        run_success(root, &["config", "filter.demo.required", "true"]);

        let error = apply_git_patch(&request(
            root, /*revert*/ false, /*preflight*/ false,
        ))
        .expect_err("refuse selected target filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported, "{name}");
        assert!(!configured_filter_ran(root), "{name}");
        assert_eq!(index_contents(root, "file.txt"), "orig\n", "{name}");
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt")).expect("read target"),
            "orig\n",
            "{name}"
        );
    }
}
