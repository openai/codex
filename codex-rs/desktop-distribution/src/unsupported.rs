use crate::DesktopDistributionError;
use crate::InstalledDesktop;

pub(crate) fn discover() -> Result<Option<InstalledDesktop>, DesktopDistributionError> {
    Ok(None)
}
