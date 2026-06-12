#![cfg(target_os = "windows")]

mod common;

use anyhow::Context;
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
use std::path::Path;
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
    _codex_home: TempDir,
    _guard: EnvVarGuard,
    file_system: LocalFileSystem,
}

fn sandboxed_file_system() -> Result<SandboxedFileSystem> {
    let (codex_exe, _) = common::current_test_binary_helper_paths()?;
    sandboxed_file_system_with_codex_exe(codex_exe)
}

fn sandboxed_file_system_with_codex_exe(
    codex_exe: std::path::PathBuf,
) -> Result<SandboxedFileSystem> {
    let codex_home = TempDir::new()?;
    let guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    let file_system = LocalFileSystem::with_runtime_paths(ExecServerRuntimePaths::new(
        codex_exe, /*codex_linux_sandbox_exe*/ None,
    )?);

    Ok(SandboxedFileSystem {
        _codex_home: codex_home,
        _guard: guard,
        file_system,
    })
}

fn process_is_elevated() -> bool {
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$principal = [Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent(); if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { exit 0 } else { exit 1 }",
        ])
        .status();
    status.is_ok_and(|status| status.success())
}

fn stage_windows_sandbox_helpers_next_to_codex_exe(codex_exe: &Path) -> Result<()> {
    let helper_dir = codex_exe
        .parent()
        .context("configured Codex helper path has no parent")?;
    std::fs::create_dir_all(helper_dir)
        .with_context(|| format!("create helper dir {}", helper_dir.display()))?;
    for helper_name in ["codex-windows-sandbox-setup", "codex-command-runner"] {
        let helper = codex_utils_cargo_bin::cargo_bin(helper_name)
            .with_context(|| format!("locate Windows sandbox helper binary {helper_name}"))?;
        std::fs::copy(
            &helper,
            helper_dir.join(Path::new(helper_name).with_extension("exe")),
        )
        .with_context(|| format!("stage Windows sandbox helper {}", helper.display()))?;
    }
    Ok(())
}

fn temp_workspace() -> Result<(TempDir, AbsolutePathBuf)> {
    let dir = TempDir::new()?;
    let path = AbsolutePathBuf::try_from(std::fs::canonicalize(dir.path())?)?;
    Ok((dir, path))
}

fn root_read_entry() -> FileSystemSandboxEntry {
    FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }
}

fn path_entry(path: AbsolutePathBuf, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry {
        path: FileSystemPath::Path { path },
        access,
    }
}

fn sandbox_from_entries(
    workspace: AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
    entries: Vec<FileSystemSandboxEntry>,
) -> FileSystemSandboxContext {
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
    sandbox_from_entries(
        workspace.clone(),
        windows_sandbox_level,
        vec![
            root_read_entry(),
            path_entry(workspace, FileSystemAccessMode::Write),
        ],
    )
}

