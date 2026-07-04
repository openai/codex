use super::*;
use pretty_assertions::assert_eq;
use std::process::Command;

use crate::safe_git::DISABLED_HOOKS_PATH;

#[cfg(unix)]
fn write_executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, body).expect("write executable");
    let mut permissions = std::fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("set executable permissions");
}

fn run_git(cwd: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let status = command
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run real Git");
    assert!(status.success(), "git {args:?} failed");
}

fn run_git_stdout(cwd: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run real Git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 Git output")
        .trim()
        .to_string()
}

#[test]
fn runner_binds_config_environment_across_ambient_mutation() {
    const CHILD: &str = "CODEX_GIT_CONFIG_BINDING_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let fixture = tempfile::tempdir().expect("fixture");
        let repo = fixture.path().join("repo");
        let safe_home = fixture.path().join("safe-home");
        let malicious_home = repo.join("malicious-home");
        std::fs::create_dir_all(&repo).expect("repo");
        std::fs::create_dir_all(&safe_home).expect("safe home");
        std::fs::create_dir_all(&malicious_home).expect("malicious home");
        run_git(&repo, &["init", "-q"]);
        std::fs::write(
            safe_home.join(".gitconfig"),
            "[codex]\n\tbound = safe-home\n",
        )
        .expect("safe home config");
        let malicious_global = repo.join("malicious.gitconfig");
        std::fs::write(&malicious_global, "[codex]\n\tbound = malicious\n")
            .expect("malicious global config");
        std::fs::write(
            malicious_home.join(".gitconfig"),
            "[codex]\n\tbound = malicious-home\n",
        )
        .expect("malicious home config");

        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        isolate_git_command_environment(&mut command);
        let output = command
            .arg("git_command::tests::runner_binds_config_environment_across_ambient_mutation")
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD, "1")
            .env("RUST_TEST_THREADS", "1")
            .env("CODEX_GIT_CONFIG_BINDING_ROOT", &repo)
            .env("CODEX_GIT_CONFIG_BINDING_MALICIOUS", &malicious_global)
            .env("CODEX_GIT_CONFIG_BINDING_MALICIOUS_HOME", &malicious_home)
            .env("HOME", &safe_home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "codex.count")
            .env("GIT_CONFIG_VALUE_0", "safe-count")
            .env_remove("GIT_CONFIG_GLOBAL")
            .env_remove("GIT_CONFIG_SYSTEM")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .output()
            .expect("isolated config binding test");
        assert!(
            output.status.success(),
            "isolated config binding test failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let root = PathBuf::from(std::env::var_os("CODEX_GIT_CONFIG_BINDING_ROOT").expect("root"));
    let malicious =
        std::env::var_os("CODEX_GIT_CONFIG_BINDING_MALICIOUS").expect("malicious config");
    let malicious_home =
        std::env::var_os("CODEX_GIT_CONFIG_BINDING_MALICIOUS_HOME").expect("malicious home");
    let runner = GitRunner::for_cwd_io(&root).expect("runner with captured config environment");

    // SAFETY: the parent starts an isolated test process that runs only this
    // exact test with one harness thread. No other application thread reads or
    // writes these variables, and the process exits after the assertions.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", malicious);
        std::env::set_var("HOME", malicious_home);
        std::env::set_var("GIT_CONFIG_VALUE_0", "malicious-count");
        std::env::set_var(
            "GIT_CONFIG_PARAMETERS",
            "'codex.count'='malicious-parameter'",
        );
    }

    for (key, expected) in [("codex.bound", "safe-home"), ("codex.count", "safe-count")] {
        let mut command = runner.command_for_cwd(&root).expect("bound command");
        command.args(["config", "--get", key]);
        let output = runner.output(command).expect("bound config read");
        assert!(
            output.status.success(),
            "{key}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

fn commit_all(cwd: &Path, message: &str) {
    run_git(
        cwd,
        &[
            "-c",
            "user.name=Codex Test",
            "-c",
            "user.email=codex@example.com",
            "-c",
            "commit.gpgSign=false",
            "commit",
            "-qam",
            message,
        ],
    );
}

fn write_git_candidate(directory: &Path) {
    std::fs::create_dir_all(directory).expect("create candidate directory");
    let candidate = directory.join(git_executable_name());
    #[cfg(windows)]
    {
        let mut pe = [0_u8; 68];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64_u32.to_le_bytes());
        pe[64..].copy_from_slice(b"PE\0\0");
        std::fs::write(candidate, pe).expect("write native PE fixture");
    }
    #[cfg(not(windows))]
    std::fs::copy(native_git_fixture(), candidate).expect("copy native Git fixture");
}

fn write_runnable_git_candidate(directory: &Path) {
    #[cfg(windows)]
    {
        std::fs::create_dir_all(directory.parent().expect("candidate parent"))
            .expect("create candidate parent");
        create_junction(directory, &native_git_search_directory());
    }
    #[cfg(not(windows))]
    write_git_candidate(directory);
}

#[cfg(windows)]
fn native_git_search_directory() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(git_executable_name());
        if let Ok(candidate) = std::fs::canonicalize(candidate)
            && crate::git_executable::is_native_executable_file(&candidate)
        {
            return directory;
        }
    }
    panic!("no native Git directory in PATH")
}

fn native_git_fixture() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(git_executable_name());
        if let Ok(candidate) = std::fs::canonicalize(candidate)
            && crate::git_executable::is_native_executable_file(&candidate)
        {
            return candidate;
        }
    }
    panic!("no native Git fixture in PATH")
}

#[cfg(windows)]
fn create_junction(path: &Path, target: &Path) {
    // Bazel's GNU Windows runner can surface temporary paths with `/`
    // separators. `mklink` treats those separators as option prefixes, so
    // pass native separators to the cmd.exe built-in.
    let path = path.as_os_str().to_string_lossy().replace('/', "\\");
    let target = target.as_os_str().to_string_lossy().replace('/', "\\");
    let output = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("create junction");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mklink failed: stdout={stdout} stderr={stderr}"
    );
}

fn locations_for_root(root: &Path) -> RepositoryAuthority {
    let mut roots = vec![root.to_path_buf()];
    let canonical = std::fs::canonicalize(root).expect("canonical root");
    if !roots.contains(&canonical) {
        roots.push(canonical);
    }
    RepositoryAuthority::from_test_locations(roots.clone(), roots, Vec::new())
        .expect("test repository authority")
}

fn raw_parent_traversal(root: &Path, sibling: &str) -> PathBuf {
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let mut path = root.as_os_str().to_os_string();
    path.push(&separator);
    path.push("..");
    path.push(&separator);
    path.push(sibling);
    path.into()
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}

fn tempdir_for_native_git() -> tempfile::TempDir {
    #[cfg(windows)]
    {
        // Git for Windows rejects the `\\?\` spelling returned when the
        // process temp directory is canonicalized.
        tempfile::tempdir().expect("fixture")
    }
    #[cfg(not(windows))]
    {
        let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
        tempfile::tempdir_in(temp_base).expect("fixture")
    }
}

fn git_config_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    #[cfg(not(windows))]
    let path = path.into_owned();
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

struct OverlappingRegisteredRoute {
    main: PathBuf,
    nested: PathBuf,
    linked: PathBuf,
    #[cfg(unix)]
    nested_admin: PathBuf,
    linked_admin: PathBuf,
    registry_marker: PathBuf,
    raw_marker: PathBuf,
}

fn overlapping_registered_route(fixture: &Path) -> OverlappingRegisteredRoute {
    let main = fixture.join("main");
    let nested = main.join("nested");
    let linked = main.join("linked");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&nested)]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    #[cfg(unix)]
    let nested_admin = PathBuf::from(run_git_stdout(
        &nested,
        &["rev-parse", "--absolute-git-dir"],
    ));
    let linked_admin = PathBuf::from(run_git_stdout(
        &linked,
        &["rev-parse", "--absolute-git-dir"],
    ));
    let registry_marker = linked_admin.join("gitdir");
    let raw_marker = nested.join("..").join("linked/.git");
    std::fs::write(&registry_marker, format!("{}\n", raw_marker.display()))
        .expect("write overlapping registry route");
    OverlappingRegisteredRoute {
        main,
        nested,
        linked,
        #[cfg(unix)]
        nested_admin,
        linked_admin,
        registry_marker,
        raw_marker,
    }
}

struct MetadataAliasRegisteredRoute {
    main: PathBuf,
    linked: PathBuf,
    linked_admin: PathBuf,
    pivot: PathBuf,
    registry_marker: PathBuf,
    #[cfg(unix)]
    raw_marker: PathBuf,
}

fn metadata_alias_registered_route(
    fixture: &Path,
    main_name: &str,
    route_main_name: &str,
) -> MetadataAliasRegisteredRoute {
    let main = fixture.join(main_name);
    let linked = main.join("linked");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    let linked_admin = PathBuf::from(run_git_stdout(
        &linked,
        &["rev-parse", "--absolute-git-dir"],
    ));
    let pivot = main.join("pivot");
    let registry_marker = linked_admin.join("gitdir");
    let raw_marker = fixture
        .join(route_main_name)
        .join("pivot")
        .join("..")
        .join("linked/.git");
    std::fs::write(&registry_marker, format!("{}\n", raw_marker.display()))
        .expect("write metadata-alias registry route");
    MetadataAliasRegisteredRoute {
        main,
        linked,
        linked_admin,
        pivot,
        registry_marker,
        #[cfg(unix)]
        raw_marker,
    }
}

#[test]
fn git_metadata_marker_parser_preserves_leading_path_space() {
    assert_eq!(
        parse_git_marker_path(b"gitdir:  relative/path \r\n", b"gitdir: ")
            .expect("parse .git marker"),
        PathBuf::from(" relative/path")
    );
    assert_eq!(
        parse_git_marker_path(b" relative/common \n", b"").expect("parse commondir marker"),
        PathBuf::from(" relative/common")
    );
    assert!(parse_git_marker_path(b"gitdir:/missing-space\n", b"gitdir: ").is_err());
}

fn selected_git(locations: &RepositoryAuthority, directories: &[&Path]) -> PathBuf {
    let search_path = std::env::join_paths(directories).expect("PATH");
    select_git_executable(locations, &search_path)
        .expect("trusted Git")
        .argv0
}

