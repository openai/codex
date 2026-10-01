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
use pretty_assertions::assert_eq;

enum DeniedFileRules {
    ExactPaths,
    Globs,
}

#[test_case::test_case(DeniedFileRules::ExactPaths; "exact_paths")]
#[test_case::test_case(DeniedFileRules::Globs; "globs")]
#[tokio::test]
async fn sandbox_starts_with_multiple_denied_files(rules: DeniedFileRules) {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = AbsolutePathBuf::try_from(temp.path()).expect("absolute workspace");
    std::fs::create_dir(workspace.join("nested")).expect("create nested directory");
    std::fs::write(workspace.join("AGENTS.md"), "project instructions\n")
        .expect("write allowed instructions");
    let denied_files = [
        (workspace.join("one.key"), "dummy first key"),
        (workspace.join("nested/two.key"), "dummy second key"),
        (workspace.join(".env.local"), "dummy environment"),
    ];
    for (path, contents) in &denied_files {
        std::fs::write(path, contents).expect("write denied dummy file");
    }

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
    match rules {
        DeniedFileRules::ExactPaths => {
            entries.extend(denied_files.iter().map(|(path, _)| {
                FileSystemSandboxEntry::new(path.clone().into(), FileSystemAccessMode::Deny)
            }));
        }
        DeniedFileRules::Globs => {
            for suffix in ["**/*.key", "**/.env.local"] {
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
cat AGENTS.md
for blocked in one.key nested/two.key .env.local; do
    if (: < "$blocked") 2>/dev/null; then
        printf 'read unexpectedly allowed: %s\n' "$blocked" >&2
        exit 10
    fi
    if (printf changed > "$blocked") 2>/dev/null; then
        printf 'write unexpectedly allowed: %s\n' "$blocked" >&2
        exit 11
    fi
done
printf 'allowed\n' > allowed.txt
cat allowed.txt
"#,
        ],
        workspace.clone(),
        permission_profile,
        create_env_from_core_vars(),
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
}
