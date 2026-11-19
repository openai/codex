//! Prepare the MXC launch command and report native Windows capability.
//! Probing is cached upstream; reporting happens at most once per process.

use crate::SandboxExecRequest;
use crate::SandboxTransformError;
use crate::manager::SandboxBackend;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;

pub(super) struct Mxc<'a> {
    pub(super) sandbox_exe: Option<&'a Path>,
    pub(super) route: Option<&'a ManagedNetworkSandboxContext>,
}

impl SandboxBackend for Mxc<'_> {
    fn prepare(
        &mut self,
        exec: &mut SandboxExecRequest,
        _cwd: &AbsolutePathBuf,
        policy_cwd: &AbsolutePathBuf,
    ) -> Result<(), SandboxTransformError> {
        let exe = self.sandbox_exe.ok_or_else(|| {
            SandboxTransformError::WindowsMxcPreparation("missing Codex executable path".into())
        })?;
        let args =
            codex_mxc_sandbox::create_command_args(codex_mxc_sandbox::CreateMxcCommandArgsParams {
                command: std::mem::take(&mut exec.command),
                permission_profile: &exec.permission_profile,
                sandbox_policy_cwd: policy_cwd,
                managed_network: self.route,
                env: &mut exec.env,
            })
            .map_err(|err| SandboxTransformError::WindowsMxcPreparation(err.to_string()))?;
        exec.command = std::iter::once(exe.to_string_lossy().into_owned())
            .chain(args)
            .collect();
        Ok(())
    }
}

#[cfg(windows)]
pub(super) fn record_availability_once() {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    static AVAILABILITY_RECORDED: AtomicBool = AtomicBool::new(false);
    // Probe actual PSEC create/close support; symbol presence alone also
    // succeeds on transitional Windows builds where MXC is not enabled.
    let available = codex_mxc_sandbox::is_available();
    if let Some(metrics) = codex_otel::global()
        && !AVAILABILITY_RECORDED.swap(true, Ordering::Relaxed)
    {
        let _ = metrics.counter(
            "codex.windows_mxc.available",
            /*inc*/ 1,
            &[("available", if available { "true" } else { "false" })],
        );
    }
}