fn assert_unsafe_metadata_route(result: Result<RepositoryAuthority, GitReadError>, marker: &Path) {
    match result {
        Err(GitReadError::UnsafeRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, marker);
            assert_eq!(reason, "Git metadata route crosses a repository worktree");
        }
        other => panic!("expected unsafe repository metadata, got {other:?}"),
    }
}

fn assert_unsafe_registry_route(result: Result<RepositoryAuthority, GitReadError>, marker: &Path) {
    match result {
        Err(GitReadError::UnsafeRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, marker);
            assert_eq!(
                reason,
                "Git worktree registry route crosses a repository worktree"
            );
        }
        other => panic!("expected unsafe worktree registry route, got {other:?}"),
    }
}

#[cfg(unix)]
fn assert_unsafe_registry_route_at_one_of(
    result: Result<RepositoryAuthority, GitReadError>,
    markers: &[&Path],
) {
    match result {
        Err(GitReadError::UnsafeRepositoryMetadata { path, reason }) => {
            assert_eq!(
                reason,
                "Git worktree registry route crosses a repository worktree"
            );
            assert!(
                markers
                    .iter()
                    .any(|marker| affected_paths_match(&path, marker)),
                "unexpected affected registry path: {}",
                path.display()
            );
        }
        other => panic!("expected unsafe worktree registry route, got {other:?}"),
    }
}

fn assert_same_affected_path(actual: &Path, expected: &Path) {
    assert!(
        affected_paths_match(actual, expected),
        "affected paths differ: actual={} expected={}",
        actual.display(),
        expected.display()
    );
}

fn affected_paths_match(actual: &Path, expected: &Path) -> bool {
    if actual.file_name() != expected.file_name() {
        return false;
    }
    let Some(actual_parent) = actual.parent() else {
        return false;
    };
    let Some(expected_parent) = expected.parent() else {
        return false;
    };
    same_file::is_same_file(actual_parent, expected_parent).unwrap_or(false)
}

#[test]
fn git_read_error_io_kind_table_is_exhaustive() {
    let path = PathBuf::from("repository/.git");
    for (error, expected) in [
        (GitReadError::NoTrustedGit, io::ErrorKind::NotFound),
        (
            GitReadError::NotRepository { path: path.clone() },
            io::ErrorKind::NotFound,
        ),
        (
            GitReadError::UnprovenPrimaryAuthority {
                common_dir: "repository.git".to_string(),
            },
            io::ErrorKind::PermissionDenied,
        ),
        (
            GitReadError::UnsafeRepositoryMetadata {
                path: path.clone(),
                reason: "crosses worktree".to_string(),
            },
            io::ErrorKind::PermissionDenied,
        ),
        (
            GitReadError::InvalidRepositoryMetadata {
                path,
                reason: "malformed marker".to_string(),
            },
            io::ErrorKind::InvalidData,
        ),
        (
            GitReadError::InvalidConfigEnvironment {
                reason: "malformed GIT_CONFIG_COUNT".to_string(),
            },
            io::ErrorKind::InvalidData,
        ),
    ] {
        assert_eq!(error.io_kind(), expected, "{error}");
        assert_eq!(error.into_io_error().kind(), expected);
    }
}

#[test]
fn malformed_metadata_preserves_path_reason_and_invalid_data_mapping() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repository");
    std::fs::create_dir_all(&root).expect("create repository root");
    let marker = root.join(".git");
    std::fs::write(&marker, "not-a-gitdir-marker\n").expect("write malformed marker");

    match GitRunner::for_cwd(&root) {
        Err(GitReadError::InvalidRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, &marker);
            assert!(reason.contains("malformed Git metadata marker"), "{reason}");
        }
        other => panic!("expected invalid repository metadata, got {other:?}"),
    }
    let error = GitRunner::for_cwd_io(&root).expect_err("malformed metadata");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("malformed Git metadata marker"),
        "{error}"
    );
    assert!(!error.to_string().contains("PATH"), "{error}");
}

#[cfg(unix)]
#[test]
fn cyclic_metadata_route_is_invalid_not_unsafe() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repository");
    let external = fixture.path().join("external");
    std::fs::create_dir_all(&root).expect("create repository root");
    std::fs::create_dir_all(&external).expect("create external directory");
    let route = external.join("entry");
    symlink("entry", &route).expect("create route cycle");
    let marker = root.join(".git");
    std::fs::write(&marker, format!("gitdir: {}\n", route.display())).expect("write cyclic marker");

    match GitRunner::for_cwd(&root) {
        Err(GitReadError::InvalidRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, &marker);
            assert!(!reason.is_empty());
        }
        other => panic!("expected invalid repository metadata, got {other:?}"),
    }
    assert_eq!(
        GitRunner::for_cwd_io(&root)
            .expect_err("cyclic metadata route")
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[cfg(unix)]
#[test]
fn symlinked_directory_metadata_marker_is_invalid() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repository");
    let external = fixture.path().join("external-metadata");
    std::fs::create_dir_all(&root).expect("create repository root");
    run_git(&root, &["init", "-q"]);
    std::fs::rename(root.join(".git"), &external).expect("move metadata directory");
    symlink(&external, root.join(".git")).expect("symlink metadata marker");

    match GitRunner::for_cwd(&root) {
        Err(GitReadError::InvalidRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, &root.join(".git"));
            assert_eq!(reason, "symlinked Git metadata marker");
        }
        other => panic!("expected invalid symlinked metadata marker, got {other:?}"),
    }
    assert_eq!(
        GitRunner::for_cwd_io(&root)
            .expect_err("symlinked metadata marker")
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[cfg(windows)]
#[test]
fn directory_git_junction_to_external_metadata_is_rejected() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repository");
    let external = fixture.path().join("external-metadata");
    std::fs::create_dir_all(&root).expect("create repository root");
    run_git(&root, &["init", "-q"]);
    std::fs::rename(root.join(".git"), &external).expect("move metadata directory");
    create_junction(&root.join(".git"), &external);
    run_git(&root, &["rev-parse", "--absolute-git-dir"]);

    let error = GitRunner::for_cwd(&root).expect_err("nonstandard directory metadata route");
    match error {
        GitReadError::UnsafeRepositoryMetadata { path, reason } => {
            assert_same_affected_path(&path, &root.join(".git"));
            assert_eq!(reason, "nonstandard Git metadata directory");
        }
        GitReadError::InvalidRepositoryMetadata { path, reason } => {
            assert_same_affected_path(&path, &root.join(".git"));
            assert_eq!(reason, "symlinked Git metadata marker");
        }
        other => panic!("expected rejected directory metadata junction, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn resolver_skips_untrusted_path_entries_and_runs_external_candidate() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let outside = fixture.path().join("outside");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo_bin).expect("repo bin");
    std::fs::create_dir_all(&outside).expect("outside bin");
    std::fs::create_dir_all(&trusted_bin).expect("trusted bin");
    write_git_candidate(&repo_bin);
    std::os::unix::fs::symlink(repo_bin.join("git"), outside.join("git"))
        .expect("outside symlink into repository");
    write_git_candidate(&trusted_bin);

    let path = std::env::join_paths([
        PathBuf::from("relative"),
        repo_bin,
        outside,
        trusted_bin.clone(),
    ])
    .expect("PATH");
    let locations = locations_for_root(&repo);
    let runner = GitRunner::from_search_path(locations, &path).expect("trusted Git");
    assert_eq!(runner.argv0, trusted_bin.join("git"));
    let mut command = runner.command();
    command.arg("--version");
    let output = runner.output(command).expect("run trusted Git");
    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"git version "));
}

