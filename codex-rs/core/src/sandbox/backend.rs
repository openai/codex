use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;

use crate::error::Result;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::process_exec_tool_call;
use crate::protocol::SandboxPolicy;

#[async_trait]
pub trait SpawnBackend: Send + Sync {
    fn sandbox_type(&self) -> SandboxType;

    async fn spawn(
        &self,
        params: ExecParams,
        sandbox_policy: &SandboxPolicy,
        sandbox_cwd: &Path,
        codex_linux_sandbox_exe: &Option<PathBuf>,
        stdout_stream: Option<StdoutStream>,
    ) -> Result<ExecToolCallOutput> {
        process_exec_tool_call(
            params,
            self.sandbox_type(),
            sandbox_policy,
            sandbox_cwd,
            codex_linux_sandbox_exe,
            stdout_stream,
        )
        .await
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DirectBackend;

#[async_trait]
impl SpawnBackend for DirectBackend {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::None
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SeatbeltBackend;

#[async_trait]
impl SpawnBackend for SeatbeltBackend {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::MacosSeatbelt
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxBackend;

#[async_trait]
impl SpawnBackend for LinuxBackend {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::LinuxSeccomp
    }
}

#[derive(Default)]
pub struct BackendRegistry {
    direct: DirectBackend,
    seatbelt: SeatbeltBackend,
    linux: LinuxBackend,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_type(&self, sandbox: SandboxType) -> &dyn SpawnBackend {
        match sandbox {
            SandboxType::None => &self.direct,
            SandboxType::MacosSeatbelt => &self.seatbelt,
            SandboxType::LinuxSeccomp => &self.linux,
        }
    }
}
