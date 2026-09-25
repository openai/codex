//! Copies executable fixtures without exposing writable descriptors to sibling test spawns.
//! Linux copies finish in a separate process before the fixture can be launched.

use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::Result;
#[cfg(target_os = "linux")]
use anyhow::ensure;

pub(super) fn copy_executable(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        // Sibling tests can spawn children while a copy holds the executable open
        // for writing. Keep that descriptor in a separate process and wait for it
        // to exit, so inherited descriptors cannot cause ETXTBSY during launch.
        let output = Command::new("/bin/cp")
            .arg("--")
            .arg(source)
            .arg(destination)
            .output()
            .context("failed to start fixture executable copy")?;
        ensure!(
            output.status.success(),
            "failed to copy fixture executable {} to {}: {}",
            source.display(),
            destination.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        // Match fs::copy even when the test process has a restrictive umask.
        std::fs::set_permissions(destination, std::fs::metadata(source)?.permissions())?;
    }
    #[cfg(not(target_os = "linux"))]
    std::fs::copy(source, destination)?;
    Ok(())
}