#[cfg(unix)]
#[test]
fn resolver_skips_env_absolute_and_no_shebang_git_scripts() {
    let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
    let fixture = tempfile::tempdir_in(temp_base).expect("fixture");
    let repo = fixture.path().join("repo");
    let trusted = fixture.path().join("trusted");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&trusted);

    let mut script_dirs = Vec::new();
    for (name, prefix) in [
        ("env-shebang", "#!/usr/bin/env sh\n"),
        ("absolute-shebang", "#!/bin/sh\n"),
        ("no-shebang", ""),
    ] {
        let directory = fixture.path().join(name);
        let marker = fixture.path().join(format!("{name}-ran"));
        std::fs::create_dir_all(&directory).expect("create script directory");
        write_executable(
            &directory.join("git"),
            &format!("{prefix}touch '{}'\n", marker.display()),
        );
        script_dirs.push((directory, marker));
    }

    let locations = locations_for_root(&repo);
    let mut path_entries = script_dirs
        .iter()
        .map(|(directory, _)| directory.as_path())
        .collect::<Vec<_>>();
    let scripts_only = std::env::join_paths(&path_entries).expect("scripts-only PATH");
    let error = match select_git_executable(&locations, &scripts_only) {
        Ok(_) => panic!("selected a non-native Git wrapper"),
        Err(error) => error,
    };
    assert_eq!(error, GitReadError::NoTrustedGit);
    let message = error.to_string();
    assert!(message.contains("native Git"), "{message}");
    assert!(
        message.contains("script-based and non-native Git wrappers"),
        "{message}"
    );
    assert!(message.contains("outside the repository"), "{message}");
    assert!(message.contains("PATH"), "{message}");
    assert_eq!(error.io_kind(), io::ErrorKind::NotFound);
    for (_, marker) in &script_dirs {
        assert!(
            !marker.exists(),
            "non-native Git script executed: {marker:?}"
        );
    }
    path_entries.push(&trusted);
    let path = std::env::join_paths(path_entries).expect("PATH");
    let runner = GitRunner::from_search_path(locations, &path).expect("native Git fallback");
    assert_eq!(runner.argv0, trusted.join("git"));
    let mut command = runner.command();
    command.arg("--version");
    assert!(
        runner
            .output(command)
            .expect("run native Git")
            .status
            .success()
    );
    for (_, marker) in script_dirs {
        assert!(
            !marker.exists(),
            "non-native Git script executed: {marker:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn selected_git_uses_sanitized_path_and_strips_loader_injection_environment() {
    let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
    let fixture = tempfile::tempdir_in(temp_base).expect("fixture");
    let repo = fixture.path().join("repo");
    let unsafe_bin = repo.join("bin");
    let trusted = fixture.path().join("trusted");
    std::fs::create_dir_all(&unsafe_bin).expect("create unsafe bin");
    std::fs::create_dir_all(&trusted).expect("create trusted bin");
    std::fs::copy(
        std::env::current_exe().expect("current test executable"),
        trusted.join("git"),
    )
    .expect("copy native environment probe fixture");

    let locations = locations_for_root(&repo);
    let path = std::env::join_paths([&unsafe_bin, &trusted]).expect("PATH");
    let runner = GitRunner::from_search_path(locations, &path).expect("trusted native env");
    let mut command = runner.command();
    let injected = repo.join("inject");
    command
        .arg("git_command::tests::native_environment_probe_child")
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_ENVIRONMENT_PROBE_CHILD", "1")
        .env(
            "CODEX_GIT_EXPECTED_SAFE_PATH",
            std::fs::canonicalize(&trusted).expect("canonical trusted bin"),
        );
    for name in [
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "LD_AUDIT",
        "LIBPATH",
        "SHLIB_PATH",
        "GCONV_PATH",
        "NIX_LD",
        "NIX_LD_LIBRARY_PATH",
        "CORECLR_ENABLE_PROFILING",
        "COR_ENABLE_PROFILING",
        "DOTNET_STARTUP_HOOKS",
    ] {
        command.env(name, &injected);
    }
    command.env("PATH", &path);
    let output = runner.output(command).expect("run sanitized native env");
    assert!(
        output.status.success(),
        "native env failed with {}: stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(unix)]
#[test]
fn native_environment_probe_child() {
    if std::env::var_os("CODEX_GIT_ENVIRONMENT_PROBE_CHILD").is_none() {
        return;
    }
    for name in [
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "LD_AUDIT",
        "LIBPATH",
        "SHLIB_PATH",
        "GCONV_PATH",
        "NIX_LD",
        "NIX_LD_LIBRARY_PATH",
        "CORECLR_ENABLE_PROFILING",
        "COR_ENABLE_PROFILING",
        "DOTNET_STARTUP_HOOKS",
    ] {
        assert!(std::env::var_os(name).is_none(), "{name} survived");
    }
    assert_eq!(
        std::env::var_os("PATH").expect("sanitized PATH"),
        std::env::var_os("CODEX_GIT_EXPECTED_SAFE_PATH").expect("expected safe PATH")
    );
}

#[test]
fn command_for_relative_cwd_uses_original_process_directory() {
    let current = std::fs::canonicalize(std::env::current_dir().expect("current dir"))
        .expect("canonical current dir");
    let fixture = tempfile::tempdir_in(&current).expect("fixture");
    let repo = fixture.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create repository");
    run_git(&repo, &["init", "-q"]);
    let relative = repo.strip_prefix(&current).expect("relative repository");
    let locations = repository_authority_for_cwd(&repo).expect("untrusted locations");
    let path = std::env::var_os("PATH").expect("PATH");
    let runner = GitRunner::from_search_path(locations, &path).expect("trusted Git");
    let mut command = runner
        .command_for_cwd(relative)
        .expect("relative command cwd");
    command.args(["rev-parse", "--show-toplevel"]);
    crate::repository_authority::reset_bounded_marker_read_count();
    let output = runner.output(command).expect("run Git from relative cwd");
    assert!(output.status.success());
    assert_eq!(
        std::fs::canonicalize(String::from_utf8_lossy(&output.stdout).trim())
            .expect("canonical Git root"),
        std::fs::canonicalize(&repo).expect("canonical expected root")
    );
    assert_eq!(
        crate::repository_authority::bounded_marker_read_count(),
        0,
        "standard repository should not reread marker bodies"
    );
}

#[cfg(unix)]
#[test]
fn selected_runner_executes_pinned_target_after_path_hop_is_retargeted() {
    let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
    let fixture = tempfile::tempdir_in(temp_base).expect("fixture");
    let repo = fixture.path().join("repo");
    let switch = fixture.path().join("switch");
    let trusted = fixture.path().join("trusted");
    let attacker = fixture.path().join("attacker");
    let entry = fixture.path().join("entry");
    std::fs::create_dir_all(&repo).expect("create repository");
    std::fs::create_dir_all(&trusted).expect("create trusted target");
    std::fs::create_dir_all(&attacker).expect("create attacker target");
    std::fs::create_dir_all(&entry).expect("create PATH entry");
    std::fs::copy("/usr/bin/true", trusted.join("git")).expect("copy trusted native target");
    std::fs::copy("/usr/bin/false", attacker.join("git")).expect("copy attacker native target");
    std::os::unix::fs::symlink(&trusted, &switch).expect("trusted PATH hop");
    std::os::unix::fs::symlink(switch.join("git"), entry.join("git"))
        .expect("external candidate through PATH hop");

    let locations = locations_for_root(&repo);
    let path = std::env::join_paths([&entry]).expect("PATH");
    let runner = GitRunner::from_search_path(locations, &path).expect("initial trusted target");
    std::fs::remove_file(&switch).expect("remove trusted PATH hop");
    std::os::unix::fs::symlink(&attacker, &switch).expect("retarget PATH hop");

    let output = runner.output(runner.command()).expect("run selected Git");
    assert!(output.status.success(), "selected Git failed");
}

#[cfg(unix)]
#[test]
fn resolver_rejects_same_identity_symlink_alias_to_repository() {
    let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
    let fixture = tempfile::tempdir_in(temp_base).expect("fixture");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let alias = fixture.path().join("repo-alias");
    let alias_bin = alias.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&repo_bin);
    write_git_candidate(&trusted_bin);
    std::os::unix::fs::symlink(&repo, &alias).expect("symlink repository alias");

    let locations = locations_for_root(&repo);
    assert_eq!(
        selected_git(&locations, &[&alias_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[cfg(target_os = "macos")]
#[test]
fn resolver_rejects_apfs_data_firmlink_alias_to_repository() {
    let data_volume_temp =
        std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp directory");
    let fixture = tempfile::tempdir_in(&data_volume_temp).expect("fixture under Data volume");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&repo_bin);
    write_git_candidate(&trusted_bin);

    let data_alias = PathBuf::from("/System/Volumes/Data")
        .join(repo.strip_prefix("/").expect("absolute repository path"));
    let alias_bin = data_alias.join("bin");
    if std::fs::metadata(&data_alias).is_err() {
        eprintln!("APFS Data firmlink alias is unavailable; skipping native alias assertion");
        return;
    }
    assert!(
        same_file::is_same_file(&data_alias, &repo).expect("compare APFS Data alias identity"),
        "APFS Data spelling does not identify the fixture repository"
    );

    let locations = locations_for_root(&repo);
    assert_eq!(
        selected_git(&locations, &[&alias_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[cfg(target_os = "macos")]
#[test]
fn metadata_routes_distinguish_apfs_worktree_hop_from_direct_metadata_alias() {
    use std::os::unix::fs::symlink;

    let data_volume_temp =
        std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp directory");
    let fixture = tempfile::tempdir_in(&data_volume_temp).expect("fixture under Data volume");
    let outer = fixture.path().join("outer");
    let root = outer.join("nested");
    let admin = outer.join(".git/modules/nested");
    std::fs::create_dir_all(&root).expect("create nested repository");
    run_git(&outer, &["init", "-q"]);
    run_git(&root, &["init", "-q"]);
    std::fs::create_dir_all(admin.parent().expect("module metadata parent"))
        .expect("create module metadata parent");
    std::fs::rename(root.join(".git"), &admin).expect("move metadata into outer .git");
    let data_alias = PathBuf::from("/System/Volumes/Data")
        .join(outer.strip_prefix("/").expect("absolute repository path"));
    if std::fs::metadata(&data_alias).is_err() {
        eprintln!("APFS Data firmlink alias is unavailable; skipping native route assertion");
        return;
    }
    assert!(same_file::is_same_file(&data_alias, &outer).expect("compare APFS root alias"));

    // A direct alternate spelling of proven outer metadata remains compatible.
    std::fs::write(
        root.join(".git"),
        format!(
            "gitdir: {}\n",
            data_alias.join(".git/modules/nested").display()
        ),
    )
    .expect("write direct metadata alias marker");
    let runner = GitRunner::for_cwd(&root).expect("direct APFS metadata alias");
    let mut command = runner
        .command_for_cwd(&root)
        .expect("direct APFS alias command");
    command.args(["rev-parse", "--absolute-git-dir"]);
    assert!(
        runner
            .output(command)
            .expect("direct APFS alias rev-parse")
            .status
            .success()
    );

    // A different symlink entry reached through the same root identity is a
    // mutable worktree hop even when it exits to the same external metadata.
    symlink(&admin, outer.join("switch")).expect("worktree route switch");
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", data_alias.join("switch").display()),
    )
    .expect("write APFS worktree-hop marker");
    run_git(&root, &["rev-parse", "--absolute-git-dir"]);
    assert_unsafe_metadata_route(repository_authority_for_cwd(&root), &root.join(".git"));
}

#[cfg(windows)]
#[test]
fn resolver_rejects_same_identity_junction_alias_to_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let repo_bin = repo.join("bin");
    let alias = fixture.path().join("repo-alias");
    let alias_bin = alias.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&repo_bin);
    write_git_candidate(&trusted_bin);
    create_junction(&alias, &repo);

    let locations = locations_for_root(&repo);
    assert_eq!(
        selected_git(&locations, &[&alias_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[cfg(unix)]
#[test]
fn gitdir_route_through_current_worktree_is_denied_before_path_selection() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repo");
    let external = fixture.path().join("external");
    let admin = external.join("admin");
    std::fs::create_dir_all(&root).expect("create repository");
    std::fs::create_dir_all(&external).expect("create external directory");
    run_git(&root, &["init", "-q"]);
    std::fs::rename(root.join(".git"), &admin).expect("move metadata external");
    symlink(&admin, root.join("switch")).expect("worktree metadata switch");
    std::fs::write(root.join(".git"), "gitdir: switch\n").expect("write gitdir marker");
    run_git(&root, &["rev-parse", "--absolute-git-dir"]);

    assert_unsafe_metadata_route(repository_authority_for_cwd(&root), &root.join(".git"));
    let error = GitRunner::for_cwd_io(&root).expect_err("unsafe metadata route");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("Git metadata route crosses"));
    assert!(!error.to_string().contains("PATH"));
}

#[cfg(unix)]
#[test]
fn gitdir_route_that_enters_and_exits_worktree_by_identity_is_denied() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repo");
    let external = fixture.path().join("external");
    let admin = external.join("admin");
    let entry = external.join("entry");
    std::fs::create_dir_all(&root).expect("create repository");
    std::fs::create_dir_all(&external).expect("create external directory");
    run_git(&root, &["init", "-q"]);
    std::fs::rename(root.join(".git"), &admin).expect("move metadata external");
    symlink(&root, &entry).expect("external entry into worktree");
    symlink(&admin, root.join("switch")).expect("worktree exit to metadata");
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", entry.join("switch").display()),
    )
    .expect("write identity-hop gitdir marker");
    run_git(&root, &["rev-parse", "--absolute-git-dir"]);

    assert_unsafe_metadata_route(repository_authority_for_cwd(&root), &root.join(".git"));
}

#[cfg(unix)]
#[test]
fn gitdir_symlink_target_cannot_hide_mutable_worktree_pivot_before_parent_traversal() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let primary = fixture.path().join("primary");
    let current = fixture.path().join("current");
    let attacker = fixture.path().join("attacker");
    let external = fixture.path().join("external");
    std::fs::create_dir_all(primary.join("pivot")).expect("create worktree pivot");
    std::fs::create_dir_all(&current).expect("create current worktree");
    std::fs::create_dir_all(&attacker).expect("create attacker repository");
    std::fs::create_dir_all(&external).expect("create external directory");
    run_git(&primary, &["init", "-q"]);
    run_git(&attacker, &["init", "-q"]);
    symlink("../primary/pivot/../.git", external.join("entry"))
        .expect("external entry with hidden worktree pivot");
    std::fs::write(
        current.join(".git"),
        format!("gitdir: {}\n", external.join("entry").display()),
    )
    .expect("write current gitdir marker");
    assert_eq!(
        std::fs::canonicalize(run_git_stdout(
            &current,
            &["rev-parse", "--absolute-git-dir"]
        ))
        .expect("canonical initial Git dir"),
        std::fs::canonicalize(primary.join(".git")).expect("canonical primary Git dir")
    );

    assert_unsafe_metadata_route(
        repository_authority_for_cwd(&current),
        &current.join(".git"),
    );

    std::fs::remove_dir(primary.join("pivot")).expect("remove ordinary pivot");
    std::fs::create_dir_all(attacker.join("nested")).expect("create attacker nested directory");
    symlink(attacker.join("nested"), primary.join("pivot")).expect("retarget worktree pivot");
    assert_eq!(
        std::fs::canonicalize(run_git_stdout(
            &current,
            &["rev-parse", "--absolute-git-dir"]
        ))
        .expect("canonical retargeted Git dir"),
        std::fs::canonicalize(attacker.join(".git")).expect("canonical attacker Git dir")
    );
    assert_unsafe_metadata_route(
        repository_authority_for_cwd(&current),
        &current.join(".git"),
    );
}

#[cfg(unix)]
#[test]
fn standard_directory_cwd_cannot_hide_mutable_worktree_pivot() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let original = fixture.path().join("external-standard-repo");
    let attacker = fixture.path().join("attacker");
    let attacker_repo = attacker.join("external-standard-repo");
    std::fs::create_dir_all(outer.join("pivot")).expect("create worktree pivot");
    std::fs::create_dir_all(&original).expect("create original repository");
    std::fs::create_dir_all(&attacker_repo).expect("create attacker repository");
    run_git(&outer, &["init", "-q"]);
    run_git(&original, &["init", "-q"]);
    run_git(&attacker_repo, &["init", "-q"]);

    let raw = outer
        .join("pivot")
        .join("..")
        .join("..")
        .join("external-standard-repo");
    assert_eq!(
        std::fs::canonicalize(run_git_stdout(&raw, &["rev-parse", "--show-toplevel"]))
            .expect("canonical initial Git root"),
        std::fs::canonicalize(&original).expect("canonical original repository")
    );
    let exact_root_route = outer.join("..").join("external-standard-repo");
    repository_authority_for_cwd(&exact_root_route)
        .expect("exact worktree root traversal remains valid");
    assert_unsafe_metadata_route(repository_authority_for_cwd(&raw), &raw.join(".git"));

    std::fs::remove_dir(outer.join("pivot")).expect("remove ordinary pivot");
    let attacker_target = attacker.join("deep/nested");
    std::fs::create_dir_all(&attacker_target).expect("create attacker pivot target");
    symlink(&attacker_target, outer.join("pivot")).expect("retarget worktree pivot");
    assert_eq!(
        std::fs::canonicalize(run_git_stdout(&raw, &["rev-parse", "--show-toplevel"]))
            .expect("canonical retargeted Git root"),
        std::fs::canonicalize(&attacker_repo).expect("canonical attacker repository")
    );
    assert_unsafe_metadata_route(repository_authority_for_cwd(&raw), &raw.join(".git"));
}

#[cfg(unix)]
#[test]
fn command_cwd_is_bound_before_worktree_symlink_retarget() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let original = fixture.path().join("original");
    let attacker = fixture.path().join("attacker");
    let alias = outer.join("nested");
    for repository in [&outer, &original, &attacker] {
        std::fs::create_dir_all(repository).expect("create repository");
        run_git(repository, &["init", "-q"]);
    }
    symlink(&original, &alias).expect("create worktree-controlled cwd alias");

    let runner = GitRunner::for_cwd(&alias).expect("runner for cwd alias");
    let mut pinned = runner
        .command_for_cwd(&alias)
        .expect("command pinned before retarget");
    pinned.args(["rev-parse", "--show-toplevel"]);

    std::fs::remove_file(&alias).expect("remove original cwd alias");
    symlink(&attacker, &alias).expect("retarget cwd alias");
    let error = match runner.command_for_cwd(&alias) {
        Ok(_) => panic!("retargeted cwd alias was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(
        error
            .to_string()
            .contains("no longer resolves within the selected worktree"),
        "{error}"
    );

    let output = runner.output(pinned).expect("run pinned cwd command");
    assert!(output.status.success());
    assert_eq!(
        std::fs::canonicalize(String::from_utf8_lossy(&output.stdout).trim())
            .expect("canonical pinned Git root"),
        std::fs::canonicalize(&original).expect("canonical original repository")
    );
}

#[cfg(windows)]
#[test]
fn command_cwd_is_bound_before_worktree_junction_retarget() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let original = fixture.path().join("original");
    let attacker = fixture.path().join("attacker");
    let alias = outer.join("nested");
    for repository in [&outer, &original, &attacker] {
        std::fs::create_dir_all(repository).expect("create repository");
        run_git(repository, &["init", "-q"]);
    }
    create_junction(&alias, &original);

    let runner = GitRunner::for_cwd(&alias).expect("runner for cwd junction");
    let mut pinned = runner
        .command_for_cwd(&alias)
        .expect("command pinned before retarget");
    pinned.args(["rev-parse", "--show-toplevel"]);

    std::fs::remove_dir(&alias).expect("remove original cwd junction");
    create_junction(&alias, &attacker);
    let error = match runner.command_for_cwd(&alias) {
        Ok(_) => panic!("retargeted cwd junction was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");

    let output = runner.output(pinned).expect("run pinned cwd command");
    assert!(output.status.success());
    assert_eq!(
        std::fs::canonicalize(String::from_utf8_lossy(&output.stdout).trim())
            .expect("canonical pinned Git root"),
        std::fs::canonicalize(&original).expect("canonical original repository")
    );
}

#[cfg(windows)]
#[test]
fn command_for_cwd_executes_from_a_canonical_windows_path() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repo");
    std::fs::create_dir_all(&root).expect("create repository");
    run_git(&root, &["init", "-q"]);
    let canonical_root = std::fs::canonicalize(&root).expect("canonical repository");

    let runner = GitRunner::for_cwd(&root).expect("runner for canonical cwd");
    let mut command = runner
        .command_for_cwd(&canonical_root)
        .expect("command for canonical cwd");
    command.args(["rev-parse", "--show-toplevel"]);
    let output = runner.output(command).expect("run Git from canonical cwd");
    assert!(
        output.status.success(),
        "Git rejected canonical cwd {}: {}",
        canonical_root.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::canonicalize(String::from_utf8_lossy(&output.stdout).trim())
            .expect("canonical Git root"),
        canonical_root
    );
}

#[test]
fn gitdir_terminal_worktree_root_is_never_promoted_to_metadata() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repo");
    run_git(fixture.path(), &["init", "--bare", path_text(&root)]);
    std::fs::write(root.join(".git"), "gitdir: .\n").expect("write self-root marker");
    assert_eq!(
        std::fs::canonicalize(run_git_stdout(&root, &["rev-parse", "--absolute-git-dir"]))
            .expect("canonical native self-root Git dir"),
        std::fs::canonicalize(&root).expect("canonical worktree root")
    );
    assert_unsafe_metadata_route(repository_authority_for_cwd(&root), &root.join(".git"));
}

#[cfg(unix)]
#[test]
fn commondir_routes_through_worktree_are_denied_before_path_selection() {
    use std::os::unix::fs::symlink;

    for identity_entry in [false, true] {
        let fixture = tempfile::tempdir().expect("fixture");
        let primary = fixture.path().join("primary");
        let linked = fixture.path().join("linked");
        let entry = fixture.path().join("entry");
        std::fs::create_dir_all(&primary).expect("create primary");
        run_git(&primary, &["init", "-q"]);
        run_git(
            &primary,
            &["worktree", "add", "--orphan", path_text(&linked)],
        );
        let admin = PathBuf::from({
            let mut command = Command::new("git");
            isolate_git_command_environment(&mut command);
            let output = command
                .args(["rev-parse", "--absolute-git-dir"])
                .current_dir(&linked)
                .output()
                .expect("resolve linked admin");
            assert!(output.status.success());
            String::from_utf8(output.stdout)
                .expect("UTF-8 linked admin")
                .trim()
                .to_string()
        });
        symlink(primary.join(".git"), linked.join("switch")).expect("worktree common-dir switch");
        let route = if identity_entry {
            symlink(&linked, &entry).expect("external entry into linked worktree");
            entry.join("switch")
        } else {
            linked.join("switch")
        };
        std::fs::write(admin.join("commondir"), format!("{}\n", route.display()))
            .expect("rewrite commondir route");
        run_git(&linked, &["rev-parse", "--git-common-dir"]);

        assert_unsafe_metadata_route(repository_authority_for_cwd(&linked), &linked.join(".git"));
    }
}

#[cfg(unix)]
#[test]
fn active_gitdir_route_is_revalidated_before_every_child() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repo");
    let admin = fixture.path().join("external-admin");
    std::fs::create_dir_all(&root).expect("create repository");
    run_git(
        fixture.path(),
        &[
            "init",
            "--separate-git-dir",
            path_text(&admin),
            path_text(&root),
        ],
    );
    let runner = GitRunner::for_cwd(&root).expect("runner for direct external gitdir");
    let mut first = runner.command_for_cwd(&root).expect("first Git command");
    first.args(["rev-parse", "--absolute-git-dir"]);
    assert!(
        runner
            .output(first)
            .expect("first Git child")
            .status
            .success()
    );

    symlink(&admin, root.join("switch")).expect("worktree gitdir switch");
    std::fs::write(root.join(".git"), "gitdir: switch\n").expect("retarget gitdir marker");
    let sentinel = fixture.path().join("git-child-ran");
    let mut second = runner.command();
    second
        .args(["config", "--file"])
        .arg(&sentinel)
        .args(["probe.value", "ran"]);
    let error = runner
        .output(second)
        .expect_err("retargeted gitdir must block second child");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(
        !sentinel.exists(),
        "Git child executed after gitdir retarget"
    );
}

#[cfg(unix)]
#[test]
fn active_commondir_route_is_revalidated_before_every_child() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let primary = fixture.path().join("primary");
    let linked = fixture.path().join("linked");
    std::fs::create_dir_all(&primary).expect("create primary");
    run_git(&primary, &["init", "-q"]);
    run_git(
        &primary,
        &["worktree", "add", "--orphan", path_text(&linked)],
    );
    let runner = GitRunner::for_cwd(&linked).expect("runner for linked worktree");
    let admin = runner
        .authority
        .active_git_dir()
        .expect("active linked metadata")
        .to_path_buf();
    let mut first = runner.command_for_cwd(&linked).expect("first Git command");
    first.args(["rev-parse", "--git-common-dir"]);
    assert!(
        runner
            .output(first)
            .expect("first Git child")
            .status
            .success()
    );

    symlink(primary.join(".git"), linked.join("switch")).expect("worktree common-dir switch");
    std::fs::write(
        admin.join("commondir"),
        format!("{}\n", linked.join("switch").display()),
    )
    .expect("retarget commondir marker");
    let sentinel = fixture.path().join("git-child-ran");
    let mut second = runner.command();
    second
        .args(["config", "--file"])
        .arg(&sentinel)
        .args(["probe.value", "ran"]);
    let error = runner
        .output(second)
        .expect_err("retargeted commondir must block second child");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(
        !sentinel.exists(),
        "Git child executed after commondir retarget"
    );
}

