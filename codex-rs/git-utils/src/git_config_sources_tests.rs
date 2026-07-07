use super::*;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;

fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut command = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut command);
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

fn run_success(cwd: &Path, args: &[&str]) -> String {
    let (code, stdout, stderr) = run(cwd, args);
    assert_eq!(code, 0, "git {args:?}: {stderr}");
    stdout.trim().to_string()
}

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("tempdir");
    init_repo_at(repo.path());
    repo
}

fn init_repo_at(root: &Path) {
    std::fs::create_dir_all(root).expect("create repository directory");
    run_success(root, &["init"]);
    run_success(root, &["config", "user.email", "codex@example.com"]);
    run_success(root, &["config", "user.name", "Codex"]);
    std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
    run_success(root, &["add", "file.txt"]);
    run_success(root, &["commit", "-m", "seed"]);
}

#[cfg(windows)]
#[allow(dead_code)]
fn create_junction(path: &Path, target: &Path) {
    // Bazel's GNU Windows runner can surface temporary paths with `/`
    // separators. `mklink` treats those separators as option prefixes, so
    // pass native separators to the cmd.exe built-in.
    let path = path.as_os_str().to_string_lossy().replace('/', "\\");
    let target = target.as_os_str().to_string_lossy().replace('/', "\\");
    let output = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("create junction");
    assert!(
        output.status.success(),
        "mklink failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn guard(root: &Path) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(root)?;
    ensure_no_worktree_config_sources(&git, root, &[])
}

#[test]
fn blocking_source_authorization_is_safe_inside_futures_local_pool() {
    let repo = init_repo();
    let git = GitRunner::for_cwd_io(repo.path()).expect("Git runner");

    futures::executor::block_on(async {
        ensure_no_worktree_config_sources(&git, repo.path(), &[])
            .expect("blocking traversal must not enter a nested executor");
    });
}

fn add_include(root: &Path, key: &str, value: &str) {
    run_success(root, &["config", "--add", key, value]);
}

fn assert_worktree_rejection(error: io::Error) {
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(error.to_string().contains("worktree-controlled"), "{error}");
}

#[cfg(target_os = "linux")]
fn assert_process_relative_include_rejection(error: io::Error) {
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(
        error
            .to_string()
            .contains("process-relative Git config include"),
        "{error}"
    );
}

fn run_isolated_source_test(test_name: &str, env: &[(&str, &OsStr)], removed: &[&str]) {
    let cwd = std::env::current_dir().expect("current directory");
    run_isolated_source_test_from(test_name, env, removed, &cwd);
}

fn run_isolated_source_test_from(
    test_name: &str,
    env: &[(&str, &OsStr)],
    removed: &[&str],
    cwd: &Path,
) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    crate::safe_git::isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_CONFIG_SOURCE_CHILD", "1")
        .env("RUST_TEST_THREADS", "1")
        .current_dir(cwd);
    for (name, value) in env {
        command.env(name, value);
    }
    for name in removed {
        command.env_remove(name);
    }
    let output = command.output().expect("run isolated source test");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_process_relative_primary_config_sources_before_git_changes_cwd() {
    use std::os::unix::fs::symlink;

    const TEST_NAME: &str = "git_config_sources::tests::rejects_process_relative_primary_config_sources_before_git_changes_cwd";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let fixture = tempfile::tempdir().expect("fixture");
        let root = fixture.path().join("repo");
        let external = fixture.path().join("external");
        let alias = fixture.path().join("config-alias");
        let chained_alias = fixture.path().join("chained-alias");
        init_repo_at(&root);
        std::fs::create_dir(&external).expect("external directory");
        std::fs::write(
            external.join("decoy.gitconfig"),
            "[safe]\nvalue = external\n",
        )
        .expect("external decoy");
        std::fs::write(
            external.join(".gitconfig"),
            "[safe]\nvalue = external-home\n",
        )
        .expect("external HOME config");
        for leaf in ["decoy.gitconfig", "missing-parent.gitconfig"] {
            std::fs::write(root.join(leaf), "[unsafe]\nhelper = worktree\n")
                .expect("worktree config");
        }
        symlink("/proc/self/cwd", &alias).expect("procfs alias");
        symlink("config-alias", &chained_alias).expect("chained procfs alias");

        let candidates = [
            PathBuf::from("/proc/self/cwd/decoy.gitconfig"),
            PathBuf::from("/proc/thread-self/cwd/decoy.gitconfig"),
            PathBuf::from("/proc/self/cwd/missing-parent.gitconfig"),
            alias.join("decoy.gitconfig"),
            chained_alias.join("decoy.gitconfig"),
        ];
        for candidate in &candidates {
            run_isolated_source_test_from(
                TEST_NAME,
                &[
                    ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                    ("GIT_CONFIG_GLOBAL", candidate.as_os_str()),
                    ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                ],
                &["GIT_CONFIG_SYSTEM"],
                &external,
            );
        }
        run_isolated_source_test_from(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("GIT_CONFIG_GLOBAL", OsStr::new("")),
                (
                    "GIT_CONFIG_SYSTEM",
                    OsStr::new("/proc/self/cwd/decoy.gitconfig"),
                ),
            ],
            &["GIT_CONFIG_NOSYSTEM"],
            &external,
        );
        run_isolated_source_test_from(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("HOME", OsStr::new("/proc/self/cwd")),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
            ],
            &["GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "XDG_CONFIG_HOME"],
            &external,
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    let error = guard(&root).expect_err("process-relative primary config source");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(error.to_string().contains("process-relative"), "{error}");
}

#[test]
fn allows_protected_metadata_and_external_config_sources() {
    let repo = init_repo();
    let root = repo.path();
    guard(root).expect("ordinary metadata config");

    std::fs::write(
        root.join(".git/protected.gitconfig"),
        "[user]\nname = protected\n",
    )
    .expect("write protected config");
    add_include(root, "include.path", "protected.gitconfig");
    guard(root).expect("protected metadata include");
    std::fs::create_dir(root.join(".git/subdir")).expect("create metadata subdirectory");
    add_include(root, "include.path", "subdir/../protected.gitconfig");
    guard(root).expect("metadata-local parent traversal");

    let external = tempfile::tempdir().expect("external config directory");
    let external_config = external.path().join("safe.gitconfig");
    std::fs::write(&external_config, "[user]\nemail = safe@example.com\n")
        .expect("write external config");
    add_include(
        root,
        "include.path",
        external_config.to_str().expect("UTF-8 external path"),
    );
    guard(root).expect("external include");
    add_include(
        root,
        "include.path",
        external
            .path()
            .join("missing.gitconfig")
            .to_str()
            .expect("UTF-8 missing external path"),
    );
    guard(root).expect("missing external include");
}

#[test]
fn rejects_empty_primary_global_and_system_sources_in_the_worktree() {
    const TEST_NAME: &str = "git_config_sources::tests::rejects_empty_primary_global_and_system_sources_in_the_worktree";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        let root = repo.path();
        let unsafe_config = root.join("empty-primary.gitconfig");
        std::fs::write(&unsafe_config, "").expect("write empty primary config");
        let safe = tempfile::NamedTempFile::new().expect("safe external config");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("GIT_CONFIG_GLOBAL", unsafe_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", safe.path().as_os_str()),
            ],
            &[],
        );
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("GIT_CONFIG_GLOBAL", safe.path().as_os_str()),
                ("GIT_CONFIG_SYSTEM", unsafe_config.as_os_str()),
            ],
            &[],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    assert_worktree_rejection(guard(&root).expect_err("unsafe primary config source"));
}

