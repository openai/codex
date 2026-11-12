//! Retains the distinct native registration failure codes and deployment activity ID.
//! Diagnostic getters never replace the completion error or turn success into failure.
//! Free-form Windows ErrorText stays in Windows deployment diagnostics, not this summary.

use anyhow::Context;
use anyhow::Result;
use windows::Foundation::AsyncStatus;
use windows::Foundation::IAsyncOperationWithProgress;
use windows::Management::Deployment::DeploymentProgress;
use windows::Management::Deployment::DeploymentResult;
use windows::core::GUID;
use windows::core::HRESULT;

pub(super) struct RegistrationResult {
    status: AsyncStatus,
    async_error: windows::core::Result<HRESULT>,
    // Keep GetResults failure separate from failure to read ExtendedErrorCode.
    result: windows::core::Result<DeploymentDetails>,
}

struct DeploymentDetails {
    extended_error: windows::core::Result<HRESULT>,
    activity_id: windows::core::Result<GUID>,
}

impl RegistrationResult {
    /// The caller must first observe a terminal status while retaining the profile/token.
    pub(super) fn read(
        operation: &IAsyncOperationWithProgress<DeploymentResult, DeploymentProgress>,
        status: AsyncStatus,
    ) -> Self {
        Self {
            status,
            async_error: operation.ErrorCode(),
            result: operation.GetResults().map(|result| DeploymentDetails {
                extended_error: result.ExtendedErrorCode(),
                activity_id: result.ActivityId(),
            }),
        }
    }

    pub(super) fn check(self, registered_for_target: impl FnOnce() -> bool) -> Result<()> {
        let Self {
            status,
            async_error,
            result,
        } = self;
        let async_error = match async_error {
            Ok(code) => format!("{code:?}"),
            Err(error) => format!("unavailable({:?})", error.code()),
        };
        let context = format!(
            "finish managed runtime package registration: status={} async_error={async_error}",
            status.0,
        );
        let details = result.map_err(|error| {
            let summary = format!("{context} phase=GetResults get_results={:?}", error.code());
            anyhow::Error::new(error).context(summary)
        })?;
        let activity_id = match details.activity_id {
            Ok(id) => format!("{id:?}"),
            Err(error) => format!("unavailable({:?})", error.code()),
        };
        let context = format!("{context} get_results=0x00000000 activity_id={activity_id}");
        let extended_error = details.extended_error.map_err(|error| {
            let summary = format!(
                "{context} phase=ExtendedErrorCode getter_error={:?}",
                error.code()
            );
            anyhow::Error::new(error).context(summary)
        })?;
        // Windows can report the service privilege error after another deployment
        // registered this same version for the target account. Trust that exact
        // postcondition only for this error. The runner still verifies its image
        // and package identity before accepting a command.
        if status == AsyncStatus::Error
            && extended_error == HRESULT::from_win32(
                windows_sys::Win32::Foundation::ERROR_PACKAGED_SERVICE_REQUIRES_ADMIN_PRIVILEGES,
            )
            && registered_for_target()
        {
            return Ok(());
        }
        extended_error.ok().with_context(|| {
            format!("{context} phase=deployment extended_error={extended_error:?}")
        })
    }
}

#[cfg(test)]
#[path = "registration_result_tests.rs"]
mod tests;