#[cfg(unix)]
#[test]
fn direct_external_separate_git_dir_absolute_and_relative_routes_execute_git() {
    for relative in [false, true] {
        let fixture = tempfile::tempdir().expect("fixture");
        let root = fixture.path().join("repo");
        let admin = fixture.path().join("external-admin");
        std::fs::create_dir_all(&root).expect("create repository");
        run_git(
            fixture.path(),
            &[
                "init",
                "--separate-git-dir",
                path_text(&admin),
                path_text(&root),
            ],
        );
        if relative {
            std::fs::write(root.join(".git"), "gitdir: ../external-admin\n")
                .expect("write relative gitdir marker");
        }
        run_git(&root, &["rev-parse", "--absolute-git-dir"]);
        let runner = GitRunner::for_cwd(&root).expect("separate-git-dir runner");
        let mut command = runner
            .command_for_cwd(&root)
            .expect("separate-git-dir command");
        command.args(["rev-parse", "--absolute-git-dir"]);
        crate::repository_authority::reset_bounded_marker_read_count();
        assert!(
            runner
                .output(command)
                .expect("separate-git-dir rev-parse")
                .status
                .success(),
            "relative={relative}"
        );
        assert_eq!(
            crate::repository_authority::bounded_marker_read_count(),
            1,
            "separate Git dir should reread only the active .git marker"
        );
    }
}

