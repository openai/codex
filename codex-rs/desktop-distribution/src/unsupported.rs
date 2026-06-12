use std::path::Path;
use std::path::PathBuf;

use crate::DesktopDistributionError;

#[derive(Debug, Clone)]
pub(crate) struct PlatformIdentity;

pub(crate) struct PlatformDistribution {
    pub app_root: PathBuf,
    pub resources_relative_path: PathBuf,
    pub identity: PlatformIdentity,
}

pub(crate) fn verify_hint(_hint: &Path) -> Result<PlatformDistribution, DesktopDistributionError> {
    Err(DesktopDistributionError::Unsupported)
}

pub(crate) fn discover() -> Result<PlatformDistribution, DesktopDistributionError> {
    Err(DesktopDistributionError::Unsupported)
}

pub(crate) fn current_process_distribution()
-> Result<Option<PlatformDistribution>, DesktopDistributionError> {
    Ok(None)
}

pub(crate) fn reverify(
    _identity: &PlatformIdentity,
    _app_root: &Path,
) -> Result<(), DesktopDistributionError> {
    Err(DesktopDistributionError::Unsupported)
}
