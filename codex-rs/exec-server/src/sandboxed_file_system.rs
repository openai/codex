use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::local_file_system::LocalFileSystem;

const INTERNAL_FS_OP_FLAG: &str = "--internal-fs-op";
const HELPER_WINDOWS_SANDBOX_LEVEL: WindowsSandboxLevel = WindowsSandboxLevel::RestrictedToken;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SandboxedFileSystemRequest {
    ReadFile {
        path: AbsolutePathBuf,
    },
    WriteFile {
        path: AbsolutePathBuf,
        data_base64: String,
    },
    CreateDirectory {
        path: AbsolutePathBuf,
        recursive: bool,
    },
    GetMetadata {
        path: AbsolutePathBuf,
    },
    ReadDirectory {
        path: AbsolutePathBuf,
    },
    Remove {
        path: AbsolutePathBuf,
        recursive: bool,
        force: bool,
    },
    Copy {
        source_path: AbsolutePathBuf,
        destination_path: AbsolutePathBuf,
        recursive: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SandboxedFileSystemResponse {
    Unit,
    ReadFile { data_base64: String },
    GetMetadata { metadata: FileMetadata },
    ReadDirectory { entries: Vec<ReadDirectoryEntry> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SandboxedFileSystemEnvelope {
    Ok(SandboxedFileSystemResponse),
    Error(SandboxedFileSystemError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SandboxedFileSystemError {
    kind: SandboxedFileSystemErrorKind,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SandboxedFileSystemErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    Unsupported,
    Other,
}

impl SandboxedFileSystemErrorKind {
    fn from_io_kind(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            io::ErrorKind::InvalidInput => Self::InvalidInput,
            io::ErrorKind::Unsupported => Self::Unsupported,
            _ => Self::Other,
        }
    }

    fn to_io_kind(self) -> io::ErrorKind {
        match self {
            Self::NotFound => io::ErrorKind::NotFound,
            Self::PermissionDenied => io::ErrorKind::PermissionDenied,
            Self::AlreadyExists => io::ErrorKind::AlreadyExists,
            Self::InvalidInput => io::ErrorKind::InvalidInput,
            Self::Unsupported => io::ErrorKind::Unsupported,
            Self::Other => io::ErrorKind::Other,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SandboxedFileSystem {
    sandbox_policy: FileSystemSandboxPolicy,
}

impl SandboxedFileSystem {
    pub(crate) fn new(sandbox_policy: FileSystemSandboxPolicy) -> Self {
        Self { sandbox_policy }
    }

    async fn execute_request_via_sandbox(
        &self,
        request: SandboxedFileSystemRequest,
    ) -> FileSystemResult<SandboxedFileSystemResponse> {
        let prepared = prepare_helper_request(&self.sandbox_policy)?;
        let request_bytes = serde_json::to_vec(&request).map_err(io::Error::other)?;
        let (program, args) = prepared.command.split_first().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "helper command is empty")
        })?;

        let mut command = Command::new(program);
        command.args(args);
        command.current_dir(&prepared.cwd);
        command.envs(&prepared.env);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        #[cfg(unix)]
        {
            if let Some(arg0) = prepared.arg0.as_ref() {
                command.arg0(arg0);
            }
        }

        let mut child = command.spawn()?;
        let Some(mut stdin) = child.stdin.take() else {
            return Err(io::Error::other("sandbox helper stdin was not piped"));
        };
        stdin.write_all(&request_bytes).await?;
        drop(stdin);

        let output = child.wait_with_output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else if !stdout.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                "sandbox helper exited without error output".to_string()
            };
            return Err(io::Error::other(message));
        }

        let envelope: SandboxedFileSystemEnvelope =
            serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
        match envelope {
            SandboxedFileSystemEnvelope::Ok(response) => Ok(response),
            SandboxedFileSystemEnvelope::Error(error) => {
                Err(io::Error::new(error.kind.to_io_kind(), error.message))
            }
        }
    }
}

#[async_trait]
impl ExecutorFileSystem for SandboxedFileSystem {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::ReadFile {
                path: path.clone(),
            })
            .await?;
        let SandboxedFileSystemResponse::ReadFile { data_base64 } = response else {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for read_file",
            ));
        };
        STANDARD.decode(data_base64).map_err(|err| {
            io::Error::other(format!("sandbox helper returned invalid base64: {err}"))
        })
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::WriteFile {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
            })
            .await?;
        if !matches!(response, SandboxedFileSystemResponse::Unit) {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for write_file",
            ));
        }
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::CreateDirectory {
                path: path.clone(),
                recursive: options.recursive,
            })
            .await?;
        if !matches!(response, SandboxedFileSystemResponse::Unit) {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for create_directory",
            ));
        }
        Ok(())
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::GetMetadata {
                path: path.clone(),
            })
            .await?;
        let SandboxedFileSystemResponse::GetMetadata { metadata } = response else {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for get_metadata",
            ));
        };
        Ok(metadata)
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::ReadDirectory {
                path: path.clone(),
            })
            .await?;
        let SandboxedFileSystemResponse::ReadDirectory { entries } = response else {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for read_directory",
            ));
        };
        Ok(entries)
    }

    async fn remove(&self, path: &AbsolutePathBuf, options: RemoveOptions) -> FileSystemResult<()> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::Remove {
                path: path.clone(),
                recursive: options.recursive,
                force: options.force,
            })
            .await?;
        if !matches!(response, SandboxedFileSystemResponse::Unit) {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for remove",
            ));
        }
        Ok(())
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
    ) -> FileSystemResult<()> {
        let response = self
            .execute_request_via_sandbox(SandboxedFileSystemRequest::Copy {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: options.recursive,
            })
            .await?;
        if !matches!(response, SandboxedFileSystemResponse::Unit) {
            return Err(io::Error::other(
                "sandbox helper returned unexpected response for copy",
            ));
        }
        Ok(())
    }
}

