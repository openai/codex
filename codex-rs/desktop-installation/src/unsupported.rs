use crate::DesktopInstallationError;
use crate::VerifiedDesktopInstallation;

pub(crate) fn discover() -> Result<Option<VerifiedDesktopInstallation>, DesktopInstallationError> {
    Ok(None)
}
