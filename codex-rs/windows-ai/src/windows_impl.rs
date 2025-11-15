//! Windows AI API implementation (Windows 11 25H2+)

use anyhow::Result;
use tracing::info;

use crate::GpuStats;

// kernel_driver and kernel_driver_ffi are declared in lib.rs at crate root

/// Check if Windows AI is available
pub fn check_windows_ai_available() -> bool {
    // Check Windows version (Build 26100+)
    match get_windows_build_number() {
        Ok(build) if build >= 26100 => {
            info!("Windows AI available (Build {build})");
            true
        }
        Ok(_build) => false,
        Err(_e) => false,
    }
}

/// Get Windows build number
fn get_windows_build_number() -> Result<u32> {
    // SystemInformation module not available in current windows crate version
    // Use registry or assume latest Windows 11 build

    // TODO: Implement via registry read when needed
    // For now, return latest Windows 11 build number
    Ok(26100) // Windows 11 25H2
}

/// Check NPU availability via registry
///
/// NOTE: Windows Registry API not available in current windows crate version (0.58)
/// This function is disabled until windows crate is updated with Registry API support
#[allow(dead_code)]
fn check_npu_via_registry() -> Result<bool> {
    // TODO: Implement via registry read when windows crate Registry API is available
    // For now, return false as placeholder
        Ok(false)
}

/// Get DirectML version info
pub fn get_directml_version() -> Result<DirectMlVersion> {
    let build = get_windows_build_number()?;

    // Windows 11 25H2 (Build 26100+) includes DirectML 1.13/1.14
    if build >= 26100 {
        Ok(DirectMlVersion {
            major: 1,
            minor: if build >= 26200 { 14 } else { 13 },
            build,
        })
    } else {
        anyhow::bail!("DirectML 1.13+ requires Windows 11 25H2 (Build 26100+)")
    }
}

/// DirectML version information
#[derive(Debug, Clone)]
pub struct DirectMlVersion {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
}

/// Windows AI Runtime Implementation
pub struct WindowsAiRuntimeImpl {
    _initialized: bool,
}

impl WindowsAiRuntimeImpl {
    /// Create new runtime
    pub fn new() -> Result<Self> {
        info!("Initializing Windows AI Runtime");

        // Check availability
        if !check_windows_ai_available() {
            anyhow::bail!("Windows AI requires Windows 11 Build 26100+");
        }

        Ok(Self { _initialized: true })
    }

    /// Get GPU statistics
    pub async fn get_gpu_stats(&self) -> Result<GpuStats> {
        // TODO: Implement actual Windows ML device querying
        // For now, return estimated values

        let stats = GpuStats {
            utilization: 50.0,
            memory_used: 4 * 1024 * 1024 * 1024,   // 4GB
            memory_total: 10 * 1024 * 1024 * 1024, // 10GB
            temperature: 0.0,                      // Not available via WinML
        };

        Ok(stats)
    }
}
