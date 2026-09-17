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
