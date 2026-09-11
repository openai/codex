//! Run an already prepared MXC request with inherited stdio and job ownership.

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use appcontainer_common::base_container_runner::BaseContainerRunner;
use learning_mode_windows::SecurityEnvironmentApi;
use wxc_common::logger::Logger;
use wxc_common::logger::Mode;
use wxc_common::models::ExecutionRequest;
use wxc_common::sandbox_process::SandboxBackend;
use wxc_common::sandbox_process::StdioMode;

// The ETW functions used by tracelogging are documented Advapi32 exports on
// both Windows toolchains; 1.2.3 leaves their import library to the application.
#[link(name = "advapi32")]
unsafe extern "system" {}

pub(super) fn launch(request: &ExecutionRequest) -> Result<i32> {
    ensure!(
        !request
            .policy
            .capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("permissiveLearningMode")),
        "MXC native launch does not support permissiveLearningMode"
    );
    // With no least-privilege mode, legacy proxy, or capture enabled, this
    // probe guarantees BaseContainerRunner chooses PSEC rather than SBOX.
    if !request.policy.denied_paths.is_empty() {
        ensure!(
            SecurityEnvironmentApi::load()?.supports_deny_paths()?,
            "this Windows build cannot enforce native MXC deny paths"
        );
    }
    let mut logger = Logger::new(Mode::Buffer);
    let mut child = BaseContainerRunner::new()
        .spawn(request, &mut logger, StdioMode::Inherit)
        .map_err(|error| anyhow::anyhow!("{}", error.error_message))?;
    // Do not publish the SDK diagnostic buffer: it may contain command or
    // environment data. This wrapper emits only the resulting launch error.
    child.wait().context("waiting for the MXC command")
}
