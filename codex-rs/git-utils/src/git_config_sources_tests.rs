use super::*;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::path::PathBuf;

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
    ensure_no_worktree_primary_config_sources(&git, root)
}

fn assert_worktree_rejection(error: io::Error) {
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(error.to_string().contains("worktree-controlled"), "{error}");
}

fn run_isolated_source_test(test_name: &str, env: &[(&str, &OsStr)], removed: &[&str]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    crate::safe_git::isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_CONFIG_SOURCE_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
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
