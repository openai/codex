#![cfg(target_os = "windows")]

mod common;

use anyhow::Result;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LocalFileSystem;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serial_test::serial;
use tempfile::TempDir;

struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

struct SandboxedFileSystem {
    codex_home: TempDir,
    _guard: EnvVarGuard,
    file_system: LocalFileSystem,
}

fn sandboxed_file_system() -> Result<SandboxedFileSystem> {
    let codex_home = TempDir::new()?;
    let guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    let (codex_exe, codex_linux_sandbox_exe) = common::current_test_binary_helper_paths()?;
    let file_system = LocalFileSystem::with_runtime_paths(ExecServerRuntimePaths::new(
        codex_exe,
        codex_linux_sandbox_exe,
    )?);

    Ok(SandboxedFileSystem {
        codex_home,
        _guard: guard,
        file_system,
    })
}

fn root_read_sandbox_with_writes(
    workspace: AbsolutePathBuf,
    writable_roots: Vec<AbsolutePathBuf>,
    windows_sandbox_level: WindowsSandboxLevel,
) -> FileSystemSandboxContext {
    let mut entries = vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }];
    entries.extend(
        writable_roots
            .into_iter()
            .map(|path| FileSystemSandboxEntry {
                path: FileSystemPath::Path { path },
                access: FileSystemAccessMode::Write,
            }),
    );
    let file_system_policy = FileSystemSandboxPolicy::restricted(entries);
    let permissions = PermissionProfile::from_runtime_permissions(
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let mut sandbox =
        FileSystemSandboxContext::from_permission_profile_with_cwd(permissions, workspace);
    sandbox.windows_sandbox_level = windows_sandbox_level;
    sandbox
}

fn workspace_write_sandbox(
    workspace: AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
) -> FileSystemSandboxContext {
    root_read_sandbox_with_writes(workspace.clone(), vec![workspace], windows_sandbox_level)
}

fn read_only_sandbox_with_write_elsewhere(
    workspace: AbsolutePathBuf,
    writable_root: AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
) -> FileSystemSandboxContext {
    root_read_sandbox_with_writes(workspace, vec![writable_root], windows_sandbox_level)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn restricted_token_fs_helper_rejects_read_only_write() -> Result<()> {
    let sandboxed = sandboxed_file_system()?;
    let workspace_dir = TempDir::new()?;
    let workspace = AbsolutePathBuf::try_from(std::fs::canonicalize(workspace_dir.path())?)?;
    let writable_dir = workspace.join("writable");
    std::fs::create_dir_all(&writable_dir)?;
    let blocked_path = workspace.join("blocked.txt");
    let sandbox = read_only_sandbox_with_write_elsewhere(
        workspace,
        writable_dir,
        WindowsSandboxLevel::RestrictedToken,
    );

    let error = sandboxed
        .file_system
        .write_file(&blocked_path, b"blocked".to_vec(), Some(&sandbox))
        .await
        .expect_err("read-only sandboxed helper should reject writes");
    assert!(
        error.to_string().contains("Access is denied")
            || error.to_string().contains("Permission denied")
            || error.to_string().contains("Operation not permitted")
            || error.to_string().contains("is not permitted"),
        "unexpected sandbox denial message: {error}",
    );
    assert_eq!(blocked_path.exists(), false);
    assert!(
        !sandboxed
            .codex_home
            .path()
            .join(".sandbox-bin")
            .read_dir()
            .ok()
            .into_iter()
            .flatten()
            .filter_map(std::result::Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".codex-fs-helper-request-")
            }),
        "request file should be removed after helper exits",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn restricted_token_fs_helper_allows_writable_path() -> Result<()> {
    let sandboxed = sandboxed_file_system()?;
    let workspace_dir = TempDir::new()?;
    let workspace = AbsolutePathBuf::try_from(std::fs::canonicalize(workspace_dir.path())?)?;
    let file_path = workspace.join("allowed.txt");
    let sandbox = workspace_write_sandbox(workspace, WindowsSandboxLevel::RestrictedToken);

    sandboxed
        .file_system
        .write_file(&file_path, b"allowed".to_vec(), Some(&sandbox))
        .await?;

    assert_eq!(std::fs::read(&file_path)?, b"allowed");
    Ok(())
}