#[test]
fn rejects_missing_home_and_xdg_sources_in_the_worktree() {
    const TEST_NAME: &str =
        "git_config_sources::tests::rejects_missing_home_and_xdg_sources_in_the_worktree";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        let root = repo.path();
        let xdg = root.join("xdg");
        std::fs::create_dir(&xdg).expect("create XDG root");
        let safe_system = tempfile::NamedTempFile::new().expect("safe system config");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("HOME", root.as_os_str()),
                ("XDG_CONFIG_HOME", xdg.as_os_str()),
                ("GIT_CONFIG_SYSTEM", safe_system.path().as_os_str()),
            ],
            &["GIT_CONFIG_GLOBAL"],
        );
        return;
    }
    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    assert_worktree_rejection(guard(&root).expect_err("unsafe HOME/XDG source"));
}

#[test]
fn allows_explicitly_disabled_global_and_system_sources() {
    const TEST_NAME: &str =
        "git_config_sources::tests::allows_explicitly_disabled_global_and_system_sources";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("GIT_CONFIG_GLOBAL", OsStr::new("")),
                ("GIT_CONFIG_SYSTEM", OsStr::new("")),
            ],
            &[],
        );
        return;
    }
    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    guard(&root).expect("disabled primary config sources");
}

#[cfg(unix)]
#[test]
fn allows_unset_home_when_git_has_no_global_source() {
    const TEST_NAME: &str =
        "git_config_sources::tests::allows_unset_home_when_git_has_no_global_source";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
            ],
            &[
                "HOME",
                "XDG_CONFIG_HOME",
                "GIT_CONFIG_GLOBAL",
                "GIT_CONFIG_SYSTEM",
            ],
        );
        return;
    }
    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    guard(&root).expect("Git without HOME or global config");
}

#[test]
fn supports_pre_242_git_without_system_var_enumeration() {
    assert_eq!(
        primary_sources::parse_git_var_config_paths_result(
            Some(129),
            b"",
            b"usage: git var (-l | <variable>)\n",
            "GIT_CONFIG_SYSTEM",
        )
        .expect("pre-2.42 fallback"),
        None
    );
}

#[cfg(unix)]
#[test]
fn rejects_newline_bearing_raw_global_source_before_git_normalizes_it() {
    const TEST_NAME: &str = "git_config_sources::tests::rejects_newline_bearing_raw_global_source_before_git_normalizes_it";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let parent = tempfile::tempdir().expect("fixture parent");
        let root = parent.path().join("repo");
        init_repo_at(&root);
        let newline_dir = parent.path().join("external\n");
        std::fs::create_dir(&newline_dir).expect("create newline path component");
        let unsafe_config = root.join("global.gitconfig");
        std::fs::write(&unsafe_config, "").expect("write unsafe global config");
        let raw = newline_dir.join("../repo/global.gitconfig");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("GIT_CONFIG_GLOBAL", raw.as_os_str()),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
            ],
            &["GIT_CONFIG_SYSTEM"],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    assert_worktree_rejection(guard(&root).expect_err("newline raw global source"));
}

#[cfg(unix)]
#[test]
fn ignores_non_utf8_values_for_unrelated_global_keys() {
    const TEST_NAME: &str =
        "git_config_sources::tests::ignores_non_utf8_values_for_unrelated_global_keys";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        let global = tempfile::NamedTempFile::new().expect("external global config");
        std::fs::write(global.path(), b"[user]\nname = \xff\n")
            .expect("write non-UTF-8 unrelated config value");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("GIT_CONFIG_GLOBAL", global.path().as_os_str()),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
            ],
            &["GIT_CONFIG_SYSTEM"],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    guard(&root).expect("unrelated non-UTF-8 global value");
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn preserves_non_utf8_unix_repository_and_config_origin_paths() {
    use std::os::unix::ffi::OsStringExt;

    const TEST_NAME: &str =
        "git_config_sources::tests::preserves_non_utf8_unix_repository_and_config_origin_paths";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let parent = tempfile::tempdir().expect("non-UTF-8 fixture parent");
        let root = parent
            .path()
            .join(std::ffi::OsString::from_vec(b"repo-\xff".to_vec()));
        init_repo_at(&root);
        let safe = tempfile::NamedTempFile::new().expect("safe included config");
        add_include(
            &root,
            "include.path",
            safe.path().to_str().expect("UTF-8 safe include"),
        );
        let global = parent
            .path()
            .join(std::ffi::OsString::from_vec(b"global-\xff".to_vec()));
        std::fs::write(
            &global,
            format!("[include]\npath = {}\n", safe.path().display()),
        )
        .expect("write non-UTF-8 global path");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", root.as_os_str()),
                ("GIT_CONFIG_GLOBAL", global.as_os_str()),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
            ],
            &["GIT_CONFIG_SYSTEM"],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    guard(&root).expect("non-UTF-8 repository and config origins");
}

#[cfg(unix)]
#[test]
fn rejects_worktree_fifo_primary_source_without_opening_it() {
    const TEST_NAME: &str =
        "git_config_sources::tests::rejects_worktree_fifo_primary_source_without_opening_it";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        let fifo = repo.path().join("global.fifo");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo");
        assert!(status.success(), "mkfifo failed: {status}");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("GIT_CONFIG_GLOBAL", fifo.as_os_str()),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
            ],
            &["GIT_CONFIG_SYSTEM"],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    assert_worktree_rejection(guard(&root).expect_err("worktree FIFO config source"));
}

#[test]
fn git_config_nosystem_uses_the_shared_parser_with_a_cross_version_numeric_subset() {
    const TEST_NAME: &str = "git_config_sources::tests::git_config_nosystem_uses_the_shared_parser_with_a_cross_version_numeric_subset";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        let unsafe_system = repo.path().join("system.gitconfig");
        std::fs::write(&unsafe_system, "").expect("write unsafe system config");
        for (value, expected) in [
            ("1", "ignored"),
            ("2", "ignored"),
            ("-1", "ignored"),
            ("01", "ignored"),
            ("+1", "ignored"),
            ("0x1", "ignored"),
            ("010", "ignored"),
            ("1k", "ignored"),
            ("-1g", "ignored"),
            (" 1", "ignored"),
            ("2147483647", "ignored"),
            ("-2147483647", "ignored"),
            ("true", "ignored"),
            ("yes", "ignored"),
            ("on", "ignored"),
            ("", "rejected"),
            ("0", "rejected"),
            ("-0", "rejected"),
            ("false", "rejected"),
            ("no", "rejected"),
            ("off", "rejected"),
            ("not-a-bool", "invalid"),
            ("08", "invalid"),
            ("2147483648", "invalid"),
            ("-2147483648", "invalid"),
            ("-0x80000000", "invalid"),
            ("-020000000000", "invalid"),
            ("-2097152k", "invalid"),
            ("-2048m", "invalid"),
            ("-2g", "invalid"),
            (" -2G", "invalid"),
            ("-2147483649", "invalid"),
            ("2g", "invalid"),
        ] {
            run_isolated_source_test(
                TEST_NAME,
                &[
                    ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                    ("CODEX_GIT_CONFIG_SOURCE_EXPECTED", OsStr::new(expected)),
                    ("GIT_CONFIG_GLOBAL", OsStr::new("")),
                    ("GIT_CONFIG_SYSTEM", unsafe_system.as_os_str()),
                    ("GIT_CONFIG_NOSYSTEM", OsStr::new(value)),
                ],
                &[],
            );
        }
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    match std::env::var("CODEX_GIT_CONFIG_SOURCE_EXPECTED")
        .expect("expected child outcome")
        .as_str()
    {
        "ignored" => guard(&root).expect("system source disabled"),
        "rejected" => assert_worktree_rejection(guard(&root).expect_err("system source enabled")),
        "invalid" => assert_eq!(
            guard(&root).expect_err("invalid NOSYSTEM value").kind(),
            io::ErrorKind::InvalidData
        ),
        expected => panic!("unexpected child outcome {expected}"),
    }
}