pub async fn run_internal_fs_op() -> io::Result<()> {
    let mut input = Vec::new();
    tokio::io::stdin().read_to_end(&mut input).await?;
    let request: SandboxedFileSystemRequest =
        serde_json::from_slice(&input).map_err(io::Error::other)?;
    let response = match execute_internal_request(request).await {
        Ok(response) => SandboxedFileSystemEnvelope::Ok(response),
        Err(err) => SandboxedFileSystemEnvelope::Error(SandboxedFileSystemError {
            kind: SandboxedFileSystemErrorKind::from_io_kind(err.kind()),
            message: err.to_string(),
        }),
    };
    let bytes = serde_json::to_vec(&response).map_err(io::Error::other)?;
    let mut stdout = tokio::io::stdout();
    stdout.write_all(&bytes).await?;
    stdout.flush().await?;
    Ok(())
}

async fn execute_internal_request(
    request: SandboxedFileSystemRequest,
) -> FileSystemResult<SandboxedFileSystemResponse> {
    let file_system = LocalFileSystem;
    match request {
        SandboxedFileSystemRequest::ReadFile { path } => {
            let bytes = file_system.read_file(&path).await?;
            Ok(SandboxedFileSystemResponse::ReadFile {
                data_base64: STANDARD.encode(bytes),
            })
        }
        SandboxedFileSystemRequest::WriteFile { path, data_base64 } => {
            let bytes = STANDARD.decode(data_base64).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid writeFile base64 payload: {err}"),
                )
            })?;
            file_system.write_file(&path, bytes).await?;
            Ok(SandboxedFileSystemResponse::Unit)
        }
        SandboxedFileSystemRequest::CreateDirectory { path, recursive } => {
            file_system
                .create_directory(&path, CreateDirectoryOptions { recursive })
                .await?;
            Ok(SandboxedFileSystemResponse::Unit)
        }
        SandboxedFileSystemRequest::GetMetadata { path } => {
            let metadata = file_system.get_metadata(&path).await?;
            Ok(SandboxedFileSystemResponse::GetMetadata { metadata })
        }
        SandboxedFileSystemRequest::ReadDirectory { path } => {
            let entries = file_system.read_directory(&path).await?;
            Ok(SandboxedFileSystemResponse::ReadDirectory { entries })
        }
        SandboxedFileSystemRequest::Remove {
            path,
            recursive,
            force,
        } => {
            file_system
                .remove(&path, RemoveOptions { recursive, force })
                .await?;
            Ok(SandboxedFileSystemResponse::Unit)
        }
        SandboxedFileSystemRequest::Copy {
            source_path,
            destination_path,
            recursive,
        } => {
            file_system
                .copy(&source_path, &destination_path, CopyOptions { recursive })
                .await?;
            Ok(SandboxedFileSystemResponse::Unit)
        }
    }
}

