//! Keep each packaged channel's service and pipe separate while accounts remain shared.
//! An unpackaged client may use a routing hint; service admission still verifies OS identity.

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use windows_sys::Win32::System::Threading::GetCurrentProcess;

use crate::package_identity::process_package_family;

/// SCM name matching the packaged application's manifest. Unpackaged callers keep
/// the legacy service lookup and may use ordinary elevated setup when it is absent.
pub fn windows_sandbox_service_name() -> Result<String> {
    let Some(family) = service_package_family()? else {
        return Ok("CodexSandboxService".into());
    };
    let (name, _) = family.rsplit_once('_').context("invalid package family")?;
    Ok(format!("CodexSandboxService.{name}"))
}

/// The full family, including publisher identity, qualifies the authenticated pipe.
pub fn windows_sandbox_service_pipe_name() -> Result<String> {
    Ok(match service_package_family()? {
        Some(family) => format!("{}.{}", crate::SANDBOX_PROVISIONING_PIPE_NAME, family),
        None => crate::SANDBOX_PROVISIONING_PIPE_NAME.into(),
    })
}

pub(crate) fn current_family() -> Result<Option<String>> {
    unsafe { process_package_family(GetCurrentProcess()) }
}

fn service_package_family() -> Result<Option<String>> {
    if let Some(family) = current_family()? {
        return Ok(Some(family));
    }
    let Some(hint) = std::env::var_os("CODEX_WINDOWS_SANDBOX_PACKAGE_FAMILY") else {
        return Ok(None);
    };
    let family = hint
        .to_str()
        .context("service package family is not UTF-8")?;
    validate_service_family_hint(family)?;
    // Lookup only: SCM/pipe verification and registered-client admission use OS identity.
    Ok(Some(family.to_owned()))
}

fn validate_service_family_hint(family: &str) -> Result<()> {
    let (name, publisher) = family
        .rsplit_once('_')
        .context("invalid service package family")?;
    ensure!(
        !name.is_empty()
            && name.len() <= 50
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-'))
            && publisher.len() == 13
            && publisher.bytes().all(|c| c.is_ascii_alphanumeric()),
        "invalid service package family"
    );
    Ok(())
}

#[cfg(test)]
#[path = "service_identity_tests.rs"]
mod tests;