#[cfg(windows)]
#[test]
fn rejects_both_windows_home_candidates_when_home_is_absent() {
    const TEST_NAME: &str =
        "git_config_sources::tests::rejects_both_windows_home_candidates_when_home_is_absent";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        let external_profile = tempfile::tempdir().expect("external USERPROFILE");
        for home in [repo.path().to_path_buf(), repo.path().join("future-home")] {
            let home = home.to_str().expect("UTF-8 Windows test path");
            assert!(home.len() >= 3 && home.as_bytes()[1] == b':', "{home}");
            run_isolated_source_test(
                TEST_NAME,
                &[
                    ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                    ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                    ("HOMEDRIVE", OsStr::new(&home[..2])),
                    ("HOMEPATH", OsStr::new(&home[2..])),
                    ("USERPROFILE", external_profile.path().as_os_str()),
                ],
                &[
                    "HOME",
                    "XDG_CONFIG_HOME",
                    "GIT_CONFIG_GLOBAL",
                    "GIT_CONFIG_SYSTEM",
                ],
            );
        }
        let external_home = external_profile
            .path()
            .to_str()
            .expect("UTF-8 external Windows home");
        assert!(
            external_home.len() >= 3 && external_home.as_bytes()[1] == b':',
            "{external_home}"
        );
        for profile in [
            repo.path().to_path_buf(),
            repo.path().join("future-profile"),
        ] {
            run_isolated_source_test(
                TEST_NAME,
                &[
                    ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                    ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                    ("HOMEDRIVE", OsStr::new(&external_home[..2])),
                    ("HOMEPATH", OsStr::new(&external_home[2..])),
                    ("USERPROFILE", profile.as_os_str()),
                ],
                &[
                    "HOME",
                    "XDG_CONFIG_HOME",
                    "GIT_CONFIG_GLOBAL",
                    "GIT_CONFIG_SYSTEM",
                ],
            );
        }
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    assert_worktree_rejection(guard(&root).expect_err("Windows synthesized HOME source"));
}

#[cfg(windows)]
#[test]
fn allows_exact_windows_nul_for_both_primary_config_channels() {
    const TEST_NAME: &str =
        "git_config_sources::tests::allows_exact_windows_nul_for_both_primary_config_channels";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        let repo = init_repo();
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("GIT_CONFIG_GLOBAL", OsStr::new("NUL")),
                ("GIT_CONFIG_SYSTEM", OsStr::new("NUL")),
            ],
            &["GIT_CONFIG_NOSYSTEM"],
        );
        return;
    }
    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    guard(&root).expect("exact NUL disables primary config channels");
}

#[cfg(windows)]
#[test]
fn windows_appdata_global_config_matches_native_git_selection_and_precedence() {
    const TEST_NAME: &str = "git_config_sources::tests::windows_appdata_global_config_matches_native_git_selection_and_precedence";
    if std::env::var_os("CODEX_GIT_CONFIG_SOURCE_CHILD").is_none() {
        for xdg in [None, Some(OsStr::new(""))] {
            for existing in [true, false] {
                let repo = init_repo();
                let appdata = repo.path().join("appdata");
                if existing {
                    let config = appdata.join("Git/config");
                    std::fs::create_dir_all(config.parent().expect("APPDATA config parent"))
                        .expect("create APPDATA config parent");
                    std::fs::write(&config, "[codex]\nappdata-marker = selected\n")
                        .expect("write APPDATA config");
                }
                let external_home = tempfile::tempdir().expect("external HOME");
                let mut env = vec![
                    ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                    (
                        "CODEX_GIT_CONFIG_SOURCE_EXPECTED",
                        OsStr::new(if existing {
                            "reject-existing-appdata"
                        } else {
                            "reject-missing-appdata"
                        }),
                    ),
                    ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                    ("HOME", external_home.path().as_os_str()),
                    ("APPDATA", appdata.as_os_str()),
                ];
                if let Some(xdg) = xdg {
                    env.push(("XDG_CONFIG_HOME", xdg));
                }
                let mut removed = vec![
                    "GIT_CONFIG_GLOBAL",
                    "GIT_CONFIG_SYSTEM",
                    "HOMEDRIVE",
                    "HOMEPATH",
                    "USERPROFILE",
                ];
                if xdg.is_none() {
                    removed.push("XDG_CONFIG_HOME");
                }
                run_isolated_source_test(TEST_NAME, &env, &removed);
            }
        }

        let repo = init_repo();
        let appdata = repo.path().join("appdata");
        let appdata_config = appdata.join("Git/config");
        std::fs::create_dir_all(appdata_config.parent().expect("APPDATA config parent"))
            .expect("create APPDATA config parent");
        std::fs::write(&appdata_config, "[codex]\nappdata-marker = appdata\n")
            .expect("write APPDATA config");
        let external_home = tempfile::tempdir().expect("external HOME");
        let external_xdg = tempfile::tempdir().expect("external XDG_CONFIG_HOME");
        let xdg_config = external_xdg.path().join("git/config");
        std::fs::create_dir_all(xdg_config.parent().expect("XDG config parent"))
            .expect("create XDG config parent");
        std::fs::write(&xdg_config, "[codex]\nappdata-marker = xdg\n").expect("write XDG config");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                ("CODEX_GIT_CONFIG_SOURCE_EXPECTED", OsStr::new("allow-xdg")),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                ("HOME", external_home.path().as_os_str()),
                ("APPDATA", appdata.as_os_str()),
                ("XDG_CONFIG_HOME", external_xdg.path().as_os_str()),
            ],
            &[
                "GIT_CONFIG_GLOBAL",
                "GIT_CONFIG_SYSTEM",
                "HOMEDRIVE",
                "HOMEPATH",
                "USERPROFILE",
            ],
        );

        let external_global = tempfile::NamedTempFile::new().expect("external global config");
        std::fs::write(
            external_global.path(),
            "[codex]\nappdata-marker = explicit\n",
        )
        .expect("write explicit global config");
        run_isolated_source_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_CONFIG_SOURCE_ROOT", repo.path().as_os_str()),
                (
                    "CODEX_GIT_CONFIG_SOURCE_EXPECTED",
                    OsStr::new("allow-explicit-global"),
                ),
                ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                ("HOME", repo.path().as_os_str()),
                ("APPDATA", appdata.as_os_str()),
                ("XDG_CONFIG_HOME", repo.path().as_os_str()),
                ("GIT_CONFIG_GLOBAL", external_global.path().as_os_str()),
            ],
            &["GIT_CONFIG_SYSTEM", "HOMEDRIVE", "HOMEPATH", "USERPROFILE"],
        );
        return;
    }

    let root = PathBuf::from(
        std::env::var_os("CODEX_GIT_CONFIG_SOURCE_ROOT").expect("fixture repository root"),
    );
    let expected =
        std::env::var("CODEX_GIT_CONFIG_SOURCE_EXPECTED").expect("expected child outcome");
    if expected != "reject-missing-appdata" {
        let output = std::process::Command::new("git")
            .args(["config", "--global", "--get", "codex.appdata-marker"])
            .current_dir(&root)
            .output()
            .expect("run native Git global config query");
        assert!(
            output.status.success(),
            "native Git global query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual = String::from_utf8_lossy(&output.stdout);
        let native_expected = match expected.as_str() {
            "reject-existing-appdata" => "selected",
            "allow-xdg" => "xdg",
            "allow-explicit-global" => "explicit",
            expected => panic!("unexpected child outcome {expected}"),
        };
        assert_eq!(
            actual.trim(),
            native_expected,
            "native Git global selection"
        );
    }
    match expected.as_str() {
        "reject-existing-appdata" | "reject-missing-appdata" => {
            assert_worktree_rejection(guard(&root).expect_err("worktree APPDATA config"));
        }
        "allow-xdg" => guard(&root).expect("nonempty XDG suppresses APPDATA"),
        "allow-explicit-global" => {
            guard(&root).expect("explicit global config suppresses HOME, XDG, and APPDATA")
        }
        expected => panic!("unexpected child outcome {expected}"),
    }
}