fn workspace_write_sandbox_with_read_only_path(
    workspace: AbsolutePathBuf,
    read_only_path: AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
) -> FileSystemSandboxContext {
    sandbox_from_entries(
        workspace.clone(),
        windows_sandbox_level,
        vec![
            root_read_entry(),
            path_entry(workspace, FileSystemAccessMode::Write),
            path_entry(read_only_path, FileSystemAccessMode::Read),
        ],
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn restricted_token_fs_helper_uses_configured_helper_path() -> Result<()> {
    let helper_dir = TempDir::new()?;
    let current_exe = std::env::current_exe()?;
    let configured_helper = helper_dir.path().join("configured-codex-helper.exe");
    std::fs::copy(&current_exe, &configured_helper)?;
    let sandboxed = sandboxed_file_system_with_codex_exe(configured_helper.clone())?;
    let (_workspace_dir, workspace) = temp_workspace()?;
    let file_path = workspace.join("allowed.txt");
    let sandbox = workspace_write_sandbox(workspace, WindowsSandboxLevel::RestrictedToken);

    sandboxed
        .file_system
        .write_file(&file_path, b"allowed".to_vec(), Some(&sandbox))
        .await?;

    let materialized_helper = sandboxed._codex_home.path().join(".sandbox-bin").join(
        configured_helper
            .file_name()
            .expect("configured helper name"),
    );
    assert_eq!(std::fs::read(&file_path)?, b"allowed");
    assert_eq!(materialized_helper.exists(), true);
    assert_eq!(
        materialized_helper.file_name(),
        configured_helper.file_name()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn restricted_token_fs_helper_rejects_split_read_restrictions() -> Result<()> {
    let sandboxed = sandboxed_file_system()?;
    let (_workspace_dir, workspace) = temp_workspace()?;
    let readable_dir = workspace.join("readable");
    std::fs::create_dir(&readable_dir)?;
    let blocked_path = workspace.join("blocked.txt");
    std::fs::write(&blocked_path, b"blocked")?;
    let sandbox = sandbox_from_entries(
        workspace,
        WindowsSandboxLevel::RestrictedToken,
        vec![path_entry(readable_dir, FileSystemAccessMode::Read)],
    );

    let error = sandboxed
        .file_system
        .read_file(&blocked_path, Some(&sandbox))
        .await
        .expect_err("restricted-token fs helper should reject split read restrictions");

    assert!(
        error
            .to_string()
            .contains("cannot enforce split filesystem read restrictions"),
        "unexpected split-read sandbox error: {error}",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn restricted_token_fs_helper_rejects_missing_read_only_carveout() -> Result<()> {
    let sandboxed = sandboxed_file_system()?;
    let (_workspace_dir, workspace) = temp_workspace()?;
    let protected_dir = workspace.join("protected");
    let blocked_path = protected_dir.join("blocked.txt");
    let sandbox = workspace_write_sandbox_with_read_only_path(
        workspace,
        protected_dir,
        WindowsSandboxLevel::RestrictedToken,
    );

    let error = sandboxed
        .file_system
        .write_file(&blocked_path, b"blocked".to_vec(), Some(&sandbox))
        .await
        .expect_err("read-only carveout should reject writes even before it exists");
    assert!(
        error.to_string().contains("Access is denied")
            || error.to_string().contains("Permission denied")
            || error.to_string().contains("Operation not permitted")
            || error.to_string().contains("is not permitted"),
        "unexpected sandbox denial message: {error}",
    );
    assert_eq!(blocked_path.exists(), false);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn elevated_fs_helper_rejects_unenforceable_reopened_writable_descendant() -> Result<()> {
    let sandboxed = sandboxed_file_system()?;
    let (_workspace_dir, workspace) = temp_workspace()?;
    let read_only_dir = workspace.join("read-only");
    let reopened_dir = read_only_dir.join("writable");
    let target_path = reopened_dir.join("blocked.txt");
    let sandbox = sandbox_from_entries(
        workspace.clone(),
        WindowsSandboxLevel::Elevated,
        vec![
            root_read_entry(),
            path_entry(workspace, FileSystemAccessMode::Write),
            path_entry(read_only_dir, FileSystemAccessMode::Read),
            path_entry(reopened_dir, FileSystemAccessMode::Write),
        ],
    );

    let error = sandboxed
        .file_system
        .write_file(&target_path, b"blocked".to_vec(), Some(&sandbox))
        .await
        .expect_err("elevated fs helper should reject unenforceable reopened descendants");

    assert!(
        error.to_string().contains(
            "windows elevated sandbox cannot reopen writable descendants under read-only carveouts"
        ),
        "unexpected elevated sandbox error: {error}",
    );
    assert_eq!(target_path.exists(), false);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn elevated_fs_helper_allows_writable_path_when_setup_is_available() -> Result<()> {
    if !process_is_elevated() {
        eprintln!("skipping elevated fs-helper write: test process is not elevated");
        return Ok(());
    }

    let helper_dir = TempDir::new()?;
    let configured_helper = helper_dir.path().join("codex.exe");
    std::fs::copy(std::env::current_exe()?, &configured_helper)?;
    stage_windows_sandbox_helpers_next_to_codex_exe(&configured_helper)?;
    let sandboxed = sandboxed_file_system_with_codex_exe(configured_helper)?;

    let (_workspace_dir, workspace) = temp_workspace()?;
    let file_path = workspace.join("allowed-elevated.txt");
    let sandbox = workspace_write_sandbox(workspace, WindowsSandboxLevel::Elevated);

    sandboxed
        .file_system
        .write_file(&file_path, b"allowed elevated".to_vec(), Some(&sandbox))
        .await?;

    assert_eq!(std::fs::read(&file_path)?, b"allowed elevated");
    Ok(())
}
