use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::io;

use super::*;

/// Reads and writes follow their own policies, and temporary rules follow the executor's OS.
#[test]
fn sandbox_routing_uses_independent_read_write_policies_and_executor_temp_rules() {
    use FileSystemAccessMode::Deny;
    use FileSystemAccessMode::Read;
    use FileSystemAccessMode::Write;
    use FileSystemSpecialPath::Minimal;
    use FileSystemSpecialPath::Root;
    use FileSystemSpecialPath::SlashTmp;
    use FileSystemSpecialPath::Tmpdir;

    for (cwd, slash_tmp_applies, foreign_path) in [
        ("file:///workspace", true, "file:///C:/private"),
        ("file:///C:/workspace", false, "file:///private"),
    ] {
        let cwd = PathUri::parse(cwd).expect("valid executor cwd");
        for (entries, expected) in [
            (vec![(Root, Read)], (false, true)),
            (vec![(Root, Write)], (false, false)),
            (vec![(Root, Read), (Minimal, Deny)], (true, true)),
            (vec![(Root, Write), (Minimal, Deny)], (true, false)),
            (
                vec![(Root, Read), (SlashTmp, Deny)],
                (slash_tmp_applies, true),
            ),
            (
                vec![(Root, Write), (SlashTmp, Deny)],
                (slash_tmp_applies, slash_tmp_applies),
            ),
            (vec![(Root, Read), (Tmpdir, Deny)], (true, true)),
            (vec![(Root, Write), (Tmpdir, Deny)], (true, true)),
        ] {
            let policy = FileSystemSandboxPolicy::restricted(
                entries
                    .into_iter()
                    .map(|(value, access)| {
                        FileSystemSandboxEntry::new(FileSystemPath::Special { value }, access)
                    })
                    .collect(),
            );
            let context = FileSystemSandboxContext::from_permission_profile(
                PermissionProfile::from_runtime_permissions(
                    &policy,
                    NetworkSandboxPolicy::Restricted,
                ),
                cwd.clone(),
            );
            assert_eq!(
                (
                    context.should_read_from_sandbox(),
                    context.should_write_into_sandbox()
                ),
                expected,
                "cwd={cwd}, policy={policy:?}",
            );
        }

        let foreign = PathUri::parse(foreign_path).expect("valid foreign path");
        for (entries, expected) in [
            (
                vec![FileSystemSandboxEntry::new(cwd.clone().into(), Read)],
                (true, true),
            ),
            (
                vec![
                    FileSystemSandboxEntry::new(FileSystemPath::Special { value: Root }, Write),
                    FileSystemSandboxEntry::new(foreign.clone().into(), Deny),
                ],
                (true, true),
            ),
            (
                vec![
                    FileSystemSandboxEntry::new(FileSystemPath::Special { value: Root }, Read),
                    FileSystemSandboxEntry::new(foreign.clone().into(), Write),
                ],
                (false, true),
            ),
            (
                vec![
                    FileSystemSandboxEntry::new(FileSystemPath::Special { value: Root }, Write),
                    FileSystemSandboxEntry::new(foreign.into(), Write),
                ],
                (false, false),
            ),
        ] {
            let policy = FileSystemSandboxPolicy::restricted(entries);
            let context = FileSystemSandboxContext::from_permission_profile(
                PermissionProfile::from_runtime_permissions(
                    &policy,
                    NetworkSandboxPolicy::Restricted,
                ),
                cwd.clone(),
            );
            assert_eq!(
                (
                    context.should_read_from_sandbox(),
                    context.should_write_into_sandbox()
                ),
                expected,
                "cwd={cwd}, policy={policy:?}",
            );
        }
    }

    let opaque = PathUri::parse("file:///%00/bad/path/YQ").expect("valid opaque path");
    let context =
        FileSystemSandboxContext::from_permission_profile(PermissionProfile::read_only(), opaque);
    assert_eq!(
        (
            context.should_read_from_sandbox(),
            context.should_write_into_sandbox()
        ),
        (false, true),
    );
}

#[tokio::test]
async fn sandboxed_file_system_rejects_non_native_uri_as_invalid_input() {
    let runtime_paths = ExecServerRuntimePaths::new(
        std::env::current_exe().expect("current exe"),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths");
    let file_system = SandboxedFileSystem::new(runtime_paths.clone());
    let cwd = PathUri::from_host_native_path(std::env::temp_dir()).expect("native temporary cwd");
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(Vec::new()),
            NetworkSandboxPolicy::Restricted,
        ),
        cwd.clone(),
    );

    let error = file_system
        .read_file(&non_native_uri(), Default::default(), Some(&sandbox))
        .await
        .expect_err("non-native URI should be rejected");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

    // A foreign permission path must not select the unsandboxed filesystem, even with a root grant.
    let file_system = crate::LocalFileSystem::with_runtime_paths(runtime_paths);
    let foreign = non_native_uri();
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(foreign.clone().into(), FileSystemAccessMode::Write),
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd,
    );
    let readable = tempfile::NamedTempFile::new().expect("readable file");
    let error = file_system
        .read_file(
            &PathUri::from_host_native_path(readable.path()).expect("readable file URI"),
            Default::default(),
            Some(&sandbox),
        )
        .await
        .expect_err("foreign permission path must not allow unsandboxed access");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains(&foreign.to_string()));
}

fn non_native_uri() -> PathUri {
    #[cfg(unix)]
    let uri = "file://server/share/file.txt";
    #[cfg(windows)]
    let uri = "file:///usr/local/file.txt";

    match PathUri::parse(uri) {
        Ok(uri) => uri,
        Err(err) => panic!("valid non-native URI should parse: {err}"),
    }
}