#[test]
fn rejects_empty_nonempty_absolute_missing_and_conditional_worktree_includes() {
    for (name, body, include_key, include_value) in [
        (
            "empty",
            "",
            "include.path".to_string(),
            "../driver-config".to_string(),
        ),
        (
            "nonempty",
            "[user]\nname = worktree\n",
            "include.path".to_string(),
            "../driver-config".to_string(),
        ),
        (
            "missing",
            "",
            "include.path".to_string(),
            "../future.gitconfig".to_string(),
        ),
        (
            "inactive includeIf",
            "",
            "includeIf.gitdir:/definitely/not/this/repository/**.path".to_string(),
            "../driver-config".to_string(),
        ),
        (
            "case-insensitive gitdir includeIf",
            "",
            "includeIf.gitdir/i:/definitely/not/this/repository/**.path".to_string(),
            "../driver-config".to_string(),
        ),
        (
            "onbranch includeIf",
            "",
            "includeIf.onbranch:definitely-never/**.path".to_string(),
            "../driver-config".to_string(),
        ),
        (
            "hasconfig includeIf",
            "",
            "includeIf.hasconfig:remote.*.url:https://never.example/**.path".to_string(),
            "../driver-config".to_string(),
        ),
    ] {
        let repo = init_repo();
        let root = repo.path();
        if name != "missing" {
            std::fs::write(root.join("driver-config"), body).expect("write config fixture");
        }
        add_include(root, &include_key, &include_value);
        assert_worktree_rejection(guard(root).expect_err(name));
    }

    let repo = init_repo();
    let root = repo.path();
    let config = root.join("absolute.gitconfig");
    std::fs::write(&config, "").expect("write absolute config");
    add_include(
        root,
        "include.path",
        config.to_str().expect("UTF-8 config path"),
    );
    assert_worktree_rejection(guard(root).expect_err("absolute include"));

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write conditional config");
    let git_dir = run_success(root, &["rev-parse", "--absolute-git-dir"]).replace('\\', "/");
    add_include(
        root,
        &format!("includeIf.gitdir:{git_dir}/.path"),
        "../driver-config",
    );
    assert_worktree_rejection(guard(root).expect_err("active includeIf"));
}

#[test]
fn command_scoped_include_paths_follow_the_same_boundary() {
    let repo = init_repo();
    let root = repo.path();
    let git = GitRunner::for_cwd_io(root).expect("Git runner");
    let unsafe_config = root.join("command.gitconfig");
    std::fs::write(&unsafe_config, "").expect("write worktree config");
    let unsafe_args = vec![
        "-c".to_string(),
        format!("include.path={}", unsafe_config.display()),
    ];
    assert_worktree_rejection(
        ensure_no_worktree_config_sources(&git, root, &unsafe_args)
            .expect_err("command-scoped worktree include"),
    );

    let external = tempfile::NamedTempFile::new().expect("safe external config");
    let safe_args = vec![
        "-c".to_string(),
        format!("include.path={}", external.path().display()),
    ];
    ensure_no_worktree_config_sources(&git, root, &safe_args)
        .expect("command-scoped external include");

    let relative_args = vec![
        "-c".to_string(),
        "include.path=relative.gitconfig".to_string(),
    ];
    assert!(
        ensure_no_worktree_config_sources(&git, root, &relative_args).is_err(),
        "relative command includes must fail closed"
    );
}

#[tokio::test]
async fn blocking_and_async_source_authorization_share_the_same_include_policy() {
    let repo = init_repo();
    let root = repo.path();
    let git = GitRunner::for_cwd_io(root).expect("Git runner");

    ensure_no_worktree_config_sources(&git, root, &[]).expect("blocking safe source graph");
    ensure_no_worktree_config_sources_async(&git, root, &[])
        .await
        .expect("async safe source graph");

    let worktree_include = root.join("attacker.gitconfig");
    std::fs::write(&worktree_include, "[codex]\n\tvalue = attacker\n").expect("worktree include");
    add_include(
        root,
        "include.path",
        worktree_include.to_str().expect("UTF-8 fixture path"),
    );
    let blocking = ensure_no_worktree_config_sources(&git, root, &[])
        .expect_err("blocking authorization must reject worktree include");
    let asynchronous = ensure_no_worktree_config_sources_async(&git, root, &[])
        .await
        .expect_err("async authorization must reject worktree include");
    assert_eq!(blocking.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(asynchronous.kind(), blocking.kind());
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_process_relative_procfs_includes_across_the_include_graph() {
    use std::os::unix::fs::symlink;

    for (key, value, leaf) in [
        (
            "include.path",
            "/proc/self/cwd/direct.gitconfig",
            "direct.gitconfig",
        ),
        (
            "includeIf.gitdir:/definitely/not/this/repository/**.path",
            "/proc/thread-self/cwd/conditional.gitconfig",
            "conditional.gitconfig",
        ),
    ] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join(leaf), "[unsafe]\nhelper = worktree\n")
            .expect("write process-relative include target");
        add_include(root, key, value);
        assert_process_relative_include_rejection(
            guard(root).expect_err("process-relative repository include"),
        );
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join("command.gitconfig"),
        "[unsafe]\nhelper = worktree\n",
    )
    .expect("write command include target");
    let git = GitRunner::for_cwd_io(root).expect("Git runner");
    let args = vec![
        "-c".to_string(),
        "include.path=/proc/self/cwd/command.gitconfig".to_string(),
    ];
    assert_process_relative_include_rejection(
        ensure_no_worktree_config_sources(&git, root, &args)
            .expect_err("process-relative command-scoped include"),
    );

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join("nested.gitconfig"),
        "[unsafe]\nhelper = worktree\n",
    )
    .expect("write nested include target");
    let external = tempfile::tempdir().expect("external include directory");
    let parent = external.path().join("parent.gitconfig");
    let alias = external.path().join("procfs-alias");
    symlink("/proc/self/cwd", &alias).expect("create procfs alias");
    std::fs::write(&parent, "[include]\npath = procfs-alias/nested.gitconfig\n")
        .expect("write external parent config");
    add_include(
        root,
        "include.path",
        parent.to_str().expect("UTF-8 parent config path"),
    );
    assert_process_relative_include_rejection(
        guard(root).expect_err("nested aliased process-relative include"),
    );
}

