//! Run an already prepared MXC request with inherited stdio and job ownership.

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use mxc_sdk::mxc_common::logger::Logger;
use mxc_sdk::mxc_common::logger::Mode;
use mxc_sdk::mxc_common::models::ExecutionRequest;
use mxc_sdk::mxc_common::sandbox_process::SandboxBackend;
use mxc_sdk::mxc_common::sandbox_process::StdioMode;
use mxc_sdk::process_container_common::base_container_runner::BaseContainerRunner;

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
    if !request.policy.denied_paths.is_empty() {
        ensure!(
            BaseContainerRunner::supports_native_denied_paths(),
            "this Windows build cannot enforce native MXC deny paths"
        );
    }
    let mut logger = Logger::new(Mode::Buffer);
    let mut child = BaseContainerRunner::new()
        .spawn(request, &mut logger, StdioMode::Inherit)
        .map_err(|error| match create_process_error(&error.extended_error) {
            Some(code) => anyhow::anyhow!(
                "{} (CreateProcessW, Win32 error {code})",
                error.error_message
            ),
            None => anyhow::anyhow!("{}", error.error_message),
        })?;
    // Do not publish the SDK diagnostic buffer: it may contain command or
    // environment data. This wrapper emits only the resulting launch error.
    child.wait().context("waiting for the MXC command")
}

// The SDK appends the working directory and other diagnostics to this field.
// Only the Windows error number is safe to copy into the command result.
fn create_process_error(detail: &str) -> Option<u32> {
    let number = detail
        .strip_prefix(
            "CreateProcessW(PROC_THREAD_ATTRIBUTE_SECURITY_ENVIRONMENT) failed: WIN32_ERROR(",
        )?
        .split_once(')')?
        .0;
    number.parse().ok()
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