fn prepare_helper_request(
    sandbox_policy: &FileSystemSandboxPolicy,
) -> io::Result<SandboxExecRequest> {
    let helper_exe = helper_executable_path()?;
    let codex_linux_sandbox_exe = linux_sandbox_executable_path(&helper_exe);
    let helper_cwd = std::env::current_dir()?;
    let network_policy = NetworkSandboxPolicy::Enabled;
    let helper_readable_roots =
        helper_readable_roots(&helper_exe, codex_linux_sandbox_exe.as_ref());
    let effective_file_system_policy = sandbox_policy
        .clone()
        .with_additional_readable_roots(helper_cwd.as_path(), &helper_readable_roots);
    let legacy_policy = helper_legacy_policy(
        &effective_file_system_policy,
        network_policy,
        helper_cwd.as_path(),
    )?;
    let manager = SandboxManager::new();
    let sandbox = manager.select_initial(
        &effective_file_system_policy,
        network_policy,
        codex_sandboxing::SandboxablePreference::Auto,
        HELPER_WINDOWS_SANDBOX_LEVEL,
        /*has_managed_network_requirements*/ false,
    );
    let command = SandboxCommand {
        program: helper_exe.into(),
        args: vec![INTERNAL_FS_OP_FLAG.to_string()],
        cwd: helper_cwd.clone(),
        env: HashMap::new(),
        additional_permissions: None,
    };
    manager
        .transform(SandboxTransformRequest {
            command,
            policy: &legacy_policy,
            file_system_policy: &effective_file_system_policy,
            network_policy,
            sandbox,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: helper_cwd.as_path(),
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.as_ref(),
            use_legacy_landlock: false,
            windows_sandbox_level: HELPER_WINDOWS_SANDBOX_LEVEL,
            windows_sandbox_private_desktop: false,
        })
        .map_err(io::Error::other)
}

fn helper_readable_roots(
    helper_exe: &Path,
    codex_linux_sandbox_exe: Option<&PathBuf>,
) -> Vec<AbsolutePathBuf> {
    let mut roots = Vec::new();

    for path in
        std::iter::once(helper_exe).chain(codex_linux_sandbox_exe.into_iter().map(PathBuf::as_path))
    {
        if let Some(parent) = path.parent()
            && let Ok(root) = AbsolutePathBuf::from_absolute_path(parent)
            && !roots.iter().any(|existing| existing == &root)
        {
            roots.push(root);
        }
    }

    roots
}

fn helper_legacy_policy(
    sandbox_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    helper_cwd: &Path,
) -> io::Result<SandboxPolicy> {
    match sandbox_policy.to_legacy_sandbox_policy(network_policy, helper_cwd) {
        Ok(policy) => Ok(policy),
        Err(_) if sandbox_policy.needs_direct_runtime_enforcement(network_policy, helper_cwd) => {
            Ok(SandboxPolicy::ExternalSandbox {
                network_access: if network_policy.is_enabled() {
                    NetworkAccess::Enabled
                } else {
                    NetworkAccess::Restricted
                },
            })
        }
        Err(err) => Err(err),
    }
}

fn helper_executable_path() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_EXEC_SERVER_SELF_EXE") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_codex-exec-server") {
        return Ok(PathBuf::from(path));
    }
    if let Ok(path) = which::which("codex-exec-server") {
        return Ok(path);
    }

    let current_exe = std::env::current_exe()?;
    if current_exe.file_name().and_then(|value| value.to_str()) == Some("codex-exec-server") {
        return Ok(current_exe);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not find codex-exec-server executable for sandboxed filesystem helper",
    ))
}

fn linux_sandbox_executable_path(helper_exe: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    std::env::var_os("CODEX_LINUX_SANDBOX_EXE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("CARGO_BIN_EXE_codex-linux-sandbox").map(PathBuf::from))
        .or_else(|| which::which("codex-linux-sandbox").ok())
        .or_else(|| {
            helper_exe.parent().and_then(|parent| {
                let sibling = parent.join("codex-linux-sandbox");
                sibling.exists().then_some(sibling)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::HELPER_WINDOWS_SANDBOX_LEVEL;
    use super::helper_legacy_policy;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_protocol::protocol::NetworkAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;

    #[test]
    fn helper_legacy_policy_falls_back_to_external_sandbox_for_direct_runtime_enforcement() {
        let outside_workspace =
            AbsolutePathBuf::try_from(std::path::PathBuf::from("/outside")).expect("absolute path");
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: outside_workspace,
            },
            access: FileSystemAccessMode::Write,
        }]);

        let resolved = helper_legacy_policy(
            &policy,
            NetworkSandboxPolicy::Enabled,
            std::path::Path::new("/workspace"),
        )
        .expect("direct-enforcement policy should resolve");

        assert_eq!(
            resolved,
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            }
        );
    }

    #[test]
    fn helper_sandbox_requests_use_restricted_windows_sandbox_level() {
        assert_eq!(
            HELPER_WINDOWS_SANDBOX_LEVEL,
            WindowsSandboxLevel::RestrictedToken
        );
    }
}