#[test]
fn rejects_every_duplicate_and_nested_include_target() {
    for unsafe_first in [true, false] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("driver-config"), "").expect("write unsafe config");
        let external = tempfile::NamedTempFile::new().expect("safe external config");
        let safe = external.path().to_str().expect("UTF-8 safe path");
        let values = if unsafe_first {
            ["../driver-config", safe]
        } else {
            [safe, "../driver-config"]
        };
        for value in values {
            add_include(root, "include.path", value);
        }
        assert_worktree_rejection(guard(root).expect_err("duplicate include"));
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write nested target");
    let external = tempfile::tempdir().expect("external config directory");
    let outer = external.path().join("outer.gitconfig");
    let nested = root.join("driver-config");
    run_success(
        root,
        &[
            "config",
            "--file",
            outer.to_str().expect("UTF-8 outer path"),
            "include.path",
            nested.to_str().expect("UTF-8 nested path"),
        ],
    );
    add_include(
        root,
        "include.path",
        outer.to_str().expect("UTF-8 outer path"),
    );
    assert_worktree_rejection(guard(root).expect_err("nested include"));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_aliases_across_the_worktree_and_metadata_boundaries() {
    use std::os::unix::fs::symlink;

    let repo = init_repo();
    let root = repo.path();
    let external = tempfile::tempdir().expect("external directory");
    std::fs::write(external.path().join("safe.gitconfig"), "").expect("write safe config");
    symlink(external.path(), root.join("inside-link")).expect("inside to outside link");
    add_include(root, "include.path", "../inside-link/safe.gitconfig");
    assert_worktree_rejection(guard(root).expect_err("worktree symlink alias"));

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write worktree target");
    let external = tempfile::tempdir().expect("external directory");
    symlink(root, external.path().join("outside-link")).expect("outside to inside link");
    add_include(
        root,
        "include.path",
        external
            .path()
            .join("outside-link/driver-config")
            .to_str()
            .expect("UTF-8 alias path"),
    );
    assert_worktree_rejection(guard(root).expect_err("external symlink alias"));

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write escaped target");
    symlink(root, root.join(".git/config-link")).expect("metadata escape link");
    add_include(root, "include.path", "config-link/driver-config");
    assert_worktree_rejection(guard(root).expect_err("metadata symlink escape"));

    let repo = init_repo();
    let root = repo.path();
    let external = tempfile::tempdir().expect("external directory");
    symlink(
        root.join("future-config"),
        external.path().join("dangling-link"),
    )
    .expect("dangling link into worktree");
    add_include(
        root,
        "include.path",
        external
            .path()
            .join("dangling-link/config")
            .to_str()
            .expect("UTF-8 dangling alias"),
    );
    assert_worktree_rejection(guard(root).expect_err("dangling symlink alias"));

    let repo = init_repo();
    let root = repo.path();
    let entry = tempfile::tempdir().expect("external entry directory");
    let exit = tempfile::tempdir().expect("external exit directory");
    std::fs::write(exit.path().join("safe.gitconfig"), "").expect("write exit config");
    symlink(root, entry.path().join("through-worktree")).expect("link into worktree");
    symlink(exit.path(), root.join("back-out")).expect("worktree-controlled exit link");
    add_include(
        root,
        "include.path",
        entry
            .path()
            .join("through-worktree/back-out/safe.gitconfig")
            .to_str()
            .expect("UTF-8 nested alias"),
    );
    assert_worktree_rejection(guard(root).expect_err("nested symlink traversal"));

    let repo = init_repo();
    let root = repo.path();
    let entry = tempfile::tempdir().expect("external entry directory");
    let exit = tempfile::tempdir().expect("external exit directory");
    std::fs::write(exit.path().join("safe.gitconfig"), "").expect("write chained config");
    symlink(exit.path(), root.join("workspace-link")).expect("workspace link out");
    symlink(
        root.join("workspace-link"),
        entry.path().join("direct-chain"),
    )
    .expect("external link to workspace link");
    add_include(
        root,
        "include.path",
        entry
            .path()
            .join("direct-chain/safe.gitconfig")
            .to_str()
            .expect("UTF-8 direct chain"),
    );
    assert_worktree_rejection(guard(root).expect_err("direct symlink chain"));

    let repo = init_repo();
    let root = repo.path();
    std::fs::create_dir(root.join("subdir")).expect("create symlink target");
    std::fs::write(root.join("evil.gitconfig"), "[user]\nname = worktree\n")
        .expect("write worktree config");
    std::fs::write(
        root.join(".git/evil.gitconfig"),
        "[user]\nname = metadata\n",
    )
    .expect("write metadata decoy");
    symlink(root.join("subdir"), root.join(".git/link")).expect("metadata link to worktree");
    add_include(root, "include.path", "link/../evil.gitconfig");
    assert_worktree_rejection(guard(root).expect_err("symlink parent traversal"));

    let repo = init_repo();
    let root = repo.path();
    std::fs::create_dir(root.join("pivot")).expect("create worktree pivot");
    std::fs::write(root.join(".git/protected.gitconfig"), "")
        .expect("write protected metadata config");
    add_include(
        root,
        "include.path",
        root.join("pivot/../.git/protected.gitconfig")
            .to_str()
            .expect("UTF-8 pivot path"),
    );
    assert_worktree_rejection(guard(root).expect_err("raw path pivots through worktree"));
}

#[cfg(unix)]
#[test]
fn rejects_external_symlink_target_with_hidden_worktree_pivot_to_protected_config() {
    use std::os::unix::fs::symlink;

    let repo = init_repo();
    let root = std::fs::canonicalize(repo.path()).expect("canonical repository");
    let external = tempfile::tempdir().expect("external alias directory");
    let protected = root.join(".git/protected.gitconfig");
    std::fs::write(&protected, "[codex]\n\tpivot = protected\n").expect("write protected config");
    std::fs::create_dir(root.join("pivot")).expect("create ordinary worktree pivot");
    let entry = external.path().join("entry");
    symlink(root.join("pivot/../.git/protected.gitconfig"), &entry)
        .expect("external symlink target with raw worktree pivot");
    add_include(
        &root,
        "include.path",
        entry.to_str().expect("UTF-8 entry path"),
    );
    assert_eq!(
        run_success(&root, &["config", "--includes", "--get", "codex.pivot"]),
        "protected"
    );
    assert_worktree_rejection(
        guard(&root).expect_err("hidden pivot inside external symlink target"),
    );
}

