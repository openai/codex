use std::fmt;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LocalFileSystem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::common::exec_server::ExecServerHarness;
use crate::common::exec_server::TestCodexHelperPaths;
use crate::common::exec_server::exec_server;
use crate::common::exec_server::test_codex_helper_paths;

pub(crate) struct FileSystemContext {
    pub(crate) file_system: Arc<dyn ExecutorFileSystem>,
    _helper_paths: Option<TestCodexHelperPaths>,
    _server: Option<ExecServerHarness>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FileSystemImplementation {
    Local,
    Remote,
}

impl fmt::Display for FileSystemImplementation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => formatter.write_str("local"),
            Self::Remote => formatter.write_str("remote"),
        }
    }
}

pub(crate) async fn create_file_system_context(
    implementation: FileSystemImplementation,
) -> Result<FileSystemContext> {
    match implementation {
        FileSystemImplementation::Local => {
            let helper_paths = test_codex_helper_paths()?;
            let runtime_paths = ExecServerRuntimePaths::new(
                helper_paths.codex_exe.clone(),
                helper_paths.codex_linux_sandbox_exe.clone(),
            )?;
            Ok(FileSystemContext {
                file_system: Arc::new(LocalFileSystem::with_runtime_paths(runtime_paths)),
                _helper_paths: Some(helper_paths),
                _server: None,
            })
        }
        FileSystemImplementation::Remote => {
            let server = exec_server().await?;
            let environment =
                Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
            Ok(FileSystemContext {
                file_system: environment.get_filesystem(),
                _helper_paths: None,
                _server: Some(server),
            })
        }
    }
}

pub(crate) fn absolute_path(path: impl AsRef<Path>) -> AbsolutePathBuf {
    let path = path.as_ref().to_path_buf();
    assert!(
        path.is_absolute(),
        "path must be absolute: {}",
        path.display()
    );
    match AbsolutePathBuf::try_from(path) {
        Ok(path) => path,
        Err(err) => panic!("path should be absolute: {err}"),
    }
}

pub(crate) fn read_only_sandbox(readable_root: std::path::PathBuf) -> FileSystemSandboxContext {
    let readable_root = absolute_path(readable_root);
    sandbox_context(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Path {
            path: readable_root,
        },
        access: FileSystemAccessMode::Read,
    }])
}

pub(crate) fn workspace_write_sandbox(
    writable_root: std::path::PathBuf,
) -> FileSystemSandboxContext {
    let writable_root = absolute_path(writable_root);
    sandbox_context(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Path {
            path: writable_root,
        },
        access: FileSystemAccessMode::Write,
    }])
}

fn sandbox_context(entries: Vec<FileSystemSandboxEntry>) -> FileSystemSandboxContext {
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(entries),
        NetworkSandboxPolicy::Restricted,
    ))
}