#[cfg(unix)]
#[test]
fn trusted_external_gitdir_symlink_chain_executes_git() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().join("repo");
    let external = fixture.path().join("external");
    let admin = external.join("admin");
    let alias_two = external.join("alias-two");
    let alias_one = external.join("alias-one");
    std::fs::create_dir_all(&root).expect("create repository");
    std::fs::create_dir_all(&external).expect("create external directory");
    run_git(&root, &["init", "-q"]);
    std::fs::rename(root.join(".git"), &admin).expect("move metadata external");
    symlink(&admin, &alias_two).expect("second trusted alias");
    symlink(&alias_two, &alias_one).expect("first trusted alias");
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", alias_one.display()),
    )
    .expect("write trusted-chain marker");
    run_git(&root, &["rev-parse", "--absolute-git-dir"]);

    let runner = GitRunner::for_cwd(&root).expect("trusted-chain runner");
    let mut command = runner
        .command_for_cwd(&root)
        .expect("trusted-chain command");
    command.args(["rev-parse", "--absolute-git-dir"]);
    assert!(
        runner
            .output(command)
            .expect("trusted-chain rev-parse")
            .status
            .success()
    );
}

#[test]
fn displaced_linked_admin_file_and_directory_routes_execute_git() {
    for directory_marker in [false, true] {
        for relative_common in [false, true] {
            let fixture = tempfile::tempdir().expect("fixture");
            let primary = fixture.path().join("primary");
            let linked = fixture.path().join("linked");
            let displaced = fixture.path().join("external/admin");
            std::fs::create_dir_all(&primary).expect("create primary");
            run_git(&primary, &["init", "-q"]);
            run_git(
                &primary,
                &["worktree", "add", "--orphan", path_text(&linked)],
            );
            let original_admin = PathBuf::from(run_git_stdout(
                &linked,
                &["rev-parse", "--absolute-git-dir"],
            ));
            let admin = if directory_marker {
                std::fs::remove_file(linked.join(".git")).expect("remove linked marker");
                std::fs::rename(&original_admin, linked.join(".git"))
                    .expect("move admin to directory marker");
                linked.join(".git")
            } else {
                std::fs::create_dir_all(displaced.parent().expect("displaced parent"))
                    .expect("create displaced parent");
                std::fs::rename(&original_admin, &displaced).expect("displace linked admin");
                std::fs::write(
                    linked.join(".git"),
                    format!("gitdir: {}\n", displaced.display()),
                )
                .expect("redirect linked marker");
                displaced.clone()
            };
            let common = if relative_common {
                "../../primary/.git".to_string()
            } else {
                primary.join(".git").display().to_string()
            };
            std::fs::write(admin.join("commondir"), format!("{common}\n"))
                .expect("rewrite displaced commondir");
            run_git(&linked, &["rev-parse", "--git-common-dir"]);

            let runner = GitRunner::for_cwd(&linked).expect("displaced-admin runner");
            let mut command = runner
                .command_for_cwd(&linked)
                .expect("displaced-admin command");
            command.args(["rev-parse", "--git-common-dir"]);
            crate::repository_authority::reset_bounded_marker_read_count();
            assert!(
                runner
                    .output(command)
                    .expect("displaced-admin rev-parse")
                    .status
                    .success(),
                "directory_marker={directory_marker} relative_common={relative_common}"
            );
            assert_eq!(
                crate::repository_authority::bounded_marker_read_count(),
                if directory_marker { 1 } else { 2 },
                "displaced active marker read count"
            );
        }
    }
}

#[test]
fn linked_worktree_rejects_git_from_main_and_linked_worktrees() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    let main_bin = main.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    write_git_candidate(&main_bin);
    write_runnable_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&linked).expect("untrusted locations");
    assert!(path_is_untrusted(
        &main_bin.join(git_executable_name()),
        &locations
    ));
    let runner = GitRunner::from_search_path(
        locations,
        &std::env::join_paths([&main_bin, &trusted_bin]).expect("PATH"),
    )
    .expect("linked-worktree runner");
    let mut command = runner
        .command_for_cwd(&linked)
        .expect("linked-worktree command");
    command.args(["rev-parse", "--show-toplevel"]);
    crate::repository_authority::reset_bounded_marker_read_count();
    assert!(
        runner
            .output(command)
            .expect("linked-worktree rev-parse")
            .status
            .success()
    );
    assert_eq!(
        crate::repository_authority::bounded_marker_read_count(),
        2,
        "linked worktree should reread active .git and commondir markers"
    );
}

#[test]
fn registered_sibling_remains_untrusted_without_its_worktree_marker_or_root() {
    let fixture = tempdir_for_native_git();
    let main = fixture.path().join("main");
    let sibling = fixture.path().join("sibling");
    let sibling_bin = sibling.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&sibling)]);
    write_git_candidate(&sibling_bin);
    write_git_candidate(&trusted_bin);

    for state in ["intact", "marker removed", "root recreated"] {
        if state == "marker removed" {
            std::fs::remove_file(sibling.join(".git")).expect("remove sibling .git marker");
        } else if state == "root recreated" {
            std::fs::remove_dir_all(&sibling).expect("remove sibling root");
            write_git_candidate(&sibling_bin);
        }
        let locations = repository_authority_for_cwd(&main).expect("untrusted locations");
        assert!(
            path_is_untrusted(&sibling_bin.join(git_executable_name()), &locations),
            "registered sibling Git became trusted in state {state}"
        );
        assert_eq!(
            selected_git(&locations, &[&sibling_bin, &trusted_bin]),
            trusted_bin.join(git_executable_name()),
            "registered sibling Git selected in state {state}"
        );
    }
}

#[test]
fn git_generated_relative_registry_route_is_accepted() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    let admin = PathBuf::from(run_git_stdout(
        &linked,
        &["rev-parse", "--absolute-git-dir"],
    ));
    let relative_marker = PathBuf::from("..")
        .join("..")
        .join("..")
        .join("..")
        .join("linked")
        .join(".git");
    std::fs::write(
        admin.join("gitdir"),
        format!("{}\n", relative_marker.display()),
    )
    .expect("write Git-style relative registry marker");

    let authority = repository_authority_for_cwd(&main).expect("relative registry authority");
    assert!(
        authority
            .contains_root(&std::fs::canonicalize(&linked).expect("canonical linked worktree"))
    );
    GitRunner::for_cwd(&linked).expect("relative linked-worktree runner");
}