#[cfg(target_os = "macos")]
#[test]
fn rejects_apfs_alias_symlink_target_with_hidden_worktree_pivot() {
    use std::os::unix::fs::symlink;

    let temp = std::fs::canonicalize(std::env::temp_dir()).expect("canonical temp directory");
    let parent = tempfile::tempdir_in(temp).expect("Data-volume fixture");
    let root = parent.path().join("repo");
    init_repo_at(&root);
    let root = std::fs::canonicalize(root).expect("canonical repository");
    let data_alias = PathBuf::from("/System/Volumes/Data")
        .join(root.strip_prefix("/").expect("absolute repository path"));
    if std::fs::metadata(&data_alias).is_err() {
        eprintln!("APFS Data alias unavailable; skipping native pivot assertion");
        return;
    }
    let protected = root.join(".git/protected.gitconfig");
    std::fs::write(&protected, "[codex]\n\tpivot = protected\n").expect("write protected config");
    std::fs::create_dir(root.join("pivot")).expect("create ordinary worktree pivot");
    let external = tempfile::tempdir().expect("external alias directory");
    let entry = external.path().join("entry");
    symlink(data_alias.join("pivot/../.git/protected.gitconfig"), &entry)
        .expect("APFS alias target with raw worktree pivot");
    add_include(
        &root,
        "include.path",
        entry.to_str().expect("UTF-8 entry path"),
    );
    assert_eq!(
        run_success(&root, &["config", "--includes", "--get", "codex.pivot"]),
        "protected"
    );
    assert_worktree_rejection(guard(&root).expect_err("APFS hidden pivot inside symlink target"));
}

#[cfg(unix)]
#[test]
fn rejects_unregistered_same_common_worktree_used_only_as_alias_intermediate() {
    use std::os::unix::fs::symlink;

    let parent = tempfile::tempdir().expect("fixture");
    let main = parent.path().join("main");
    let linked = parent.path().join("linked");
    let stale = parent.path().join("stale-unregistered");
    let external = parent.path().join("external");
    let safe = external.join("safe.gitconfig");
    let entry = external.join("entry");
    init_repo_at(&main);
    run_success(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "same-common-active",
            linked.to_str().expect("UTF-8 linked path"),
        ],
    );
    std::fs::create_dir_all(&stale).expect("create stale worktree");
    std::fs::write(
        stale.join(".git"),
        format!("gitdir: {}\n", main.join(".git").display()),
    )
    .expect("write stale same-common marker");
    std::fs::create_dir_all(&external).expect("create external directory");
    std::fs::write(&safe, "[codex]\n\tsameCommon = protected\n")
        .expect("write external safe config");
    symlink(&safe, stale.join("switch")).expect("stale worktree controlled switch");
    symlink(stale.join("switch"), &entry).expect("external entry through stale worktree");
    add_include(
        &linked,
        "include.path",
        entry.to_str().expect("UTF-8 entry path"),
    );
    assert_eq!(
        run_success(
            &linked,
            &["config", "--includes", "--get", "codex.sameCommon"]
        ),
        "protected"
    );
    assert!(
        guard(&linked).is_err(),
        "unregistered same-common route intermediate must fail closed"
    );
}

#[cfg(windows)]
#[test]
fn rejects_windows_junction_aliases_across_worktree_and_metadata_boundaries() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write worktree config");
    let external = tempfile::tempdir().expect("external directory");
    create_junction(&external.path().join("into-worktree"), root);
    add_include(
        root,
        "include.path",
        external
            .path()
            .join("into-worktree/driver-config")
            .to_str()
            .expect("UTF-8 junction path"),
    );
    assert_worktree_rejection(guard(root).expect_err("external junction into worktree"));

    let repo = init_repo();
    let root = repo.path();
    let external = tempfile::tempdir().expect("external directory");
    std::fs::write(external.path().join("safe.gitconfig"), "").expect("write external config");
    create_junction(&root.join("out-of-worktree"), external.path());
    add_include(root, "include.path", "../out-of-worktree/safe.gitconfig");
    assert_worktree_rejection(guard(root).expect_err("worktree junction to external"));

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write worktree config");
    create_junction(&root.join(".git/metadata-escape"), root);
    add_include(root, "include.path", "metadata-escape/driver-config");
    assert_worktree_rejection(guard(root).expect_err("metadata junction escape"));

    let repo = init_repo();
    let root = repo.path();
    let entry = tempfile::tempdir().expect("external entry directory");
    let exit = tempfile::tempdir().expect("external exit directory");
    std::fs::write(exit.path().join("safe.gitconfig"), "").expect("write external config");
    create_junction(&entry.path().join("through-worktree"), root);
    create_junction(&root.join("back-out"), exit.path());
    add_include(
        root,
        "include.path",
        entry
            .path()
            .join("through-worktree/back-out/safe.gitconfig")
            .to_str()
            .expect("UTF-8 multi-hop junction path"),
    );
    assert_worktree_rejection(guard(root).expect_err("multi-hop junction traversal"));

    let repo = init_repo();
    let root = repo.path();
    let external = tempfile::tempdir().expect("external directory");
    create_junction(&external.path().join("into-worktree"), root);
    add_include(
        root,
        "include.path",
        external
            .path()
            .join("into-worktree/future.gitconfig")
            .to_str()
            .expect("UTF-8 missing junction target"),
    );
    assert_worktree_rejection(guard(root).expect_err("junction missing suffix"));
}

#[test]
fn linked_worktree_metadata_is_allowed_but_linked_worktree_config_is_rejected() {
    let parent = tempfile::tempdir().expect("worktree parent");
    let main = parent.path().join("main");
    let linked = parent.path().join("linked");
    init_repo_at(&main);
    run_success(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "config-guard-linked",
            linked.to_str().expect("UTF-8 linked path"),
        ],
    );

    let git_dir = PathBuf::from(run_success(&linked, &["rev-parse", "--absolute-git-dir"]));
    let protected = git_dir.join("protected.gitconfig");
    std::fs::write(&protected, "[user]\nname = linked\n").expect("write linked metadata config");
    add_include(
        &linked,
        "include.path",
        protected.to_str().expect("UTF-8 protected path"),
    );
    guard(&linked).expect("linked metadata include");
    run_success(&linked, &["config", "--unset-all", "include.path"]);

    std::fs::write(main.join("driver-config"), "").expect("write main-worktree config");
    add_include(
        &linked,
        "include.path",
        main.join("driver-config")
            .to_str()
            .expect("UTF-8 main-worktree config"),
    );
    assert_worktree_rejection(guard(&linked).expect_err("main worktree include"));
    run_success(&linked, &["config", "--unset-all", "include.path"]);
    add_include(
        &linked,
        "include.path",
        main.join("future.gitconfig")
            .to_str()
            .expect("UTF-8 missing main-worktree config"),
    );
    assert_worktree_rejection(guard(&linked).expect_err("missing main worktree include"));
    run_success(&linked, &["config", "--unset-all", "include.path"]);

    let unsafe_config = linked.join("driver-config");
    std::fs::write(&unsafe_config, "").expect("write linked worktree config");
    add_include(
        &linked,
        "include.path",
        unsafe_config.to_str().expect("UTF-8 unsafe path"),
    );
    assert_worktree_rejection(guard(&linked).expect_err("linked worktree include"));
}

