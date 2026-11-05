//! Multiple denied files must not prevent sandbox startup or expose protected contents.

use super::LONG_TIMEOUT_MS;
use super::codex_linux_sandbox_exe;
use super::create_env_from_core_vars;
use super::run_cmd_result_with_permission_profile_for_cwd;
use super::should_skip_bwrap_tests;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cargo_bin::write_executable;
use pretty_assertions::assert_eq;

enum DeniedFileRules {
    ExactPaths,
    Globs,
    GlobsFromRoot,
    GlobsWithProtectedRg,
    GlobsWithoutRg,
}

#[test_case::test_case(DeniedFileRules::ExactPaths; "exact_paths")]
#[test_case::test_case(DeniedFileRules::Globs; "globs")]
#[test_case::test_case(DeniedFileRules::GlobsFromRoot; "globs_from_root")]
#[test_case::test_case(DeniedFileRules::GlobsWithProtectedRg; "globs_with_protected_rg")]
#[test_case::test_case(DeniedFileRules::GlobsWithoutRg; "globs_without_rg")]
#[tokio::test]
async fn sandbox_starts_with_multiple_denied_files(rules: DeniedFileRules) {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let use_controlled_rg_path = matches!(
        rules,
        DeniedFileRules::GlobsWithProtectedRg | DeniedFileRules::GlobsWithoutRg
    );
    let temp = if use_controlled_rg_path {
        // Bazel can mount the default temp directory noexec.
        tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
            .or_else(|_| tempfile::tempdir())
    } else {
        tempfile::tempdir()
    }
    .expect("tempdir");
    let workspace = AbsolutePathBuf::try_from(temp.path()).expect("absolute workspace");
    std::fs::create_dir(workspace.join("nested")).expect("create nested directory");
    std::fs::write(workspace.join("AGENTS.md"), "project instructions\n")
        .expect("write allowed instructions");
    let denied_files = [
        (workspace.join("one.key"), "dummy first key"),
        (workspace.join("nested/two.key"), "dummy second key"),
        (workspace.join(".env.local"), "dummy environment"),
        (workspace.join("nested/.env"), "dummy nested environment"),
        (workspace.join("#report-1.csv"), "dummy report"),
        (workspace.join("!report.csv"), "dummy literal bang report"),
        (workspace.join("notes.txt "), "dummy spaced filename"),
        (workspace.join("root-only.cfg"), "dummy root configuration"),
    ];
    for (path, contents) in &denied_files {
        std::fs::write(path, contents).expect("write denied dummy file");
    }
    for path in ["notes.txt", "nested/allowed.cfg"] {
        std::fs::write(workspace.join(path), "public").expect("write allowed file");
    }
    std::os::unix::fs::symlink("AGENTS.md", workspace.join("relative.key"))
        .expect("relative file symlink");
    std::os::unix::fs::symlink(workspace.join("AGENTS.md"), workspace.join("absolute.key"))
        .expect("absolute file symlink");

    let sandbox_helper = codex_linux_sandbox_exe();
    let helper_dir = AbsolutePathBuf::try_from(sandbox_helper.parent().expect("helper parent"))
        .expect("absolute helper directory");
    let mut entries = vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(helper_dir.into(), FileSystemAccessMode::Read),
        FileSystemSandboxEntry::new(workspace.clone().into(), FileSystemAccessMode::Write),
    ];
    let mut env = create_env_from_core_vars();
    // User config must not suppress the files used to construct deny masks.
    let rg_config = workspace.join("ripgrep.conf");
    std::fs::write(rg_config.as_path(), "--quiet\n").expect("write ripgrep config");
    env.insert(
        "RIPGREP_CONFIG_PATH".to_string(),
        rg_config.to_str().expect("UTF-8 config path").to_string(),
    );
    let rg_fixture = if use_controlled_rg_path {
        let fixture = tempfile::tempdir_in(temp.path().parent().expect("workspace parent"))
            .expect("rg fixture");
        write_executable(
            workspace.join("rg").as_path(),
            "#!/bin/sh\n: > \"$CODEX_TEST_REJECTED_RG_MARKER\"\nexit 0\n",
        )
        .expect("write workspace rg fixture");
        if matches!(rules, DeniedFileRules::GlobsWithProtectedRg) {
            write_executable(
                &fixture.path().join("rg"),
                r#"#!/bin/sh
: > "$CODEX_TEST_PROTECTED_RG_MARKER"
printf '%s\000' "$CODEX_TEST_DENIED_PATH_0" "$CODEX_TEST_DENIED_PATH_1" "$CODEX_TEST_DENIED_PATH_2"
"#,
            )
            .expect("write protected rg fixture");
        }
        // Keep bubblewrap available without adding an unrelated rg to the test PATH.
        if let Some(bwrap) = codex_sandboxing::find_system_bwrap_in_path(
            &PermissionProfile::read_only().file_system_sandbox_policy(),
            &std::env::current_dir().expect("current directory"),
        ) {
            std::os::unix::fs::symlink(bwrap, fixture.path().join("bwrap"))
                .expect("link system bubblewrap");
        }
        env.insert(
            "PATH".to_string(),
            std::env::join_paths([workspace.as_path(), fixture.path()])
                .expect("join PATH")
                .into_string()
                .expect("UTF-8 PATH"),
        );
        for (name, path) in [
            (
                "CODEX_TEST_REJECTED_RG_MARKER",
                fixture.path().join("rejected-rg-ran"),
            ),
            (
                "CODEX_TEST_PROTECTED_RG_MARKER",
                fixture.path().join("protected-rg-ran"),
            ),
            ("CODEX_TEST_DENIED_PATH_0", denied_files[0].0.to_path_buf()),
            ("CODEX_TEST_DENIED_PATH_1", denied_files[1].0.to_path_buf()),
            ("CODEX_TEST_DENIED_PATH_2", denied_files[2].0.to_path_buf()),
        ] {
            env.insert(
                name.to_string(),
                path.to_str().expect("UTF-8 fixture path").to_string(),
            );
        }
        Some(fixture)
    } else {
        None
    };
    match rules {
        DeniedFileRules::ExactPaths => {
            entries.extend(denied_files.iter().map(|(path, _)| {
                FileSystemSandboxEntry::new(path.clone().into(), FileSystemAccessMode::Deny)
            }));
        }
        DeniedFileRules::Globs
        | DeniedFileRules::GlobsFromRoot
        | DeniedFileRules::GlobsWithProtectedRg
        | DeniedFileRules::GlobsWithoutRg => {
            for suffix in [
                "**/*.key",
                "**/.env.local",
                "nest*/.env",
                "#report-*.csv",
                "!*.csv",
                "*.txt ",
                "*.cfg",
            ] {
                entries.push(FileSystemSandboxEntry::new(
                    FileSystemPath::GlobPattern {
                        pattern: format!("{}/{suffix}", workspace.display()),
                    },
                    FileSystemAccessMode::Deny,
                ));
            }
        }
    }
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(entries),
        NetworkSandboxPolicy::Enabled,
    );
    let output = run_cmd_result_with_permission_profile_for_cwd(
        &[
            "/bin/sh",
            "-c",
            r#"set -eu
cd "$1"
/bin/cat AGENTS.md
for blocked in one.key nested/two.key .env.local nested/.env '#report-1.csv' '!report.csv' 'notes.txt ' root-only.cfg; do
    if (: < "$blocked") 2>/dev/null; then
        printf 'read unexpectedly allowed: %s\n' "$blocked" >&2
        exit 10
    fi
    if (printf changed > "$blocked") 2>/dev/null; then
        printf 'write unexpectedly allowed: %s\n' "$blocked" >&2
        exit 11
    fi
done
for allowed in notes.txt nested/allowed.cfg; do
    test "$(/bin/cat "$allowed")" = public
done
for alias in relative.key absolute.key; do
    test "$(/bin/cat "$alias")" = 'project instructions'
done
printf 'allowed\n' > allowed.txt
/bin/cat allowed.txt
"#,
            "sh",
            workspace.to_str().expect("UTF-8 workspace"),
        ],
        if matches!(rules, DeniedFileRules::GlobsFromRoot) {
            AbsolutePathBuf::try_from("/").expect("absolute root")
        } else {
            workspace.clone()
        },
        permission_profile,
        env,
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
    )
    .await
    .expect("sandbox should start with multiple denied files");

    assert_eq!(
        (output.exit_code, output.stdout.text, output.stderr.text),
        (
            0,
            "project instructions\nallowed\n".to_string(),
            String::new()
        )
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("allowed.txt")).expect("read allowed file"),
        "allowed\n"
    );
    for (path, contents) in &denied_files {
        assert_eq!(
            std::fs::read_to_string(path).expect("read host dummy file"),
            *contents
        );
    }
    if let Some(fixture) = rg_fixture {
        assert!(
            !fixture.path().join("rejected-rg-ran").exists(),
            "writable rg ran before confinement"
        );
        assert!(!fixture.path().join("protected-rg-ran").exists());
    }
}