#[test]
fn registered_worktree_route_rejects_canceled_worktree_descendant() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    std::fs::create_dir_all(main.join("pivot")).expect("create worktree pivot");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    let admin = PathBuf::from(run_git_stdout(
        &linked,
        &["rev-parse", "--absolute-git-dir"],
    ));
    let registry_marker = admin.join("gitdir");
    let raw_marker = main.join("pivot").join("..").join("..").join("linked/.git");
    std::fs::write(&registry_marker, format!("{}\n", raw_marker.display()))
        .expect("write registry route through worktree pivot");

    assert_unsafe_registry_route(repository_authority_for_cwd(&main), &registry_marker);
    let error = GitRunner::for_cwd_io(&main).expect_err("unsafe registry route");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn registered_route_rejects_root_that_is_also_outer_worktree_descendant() {
    let fixture = tempfile::tempdir().expect("fixture");
    let route = overlapping_registered_route(fixture.path());

    assert_eq!(
        std::fs::canonicalize(&route.raw_marker).expect("canonical linked marker"),
        std::fs::canonicalize(route.linked.join(".git")).expect("canonical expected marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );
}

#[test]
fn registered_route_allows_parent_cancellation_inside_protected_metadata() {
    let fixture = tempfile::tempdir().expect("fixture");
    let route = metadata_alias_registered_route(fixture.path(), "main", "main");
    let protected_marker = route.main.join(".git").join("..").join("linked/.git");
    std::fs::write(
        &route.registry_marker,
        format!("{}\n", protected_marker.display()),
    )
    .expect("write protected-metadata registry route");

    assert_eq!(
        std::fs::canonicalize(&protected_marker).expect("canonical protected route"),
        std::fs::canonicalize(route.linked.join(".git")).expect("canonical linked marker")
    );
    repository_authority_for_cwd(&route.main).expect("protected-metadata registry authority");
    GitRunner::for_cwd(&route.linked).expect("linked runner through protected metadata route");
}

#[cfg(unix)]
#[test]
fn metadata_alias_registered_route_is_rejected_before_and_after_symlink_retarget() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let route = metadata_alias_registered_route(fixture.path(), "main", "main");
    symlink(route.main.join(".git"), &route.pivot).expect("alias pivot to protected metadata");

    assert_eq!(
        std::fs::canonicalize(&route.raw_marker).expect("canonical metadata-alias route"),
        std::fs::canonicalize(route.linked.join(".git")).expect("canonical linked marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );

    std::fs::remove_file(&route.pivot).expect("remove metadata alias");
    let attacker_target = fixture.path().join("attacker/deep");
    let attacker_linked = fixture.path().join("attacker/linked");
    std::fs::create_dir_all(&attacker_target).expect("create attacker pivot target");
    std::fs::create_dir_all(&attacker_linked).expect("create attacker linked root");
    std::fs::write(
        attacker_linked.join(".git"),
        format!("gitdir: {}\n", route.linked_admin.display()),
    )
    .expect("write attacker backlink");
    symlink(&attacker_target, &route.pivot).expect("retarget metadata alias");

    assert_eq!(
        std::fs::canonicalize(&route.raw_marker).expect("canonical retargeted route"),
        std::fs::canonicalize(attacker_linked.join(".git")).expect("canonical attacker marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );
}

#[cfg(windows)]
#[test]
fn registered_route_rejects_unicode_case_junction_retarget() {
    let fixture = tempfile::tempdir().expect("fixture");
    let route = metadata_alias_registered_route(fixture.path(), "Répo", "RÉPO");
    let raw_marker = fixture.path().join("RÉPO").join("pivot").join(".git");
    std::fs::write(
        &route.registry_marker,
        format!("{}\n", raw_marker.display()),
    )
    .expect("write direct junction registry route");
    create_junction(&route.pivot, &route.linked);

    assert_eq!(
        std::fs::canonicalize(&raw_marker).expect("canonical Unicode junction route"),
        std::fs::canonicalize(route.linked.join(".git")).expect("canonical linked marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );

    std::fs::remove_dir(&route.pivot).expect("remove metadata junction");
    let attacker_linked = fixture.path().join("attacker/linked");
    std::fs::create_dir_all(&attacker_linked).expect("create attacker linked root");
    std::fs::write(
        attacker_linked.join(".git"),
        format!("gitdir: {}\n", route.linked_admin.display()),
    )
    .expect("write attacker backlink");
    create_junction(&route.pivot, &attacker_linked);

    assert_eq!(
        std::fs::canonicalize(&raw_marker).expect("canonical retargeted route"),
        std::fs::canonicalize(attacker_linked.join(".git")).expect("canonical attacker marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );
}

#[cfg(unix)]
#[test]
fn overlapping_registered_route_remains_rejected_after_symlink_retarget() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let route = overlapping_registered_route(fixture.path());
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );

    std::fs::remove_dir_all(&route.nested).expect("remove nested worktree");
    let attacker_parent = fixture.path().join("attacker/deep");
    let attacker_nested = attacker_parent.join("nested");
    let attacker_linked = attacker_parent.join("linked");
    std::fs::create_dir_all(&attacker_nested).expect("create attacker nested root");
    std::fs::create_dir_all(&attacker_linked).expect("create attacker linked root");
    std::fs::write(
        attacker_nested.join(".git"),
        format!("gitdir: {}\n", route.nested_admin.display()),
    )
    .expect("write attacker nested backlink");
    std::fs::write(
        attacker_linked.join(".git"),
        format!("gitdir: {}\n", route.linked_admin.display()),
    )
    .expect("write attacker linked backlink");
    symlink(&attacker_nested, &route.nested).expect("retarget nested worktree root");

    assert_eq!(
        std::fs::canonicalize(&route.raw_marker).expect("canonical retargeted marker"),
        std::fs::canonicalize(attacker_linked.join(".git")).expect("canonical attacker marker")
    );
    let nested_registry_marker = route.nested_admin.join("gitdir");
    assert_unsafe_registry_route_at_one_of(
        repository_authority_for_cwd(&route.main),
        &[&route.registry_marker, &nested_registry_marker],
    );
}

#[cfg(windows)]
#[test]
fn overlapping_registered_route_remains_rejected_after_junction_retarget() {
    let fixture = tempfile::tempdir().expect("fixture");
    let route = overlapping_registered_route(fixture.path());
    let pivot = route.nested.join("pivot");
    let raw_marker = pivot.join("linked/.git");
    std::fs::write(
        &route.registry_marker,
        format!("{}\n", raw_marker.display()),
    )
    .expect("write direct overlapping registry route");
    create_junction(&pivot, &route.main);

    assert_eq!(
        std::fs::canonicalize(&raw_marker).expect("canonical overlapping junction route"),
        std::fs::canonicalize(route.linked.join(".git")).expect("canonical linked marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );

    std::fs::remove_dir(&pivot).expect("remove overlapping junction");
    let attacker_parent = fixture.path().join("attacker/deep");
    let attacker_linked = attacker_parent.join("linked");
    std::fs::create_dir_all(&attacker_linked).expect("create attacker linked root");
    std::fs::write(
        attacker_linked.join(".git"),
        format!("gitdir: {}\n", route.linked_admin.display()),
    )
    .expect("write attacker linked backlink");
    create_junction(&pivot, &attacker_parent);

    assert_eq!(
        std::fs::canonicalize(&raw_marker).expect("canonical retargeted marker"),
        std::fs::canonicalize(attacker_linked.join(".git")).expect("canonical attacker marker")
    );
    assert_unsafe_registry_route(
        repository_authority_for_cwd(&route.main),
        &route.registry_marker,
    );
}

#[cfg(unix)]
#[test]
fn registered_worktree_route_remains_rejected_after_pivot_retarget() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    let attacker = fixture.path().join("attacker");
    std::fs::create_dir_all(main.join("pivot")).expect("create worktree pivot");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    let admin = PathBuf::from(run_git_stdout(
        &linked,
        &["rev-parse", "--absolute-git-dir"],
    ));
    let registry_marker = admin.join("gitdir");
    let raw_marker = main.join("pivot").join("..").join("..").join("linked/.git");
    std::fs::write(&registry_marker, format!("{}\n", raw_marker.display()))
        .expect("write registry route through worktree pivot");
    assert_unsafe_registry_route(repository_authority_for_cwd(&main), &registry_marker);

    std::fs::remove_dir(main.join("pivot")).expect("remove ordinary pivot");
    let attacker_target = attacker.join("deep/nested");
    let attacker_worktree = attacker.join("linked");
    std::fs::create_dir_all(&attacker_target).expect("create attacker pivot target");
    std::fs::create_dir_all(&attacker_worktree).expect("create attacker worktree");
    std::fs::write(
        attacker_worktree.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .expect("write attacker backlink");
    symlink(&attacker_target, main.join("pivot")).expect("retarget worktree pivot");

    assert_unsafe_registry_route(repository_authority_for_cwd(&main), &registry_marker);
    assert!(matches!(
        GitRunner::for_cwd(&main),
        Err(GitReadError::UnsafeRepositoryMetadata { .. })
    ));
}

#[test]
fn registered_worktree_backlink_mismatch_is_unsafe() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    let marker = linked.join(".git");
    std::fs::write(
        &marker,
        format!("gitdir: {}\n", main.join(".git").display()),
    )
    .expect("rewrite linked-worktree backlink");

    match repository_authority_for_cwd(&main) {
        Err(GitReadError::UnsafeRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, &marker);
            assert_eq!(reason, "Git worktree registry backlink mismatch");
        }
        other => panic!("expected unsafe registry backlink, got {other:?}"),
    }
    let error = GitRunner::for_cwd_io(&main).expect_err("mismatched worktree backlink");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("backlink mismatch"), "{error}");
    assert!(!error.to_string().contains("PATH"), "{error}");
}

#[test]
fn unsupported_worktree_registry_entry_is_invalid_with_entry_path() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    let registry = main.join(".git/worktrees");
    std::fs::create_dir_all(&registry).expect("create worktree registry");
    let entry = registry.join("unsupported-entry");
    std::fs::write(&entry, "not a registry directory\n").expect("write invalid registry entry");

    match repository_authority_for_cwd(&main) {
        Err(GitReadError::InvalidRepositoryMetadata { path, reason }) => {
            assert_same_affected_path(&path, &entry);
            assert_eq!(reason, "unsupported Git worktree registry entry");
        }
        other => panic!("expected invalid registry entry, got {other:?}"),
    }
    let error = GitRunner::for_cwd_io(&main).expect_err("unsupported registry entry");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("unsupported Git worktree registry entry"),
        "{error}"
    );
    assert!(!error.to_string().contains("PATH"), "{error}");
}

#[test]
fn per_child_active_route_revalidation_is_independent_of_registered_sibling_count() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let active = fixture.path().join("active");
    std::fs::create_dir_all(&main).expect("create primary");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&active)]);
    for index in 0..12 {
        let sibling = fixture.path().join(format!("sibling-{index}"));
        run_git(&main, &["worktree", "add", "--orphan", path_text(&sibling)]);
    }

    let runner = GitRunner::for_cwd(&active).expect("runner with many registered siblings");
    crate::repository_authority::reset_bounded_marker_read_count();
    let mut command = runner
        .command_for_cwd(&active)
        .expect("active worktree command");
    command.args(["rev-parse", "--git-common-dir"]);
    assert!(
        runner
            .output(command)
            .expect("active worktree rev-parse")
            .status
            .success()
    );
    assert_eq!(
        crate::repository_authority::bounded_marker_read_count(),
        2,
        "per-child validation must read only active .git and commondir markers"
    );
}