#[test]
fn enclosing_repository_sibling_worktrees_are_always_untrusted() {
    let parent = tempfile::tempdir().expect("fixture parent");
    let outer = parent.path().join("outer");
    let sibling = parent.path().join("sibling");
    init_repo_at(&outer);
    run_success(
        &outer,
        &[
            "worktree",
            "add",
            "-b",
            "config-guard-sibling",
            sibling.to_str().expect("UTF-8 sibling worktree"),
        ],
    );
    let nested = outer.join("nested");
    init_repo_at(&nested);

    std::fs::write(sibling.join("driver-config"), "").expect("write sibling config");
    add_include(
        &nested,
        "include.path",
        sibling
            .join("driver-config")
            .to_str()
            .expect("UTF-8 sibling config"),
    );
    assert_worktree_rejection(guard(&nested).expect_err("enclosing repo sibling config"));

    run_success(&nested, &["config", "--unset-all", "include.path"]);
    std::fs::remove_dir_all(&sibling).expect("remove sibling but retain registry entry");
    add_include(
        &nested,
        "include.path",
        sibling
            .join("future.gitconfig")
            .to_str()
            .expect("UTF-8 missing sibling config"),
    );
    assert_worktree_rejection(guard(&nested).expect_err("missing enclosing repo sibling config"));

    let nested_primary = outer.join("inner-primary");
    let inner_linked = parent.path().join("inner-linked");
    init_repo_at(&nested_primary);
    run_success(
        &nested_primary,
        &[
            "worktree",
            "add",
            "-b",
            "inner-linked-from-outer",
            inner_linked.to_str().expect("UTF-8 inner linked worktree"),
        ],
    );
    for candidate in [
        outer.join("outer.txt"),
        outer.join("future-outer.gitconfig"),
    ] {
        if candidate.file_name() == Some(OsStr::new("outer.txt")) {
            std::fs::write(&candidate, "").expect("write outer config source");
        }
        add_include(
            &inner_linked,
            "include.path",
            candidate.to_str().expect("UTF-8 outer config source"),
        );
        assert_worktree_rejection(
            guard(&inner_linked).expect_err("enclosing primary config source"),
        );
        run_success(&inner_linked, &["config", "--unset-all", "include.path"]);
    }
    add_include(
        &inner_linked,
        "include.path",
        sibling
            .join("future-from-inner.gitconfig")
            .to_str()
            .expect("UTF-8 enclosing sibling source"),
    );
    assert_worktree_rejection(
        guard(&inner_linked).expect_err("enclosing sibling from linked inner worktree"),
    );
    run_success(&inner_linked, &["config", "--unset-all", "include.path"]);
}

#[cfg(unix)]
#[test]
fn preserves_symlinked_caller_ancestry_and_each_include_spelling() {
    use std::os::unix::fs::symlink;

    let parent = tempfile::tempdir().expect("fixture parent");
    let outer = parent.path().join("outer");
    let nested = parent.path().join("external-nested");
    init_repo_at(&outer);
    init_repo_at(&nested);
    std::fs::write(outer.join("driver-config"), "").expect("write outer config");
    symlink(&nested, outer.join("nested-link")).expect("link nested cwd through outer repo");
    add_include(
        &nested,
        "include.path",
        outer
            .join("driver-config")
            .to_str()
            .expect("UTF-8 outer config"),
    );
    let git = GitRunner::for_cwd_io(&outer.join("nested-link"))
        .expect("Git runner from symlinked nested cwd");
    assert_worktree_rejection(
        ensure_no_worktree_config_sources(&git, &nested, &[])
            .expect_err("logical outer ancestry config"),
    );

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("driver-config"), "").expect("write unsafe child config");
    let external = tempfile::tempdir().expect("external include tree");
    let shared = external.path().join("shared.gitconfig");
    std::fs::write(&shared, "[include]\npath = child.gitconfig\n")
        .expect("write shared parent config");
    let unsafe_dir = external.path().join("unsafe-spelling");
    let safe_dir = external.path().join("safe-spelling");
    std::fs::create_dir(&unsafe_dir).expect("create unsafe spelling directory");
    std::fs::create_dir(&safe_dir).expect("create safe spelling directory");
    symlink(&shared, unsafe_dir.join("parent.gitconfig"))
        .expect("unsafe spelling of shared parent");
    symlink(&shared, safe_dir.join("parent.gitconfig")).expect("safe spelling of shared parent");
    symlink(
        root.join("driver-config"),
        unsafe_dir.join("child.gitconfig"),
    )
    .expect("unsafe relative child");
    std::fs::write(safe_dir.join("child.gitconfig"), "").expect("safe relative child");
    add_include(
        root,
        "include.path",
        unsafe_dir
            .join("parent.gitconfig")
            .to_str()
            .expect("UTF-8 unsafe spelling"),
    );
    add_include(
        root,
        "include.path",
        safe_dir
            .join("parent.gitconfig")
            .to_str()
            .expect("UTF-8 safe spelling"),
    );
    assert_worktree_rejection(guard(root).expect_err("unsafe relative alias child"));

    let parent = tempfile::tempdir().expect("linked-main dangling fixture");
    let main = parent.path().join("main");
    let linked = parent.path().join("linked");
    init_repo_at(&main);
    run_success(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "config-guard-dangling-main",
            linked.to_str().expect("UTF-8 linked worktree"),
        ],
    );
    let external = tempfile::tempdir().expect("external dangling alias directory");
    symlink(
        main.join("future.gitconfig"),
        external.path().join("dangling-main-config"),
    )
    .expect("dangling alias into main worktree");
    add_include(
        &linked,
        "include.path",
        external
            .path()
            .join("dangling-main-config")
            .to_str()
            .expect("UTF-8 dangling main alias"),
    );
    assert_worktree_rejection(guard(&linked).expect_err("dangling alias into main worktree"));
}

#[cfg(target_os = "macos")]
#[test]
fn rejects_apfs_data_firmlink_aliases_for_existing_and_missing_worktree_configs() {
    let data_root = Path::new("/System/Volumes/Data");
    if !data_root.is_dir() {
        return;
    }
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
    let temp_parent = home.join(".cache/codex-git-utils-tests");
    std::fs::create_dir_all(&temp_parent).expect("create test temp parent");

    for exists in [true, false] {
        let repo = tempfile::Builder::new()
            .prefix("firmlink-config-")
            .tempdir_in(&temp_parent)
            .expect("home tempdir");
        let root = repo.path();
        init_repo_at(root);
        if exists {
            std::fs::write(root.join("driver-config"), "").expect("write aliased config");
        }
        let alias = data_root
            .join(root.strip_prefix("/").expect("absolute repository path"))
            .join("driver-config");
        add_include(
            root,
            "include.path",
            alias.to_str().expect("UTF-8 firmlink alias"),
        );
        assert_worktree_rejection(guard(root).expect_err("APFS firmlink alias"));
    }

    let family = tempfile::Builder::new()
        .prefix("firmlink-linked-family-")
        .tempdir_in(&temp_parent)
        .expect("home linked-family tempdir");
    let main = family.path().join("main");
    let linked = family.path().join("linked");
    init_repo_at(&main);
    run_success(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "firmlink-linked-main",
            linked.to_str().expect("UTF-8 linked path"),
        ],
    );
    for exists in [true, false] {
        let name = if exists {
            "driver-config"
        } else {
            "future.gitconfig"
        };
        if exists {
            std::fs::write(main.join(name), "").expect("write main config");
        }
        let alias = data_root
            .join(main.strip_prefix("/").expect("absolute main path"))
            .join(name);
        add_include(
            &linked,
            "include.path",
            alias.to_str().expect("UTF-8 main firmlink alias"),
        );
        assert_worktree_rejection(guard(&linked).expect_err("main firmlink alias"));
        run_success(&linked, &["config", "--unset-all", "include.path"]);
    }
    std::fs::write(main.join(".git/protected.gitconfig"), "")
        .expect("write protected main metadata config");
    let protected_alias = data_root
        .join(main.strip_prefix("/").expect("absolute main path"))
        .join(".git/protected.gitconfig");
    add_include(
        &linked,
        "include.path",
        protected_alias
            .to_str()
            .expect("UTF-8 protected metadata alias"),
    );
    guard(&linked).expect("protected metadata firmlink alias");
}

#[test]
fn include_path_expansion_supports_git_prefix() {
    let repo = init_repo();
    let root = repo.path();
    let git = GitRunner::for_cwd_io(root).expect("Git runner");
    let prefix =
        expand_git_config_path(&git, root, "%(prefix)/etc/gitconfig").expect("expand Git prefix");
    assert!(prefix.is_absolute());
}

#[cfg(unix)]
#[tokio::test]
async fn async_include_budget_stops_before_over_limit_expansion_launch() {
    use std::os::unix::fs::PermissionsExt;

    let repo = init_repo();
    let external = tempfile::tempdir().expect("external include fixture");
    let fake_git = external.path().join("git");
    std::fs::write(
        &fake_git,
        "#!/bin/sh\n\
         printf 'launch\\n' >>\"$0.log\"\n\
         value=\n\
         for arg in \"$@\"; do\n\
           case \"$arg\" in\n\
             codex.config-source.path=*) value=${arg#codex.config-source.path=} ;;\n\
           esac\n\
         done\n\
         printf '%s\\0' \"$value\"\n",
    )
    .expect("write fake Git");
    let mut permissions = std::fs::metadata(&fake_git)
        .expect("stat fake Git")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&fake_git, permissions).expect("make fake Git executable");

    let git = GitRunner::from_executable_for_test(repo.path(), fake_git.clone())
        .expect("authority-bound fake Git");
    let entries = (0..3)
        .map(|index| GitConfigEntry {
            scope: crate::git_config::GitConfigScope::Command,
            origin: crate::git_config::GitConfigOrigin::CommandLine,
            key: "include.path".to_string(),
            value: external
                .path()
                .join(format!("include-{index}.gitconfig"))
                .to_string_lossy()
                .into_owned(),
        })
        .collect();
    let mut pending = Vec::new();
    let mut budget = IncludeGraphBudget::with_limit(/*limit*/ 2);

    let error = validate_include_entries_async(
        &git,
        repo.path(),
        entries,
        /*depth*/ 1,
        &mut pending,
        &mut budget,
    )
    .await
    .expect_err("third include must exceed the traversal budget");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{error}");
    assert!(
        error.to_string().contains("too many Git config includes"),
        "{error}"
    );
    assert_eq!(pending.len(), 2);
    let launches =
        std::fs::read_to_string(fake_git.with_extension("log")).expect("read fake Git launch log");
    assert_eq!(launches.lines().count(), 2, "{launches}");
}

#[cfg(not(windows))]
#[test]
fn include_path_expansion_preserves_colon_parentheses_as_literal_text() {
    let repo = init_repo();
    let root = repo.path();
    let git = GitRunner::for_cwd_io(root).expect("Git runner");
    let optional = GitConfigEntry {
        scope: crate::git_config::GitConfigScope::Local,
        origin: crate::git_config::GitConfigOrigin::File(".git/config".into()),
        key: "include.path".to_string(),
        value: ":(optional)../future.gitconfig".to_string(),
    };
    let optional_path = resolve_include_path(&git, root, &optional).expect("resolve optional path");
    assert_eq!(
        AbsolutePathBuf::resolve_path_against_base(optional_path, root).as_path(),
        root.join(".git/:(optional)../future.gitconfig")
    );

    let unknown = GitConfigEntry {
        value: ":(unknown)../future.gitconfig".to_string(),
        ..optional
    };
    let unknown_path =
        resolve_include_path(&git, root, &unknown).expect("resolve literal unknown path");
    assert_eq!(
        AbsolutePathBuf::resolve_path_against_base(unknown_path, root).as_path(),
        root.join(".git/:(unknown)../future.gitconfig")
    );

    for spelling in [":(optional)", ":(unknown)"] {
        let external = tempfile::tempdir().expect("external literal config directory");
        let literal_dir = external.path().join(spelling);
        std::fs::create_dir(&literal_dir).expect("create literal include directory");
        std::fs::write(literal_dir.join("child"), "[probe]\nvalue = literal\n")
            .expect("write literal include child");
        let parent = external.path().join("parent.gitconfig");
        std::fs::write(&parent, format!("[include]\npath = {spelling}/child\n"))
            .expect("write literal parent config");
        let value = run_success(
            root,
            &[
                "config",
                "--file",
                parent.to_str().expect("UTF-8 parent path"),
                "--includes",
                "--get",
                "probe.value",
            ],
        );
        assert_eq!(value, "literal");
        add_include(
            root,
            "include.path",
            parent.to_str().expect("UTF-8 parent path"),
        );
        guard(root).expect("literal include path is external");
        run_success(root, &["config", "--unset-all", "include.path"]);
    }
}

#[test]
fn windows_path_validator_rejects_namespaces_streams_and_alias_components() {
    for path in [
        r"\??\C:\repo\config",
        r"\\?\GLOBALROOT\Device\config",
        r"\\.\pipe\config",
        r"C:\repo\config:stream",
        r"C:relative\config",
        r":(optional)..\future.gitconfig",
        r":(unknown)..\future.gitconfig",
        r"C:\repo\NUL.gitconfig",
        r"C:\repo\COM¹.gitconfig",
        r"C:\repo\LPT³.gitconfig",
        r"C:\repo\trailing.\config",
    ] {
        assert!(windows_config_path_is_ambiguous(path), "{path:?}");
    }
    for path in [
        r"C:\external\config",
        r"\\server\share\config",
        r"\\?\C:\repo\.git\config",
        r"\\.\C:\repo\.git\config",
        r"\\?\UNC\server\share\config",
        r"\\.\UNC\server\share\config",
        r"..\driver-config",
        r".\driver-config",
    ] {
        assert!(!windows_config_path_is_ambiguous(path), "{path:?}");
    }
}