#[test]
fn nested_repository_rejects_git_from_enclosing_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let nested = outer.join("nested");
    let outer_bin = outer.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&nested).expect("create nested repository");
    run_git(&outer, &["init", "-q"]);
    run_git(&nested, &["init", "-q"]);
    write_git_candidate(&outer_bin);
    write_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&nested).expect("untrusted locations");
    assert!(
        path_is_untrusted(&outer_bin.join(git_executable_name()), &locations),
        "Git from an enclosing repository must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&outer_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[cfg(unix)]
#[test]
fn symlinked_nested_repository_rejects_git_from_lexical_enclosing_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let physical_nested = fixture.path().join("physical-nested");
    let lexical_nested = outer.join("nested");
    let outer_bin = outer.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&outer).expect("create outer repository");
    std::fs::create_dir_all(&physical_nested).expect("create physical nested repository");
    run_git(&outer, &["init", "-q"]);
    run_git(&physical_nested, &["init", "-q"]);
    std::os::unix::fs::symlink(&physical_nested, &lexical_nested)
        .expect("symlink nested repository");
    write_git_candidate(&outer_bin);
    write_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&lexical_nested).expect("untrusted locations");
    assert!(
        path_is_untrusted(&outer_bin.join(git_executable_name()), &locations),
        "Git from the lexical enclosing repository must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&outer_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn nested_repository_rejects_git_from_enclosing_repository_main_worktree() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let linked = fixture.path().join("linked");
    let nested = linked.join("nested");
    let main_bin = main.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&main).expect("create main worktree");
    run_git(&main, &["init", "-q"]);
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    std::fs::create_dir_all(&nested).expect("create nested repository");
    run_git(&nested, &["init", "-q"]);
    write_git_candidate(&main_bin);
    write_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&nested).expect("untrusted locations");
    assert!(
        path_is_untrusted(&main_bin.join(git_executable_name()), &locations),
        "all worktrees of an enclosing repository must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&main_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn submodule_rejects_git_from_enclosing_superproject() {
    let fixture = tempfile::tempdir().expect("fixture");
    let source = fixture.path().join("source");
    let outer = fixture.path().join("outer");
    let submodule = outer.join("nested");
    let outer_bin = outer.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&source).expect("create source repository");
    std::fs::create_dir_all(&outer).expect("create superproject");
    run_git(&source, &["init", "-q"]);
    std::fs::write(source.join("source.txt"), "source\n").expect("write source file");
    run_git(&source, &["add", "source.txt"]);
    commit_all(&source, "source");
    run_git(&outer, &["init", "-q"]);
    std::fs::write(outer.join("outer.txt"), "outer\n").expect("write outer file");
    run_git(&outer, &["add", "outer.txt"]);
    commit_all(&outer, "outer");
    run_git(
        &outer,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            path_text(&source),
            "nested",
        ],
    );
    write_git_candidate(&outer_bin);
    write_runnable_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&submodule).expect("untrusted locations");
    assert!(
        path_is_untrusted(&outer_bin.join(git_executable_name()), &locations),
        "Git from a superproject must remain repository-controlled"
    );
    assert_eq!(
        selected_git(&locations, &[&outer_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
    let runner = GitRunner::from_search_path(
        locations,
        &std::env::join_paths([&outer_bin, &trusted_bin]).expect("PATH"),
    )
    .expect("submodule runner");
    let mut command = runner
        .command_for_cwd(&submodule)
        .expect("submodule command");
    command.args(["rev-parse", "--show-toplevel"]);
    crate::repository_authority::reset_bounded_marker_read_count();
    assert!(
        runner
            .output(command)
            .expect("submodule rev-parse")
            .status
            .success()
    );
    assert_eq!(
        crate::repository_authority::bounded_marker_read_count(),
        1,
        "submodule should reread only its active .git marker"
    );
}

#[test]
fn bare_backed_linked_worktree_allows_external_git_in_sibling_directory() {
    let fixture = tempfile::tempdir().expect("fixture");
    let bare = fixture.path().join("repository.git");
    let linked = fixture.path().join("linked");
    let trusted_bin = fixture.path().join("trusted-bin");
    run_git(fixture.path(), &["init", "--bare", path_text(&bare)]);
    run_git(
        fixture.path(),
        &[
            "--git-dir",
            path_text(&bare),
            "worktree",
            "add",
            "--orphan",
            path_text(&linked),
        ],
    );
    write_runnable_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&linked).expect("untrusted locations");
    assert_eq!(
        selected_git(&locations, &[&trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
    let runner = GitRunner::from_search_path(
        locations,
        &std::env::join_paths([&trusted_bin]).expect("PATH"),
    )
    .expect("bare-backed runner");
    let mut command = runner
        .command_for_cwd(&linked)
        .expect("bare-backed command");
    command.args(["rev-parse", "--git-common-dir"]);
    crate::repository_authority::reset_bounded_marker_read_count();
    assert!(
        runner
            .output(command)
            .expect("bare-backed rev-parse")
            .status
            .success()
    );
    assert_eq!(
        crate::repository_authority::bounded_marker_read_count(),
        2,
        "bare-backed linked worktree should reread two active markers"
    );
}

#[test]
fn linked_common_dir_nested_in_another_repo_is_rejected_before_path_selection() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("outer");
    let common = outer.join("nested-common.git");
    let linked = fixture.path().join("linked");
    run_git(fixture.path(), &["init", path_text(&outer)]);
    std::fs::write(outer.join("seed.txt"), "seed\n").expect("write outer seed");
    run_git(&outer, &["add", "seed.txt"]);
    commit_all(&outer, "outer seed");
    run_git(&outer, &["clone", "--bare", ".", path_text(&common)]);
    run_git(
        fixture.path(),
        &[
            "--git-dir",
            path_text(&common),
            "worktree",
            "add",
            "-b",
            "nested-common-linked",
            path_text(&linked),
            "HEAD",
        ],
    );
    assert_unsafe_metadata_route(repository_authority_for_cwd(&linked), &linked.join(".git"));
}

#[test]
fn separate_dot_git_dir_rejects_main_candidate_and_allows_unrelated_repo_candidate() {
    let fixture = tempfile::tempdir().expect("fixture");
    let main = fixture.path().join("main");
    let common_dir = fixture.path().join("git-storage/.git");
    let linked = fixture.path().join("linked");
    let main_bin = main.join("bin");
    let unrelated = fixture.path().join("unrelated");
    let unrelated_bin = unrelated.join("bin");
    let malformed = fixture.path().join("malformed");
    let malformed_bin = malformed.join("bin");
    std::fs::create_dir_all(&main).expect("create main worktree");
    std::fs::create_dir_all(common_dir.parent().expect("common-dir parent"))
        .expect("create common-dir parent");
    run_git(
        fixture.path(),
        &[
            "init",
            "--separate-git-dir",
            path_text(&common_dir),
            path_text(&main),
        ],
    );
    run_git(&main, &["worktree", "add", "--orphan", path_text(&linked)]);
    run_git(fixture.path(), &["init", path_text(&unrelated)]);
    write_git_candidate(&main_bin);
    write_git_candidate(&unrelated_bin);
    write_git_candidate(&malformed_bin);
    std::fs::write(malformed.join(".git"), "not a gitdir").expect("malformed marker");

    let locations = repository_authority_for_cwd(&linked).expect("untrusted locations");
    assert_eq!(
        selected_git(&locations, &[&main_bin, &malformed_bin, &unrelated_bin]),
        unrelated_bin.join(git_executable_name())
    );
}

#[cfg(unix)]
#[test]
fn unproven_separate_primary_is_denied_before_any_path_git_executes() {
    let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
    for authority_config in ["non-bare", "included false", "bare 08", "bare overflow"] {
        let fixture = tempfile::tempdir_in(&temp_base).expect("fixture");
        let primary = fixture.path().join("primary");
        let common = fixture.path().join("separate-common");
        let linked = fixture.path().join("linked");
        let primary_bin = primary.join("bin");
        let trusted_bin = fixture.path().join("trusted-bin");
        let executed = fixture.path().join("attacker-git-ran");
        std::fs::create_dir_all(&primary).expect("create primary");
        run_git(
            fixture.path(),
            &[
                "init",
                "--separate-git-dir",
                path_text(&common),
                path_text(&primary),
            ],
        );
        std::fs::write(primary.join("seed"), "seed\n").expect("write seed");
        run_git(&primary, &["add", "seed"]);
        commit_all(&primary, "seed");
        run_git(
            &primary,
            &[
                "worktree",
                "add",
                "-b",
                "unproven-primary-linked",
                path_text(&linked),
            ],
        );
        if authority_config == "included false" {
            let included = primary.join("bare-override.config");
            std::fs::write(&included, "[core]\n\tbare = false\n")
                .expect("write included bare override");
            let mut config =
                std::fs::read_to_string(common.join("config")).expect("read common config");
            config.push_str(&format!(
                "\n[core]\n\tbare = true\n[include]\n\tpath = {}\n",
                included.display()
            ));
            std::fs::write(common.join("config"), config).expect("write ambiguous bare config");
        } else if let Some(value) = match authority_config {
            "bare 08" => Some("08"),
            "bare overflow" => Some("2147483648"),
            _ => None,
        } {
            let mut config =
                std::fs::read_to_string(common.join("config")).expect("read common config");
            config.push_str(&format!("\n[core]\n\tbare = {value}\n"));
            std::fs::write(common.join("config"), config).expect("write malformed bare config");
        }
        write_git_candidate(&trusted_bin);
        std::fs::create_dir_all(&primary_bin).expect("create primary bin");
        std::fs::copy("/usr/bin/touch", primary_bin.join("git"))
            .expect("copy attacker native executable");
        std::fs::remove_file(primary.join(".git")).expect("remove primary reverse marker");

        let locations = repository_authority_for_cwd(&linked).expect("untrusted locations");
        let path = std::env::join_paths([&primary_bin, &trusted_bin]).expect("PATH");
        let error = match GitRunner::from_search_path(locations, &path) {
            Ok(runner) => {
                let mut command = runner.command();
                command.arg(&executed);
                let _ = runner.output(command);
                panic!("unproven separate primary selected a PATH Git");
            }
            Err(error) => error,
        };
        assert!(!executed.exists(), "attacker Git executed");
        assert!(matches!(
            error,
            GitReadError::UnprovenPrimaryAuthority { .. }
        ));
    }
}

#[cfg(unix)]
#[test]
fn nested_repo_inside_linked_outer_denies_unproven_outer_primary_before_path_git() {
    let temp_base = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp dir");
    let fixture = tempfile::tempdir_in(temp_base).expect("fixture");
    let primary = fixture.path().join("primary");
    let common = fixture.path().join("separate-common");
    let outer = fixture.path().join("outer");
    let nested = outer.join("nested");
    let primary_bin = primary.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    let executed = fixture.path().join("attacker-git-ran");
    std::fs::create_dir_all(&primary).expect("create primary");
    run_git(
        fixture.path(),
        &[
            "init",
            "--separate-git-dir",
            path_text(&common),
            path_text(&primary),
        ],
    );
    std::fs::write(primary.join("seed"), "seed\n").expect("write seed");
    run_git(&primary, &["add", "seed"]);
    commit_all(&primary, "seed");
    run_git(
        &primary,
        &[
            "worktree",
            "add",
            "-b",
            "nested-unproven-outer",
            path_text(&outer),
        ],
    );
    std::fs::create_dir_all(&nested).expect("create nested repository");
    run_git(&nested, &["init", "-q"]);
    write_git_candidate(&trusted_bin);
    std::fs::create_dir_all(&primary_bin).expect("create primary bin");
    std::fs::copy("/usr/bin/touch", primary_bin.join("git"))
        .expect("copy attacker native executable");
    std::fs::remove_file(primary.join(".git")).expect("remove primary reverse marker");

    let locations = repository_authority_for_cwd(&nested).expect("untrusted locations");
    let path = std::env::join_paths([&primary_bin, &trusted_bin]).expect("PATH");
    let error = match GitRunner::from_search_path(locations, &path) {
        Ok(runner) => {
            let mut command = runner.command();
            command.arg(&executed);
            let _ = runner.output(command);
            panic!("unproven enclosing linked repository selected a PATH Git");
        }
        Err(error) => error,
    };
    assert!(!executed.exists(), "attacker Git executed");
    assert!(matches!(
        error,
        GitReadError::UnprovenPrimaryAuthority { .. }
    ));
}

#[test]
fn explicit_absolute_core_worktree_is_recorded_before_path_selection() {
    let fixture = tempdir_for_native_git();
    let primary = fixture.path().join("primary");
    let common = fixture.path().join("separate-common");
    let linked = fixture.path().join("linked");
    let primary_bin = primary.join("bin");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&primary).expect("create primary");
    run_git(
        fixture.path(),
        &[
            "init",
            "--separate-git-dir",
            path_text(&common),
            path_text(&primary),
        ],
    );
    std::fs::write(primary.join("seed"), "seed\n").expect("write seed");
    run_git(&primary, &["add", "seed"]);
    commit_all(&primary, "seed");
    run_git(
        &primary,
        &[
            "worktree",
            "add",
            "-b",
            "explicit-core-worktree-linked",
            path_text(&linked),
        ],
    );
    let mut config = std::fs::read_to_string(common.join("config")).expect("read common config");
    config.push_str(&format!(
        "\n[core]\n\tworktree = {}\n",
        git_config_path(&primary)
    ));
    std::fs::write(common.join("config"), config).expect("write explicit core.worktree");
    std::fs::remove_file(primary.join(".git")).expect("remove primary reverse marker");
    write_git_candidate(&primary_bin);
    write_git_candidate(&trusted_bin);

    let locations = repository_authority_for_cwd(&linked).expect("untrusted locations");
    assert!(
        locations.contains_root(&primary),
        "explicit core.worktree was not recorded"
    );
    assert_eq!(
        selected_git(&locations, &[&primary_bin, &trusted_bin]),
        trusted_bin.join(git_executable_name())
    );
}

#[test]
fn resolver_rejects_parent_traversal_spelled_through_repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let trusted_bin = fixture.path().join("trusted-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&trusted_bin);

    let locations = locations_for_root(&repo);
    for root in locations.roots() {
        // Append without PathBuf::push: it resolves `..` when `root` has a
        // verbatim Windows prefix, before the resolver can inspect the PATH
        // spelling.
        let traversing_path = raw_parent_traversal(root, "trusted-bin");
        let search_path = std::env::join_paths([&traversing_path]).expect("PATH");
        let split_paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
        assert_eq!(split_paths, vec![traversing_path.clone()]);
        assert!(
            search_directory_is_untrusted(&split_paths[0], &locations),
            "raw PATH traversal was not rejected from {root:?}"
        );

        assert!(
            matches!(
                select_git_executable(&locations, &search_path),
                Err(GitReadError::NoTrustedGit)
            ),
            "resolver accepted parent traversal from {root:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn resolver_rejects_parent_traversal_across_windows_namespaces() {
    let traversing = [
        r"C:\Repo\..\outside",
        r"\\?\C:\Repo\..\outside",
        r"\\Server\Share\Repo\..\outside",
        r"\\?\UNC\Server\Share\Repo\..\outside",
        r"\\?\unc\Server\Share\Repo\..\outside",
        r"\\.\C:\Repo\..\outside",
        r"\\.\UNC\Server\Share\Repo\..\outside",
        r"\\?\C:\RÉPO\..\outside",
    ];
    for path in traversing {
        assert!(
            windows_path_requires_fail_closed(Path::new(path)),
            "parent traversal was accepted: {path:?}"
        );
    }

    let normalized_external = [
        r"C:\outside",
        r"\\?\C:\outside",
        r"\\Server\Share\outside",
        r"\\?\UNC\Server\Share\outside",
        r"\\?\unc\Server\Share\outside",
        r"\\.\C:\outside",
        r"\\.\UNC\Server\Share\outside",
    ];
    for path in normalized_external {
        assert!(
            !windows_path_requires_fail_closed(Path::new(path)),
            "normalized filesystem path was rejected: {path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn resolver_rejects_unicode_case_alias_through_repository_junction() {
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("Répo");
    let outside = fixture.path().join("outside");
    let junction = repo.join("git-bin");
    std::fs::create_dir_all(&repo).expect("create repository");
    write_git_candidate(&outside);
    create_junction(&junction, &outside);

    let case_alias = fixture.path().join("RÉPO").join("git-bin");
    let verbatim_case_alias = PathBuf::from(format!(r"\\?\{}", case_alias.display()));
    assert_eq!(
        std::fs::canonicalize(&verbatim_case_alias).expect("canonical alias"),
        std::fs::canonicalize(&outside).expect("canonical outside")
    );

    let locations = locations_for_root(&repo);
    assert!(
        path_is_untrusted(&verbatim_case_alias, &locations),
        "route observation missed the Unicode repository alias"
    );
    assert!(search_directory_is_untrusted(
        &verbatim_case_alias,
        &locations
    ));
    let search_path = std::env::join_paths([verbatim_case_alias]).expect("PATH");
    assert!(matches!(
        GitRunner::from_search_path(locations, &search_path),
        Err(GitReadError::NoTrustedGit)
    ));
}

#[cfg(windows)]
#[test]
fn resolver_fails_closed_for_unsupported_windows_device_namespaces() {
    let unsupported = [
        r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1\git.exe",
        r"\\?\Volume{11111111-1111-1111-1111-111111111111}\git.exe",
        r"\\.\PhysicalDrive0",
        r"\\.\pipe\codex-git",
    ];

    for path in unsupported {
        assert!(
            windows_path_requires_fail_closed(Path::new(path)),
            "unsupported namespace was trusted: {path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn resolver_selects_native_git_exe_only() {
    let mixed_case = Path::new(r"C:\Repo\.git");
    let lower_case = Path::new(r"c:\repo\.GIT");
    assert!(crate::git_config::path_is_within(mixed_case, lower_case));
    assert!(crate::git_config::path_is_within(lower_case, mixed_case));
    let fixture = tempfile::tempdir().expect("fixture");
    let repo = fixture.path().join("repo");
    let scripts = fixture.path().join("scripts");
    let native = fixture.path().join("native");
    std::fs::create_dir_all(&repo).expect("repo");
    std::fs::create_dir_all(&scripts).expect("scripts");
    std::fs::create_dir_all(&native).expect("native");
    std::fs::write(scripts.join("git.cmd"), "@exit /b 0\r\n").expect("script");
    std::fs::copy(native_git_fixture(), native.join("git.exe")).expect("native executable fixture");
    let locations = locations_for_root(&repo);
    let path = std::env::join_paths([scripts, native.clone()]).expect("PATH");
    let runner = GitRunner::from_search_path(locations, &path).expect("native Git");
    assert_eq!(runner.argv0, native.join("git.exe"));
    assert_eq!(
        runner.executable,
        std::fs::canonicalize(native.join("git.exe")).expect("canonical native Git")
    );
}
