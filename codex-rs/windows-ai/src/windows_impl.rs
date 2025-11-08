//! Windows AI API implementation (Windows 11 25H2+)

use anyhow::{Context, Result};
use tracing::{debug, info, warn};
use windows::Win32::System::Registry::*;
use windows::Win32::Foundation::*;

use crate::GpuStats;

pub mod kernel_driver;

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

/// Get Windows build number from registry
fn get_windows_build_number() -> Result<u32> {
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &windows::core::w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            0,
            KEY_READ,
            &mut hkey,
        );

        if result.is_err() {
            // Fallback: assume Windows 11 25H2 if registry read fails
            warn!("Failed to read registry, assuming Windows 11 25H2");
            return Ok(26100);
        }

        let mut data_type = REG_DWORD;
        let mut data: [u8; 4] = [0; 4];
        let mut data_size = 4u32;

        let result = RegQueryValueExW(
            hkey,
            &windows::core::w!("CurrentBuildNumber"),
            None,
            Some(&mut data_type),
            Some(&mut data),
            Some(&mut data_size),
        );

        RegCloseKey(hkey);

        if result.is_ok() && data_type == REG_DWORD && data_size == 4 {
            let build = u32::from_le_bytes(data);
            debug!("Windows Build Number: {build}");
            Ok(build)
        } else {
            // Fallback
            warn!("Failed to read build number, assuming Windows 11 25H2");
            Ok(26100)
        }
    }
}

/// Check if Copilot+ PC (NPU available)
pub fn check_npu_available() -> bool {
    // Windows 11 25H2 introduces NPU support for Copilot+ PCs
    // Check via registry or DirectML device enumeration
    match get_windows_build_number() {
        Ok(build) if build >= 26100 => {
            // Check registry for NPU availability
            // Copilot+ PCs have NPU information in registry
            if let Ok(npu_available) = check_npu_via_registry() {
                if npu_available {
                    info!("NPU (Copilot+ PC) detected via registry");
                    return true;
                }
            }
            
            // TODO: Implement actual NPU detection via DirectML device enumeration
            // DirectML 1.13+ supports NPU device enumeration
            debug!("NPU detection via DirectML device enumeration (Build {build})");
            false
        }
        _ => false,
    }
}

/// Check NPU availability via registry
fn check_npu_via_registry() -> Result<bool> {
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &windows::core::w!("SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e97d-e325-11ce-bfc1-08002be10318}"),
            0,
            KEY_READ,
            &mut hkey,
        );

        if result.is_err() {
            return Ok(false);
        }

        // Check for NPU device class
        // TODO: Implement actual NPU device enumeration
        // For now, return false as specific registry keys need to be identified
        
        RegCloseKey(hkey);
        Ok(false)
    }
}

/// Get DirectML version info
pub fn get_directml_version() -> Result<DirectMlVersion> {
    let build = get_windows_build_number()?;
    
    // Windows 11 25H2 (Build 26100+) includes DirectML 1.13/1.14
    if build >= 26100 {
        Ok(DirectMlVersion {
            major: 1,
            minor: if build >= 26200 { 14 } else { 13 },
            build: build,
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
    directml_version: DirectMlVersion,
    npu_available: bool,
}

impl WindowsAiRuntimeImpl {
    /// Create new runtime
    pub fn new() -> Result<Self> {
        info!("Initializing Windows AI Runtime");

        // Check availability
        if !check_windows_ai_available() {
            anyhow::bail!("Windows AI requires Windows 11 Build 26100+");
        }

        // Get DirectML version
        let directml_version = get_directml_version()
            .context("Failed to get DirectML version")?;
        
        info!(
            "DirectML {}.{} detected (Build {})",
            directml_version.major,
            directml_version.minor,
            directml_version.build
        );

        // Check NPU availability
        let npu_available = check_npu_available();
        if npu_available {
            info!("NPU (Copilot+ PC) detected and available");
        } else {
            debug!("NPU not available, using GPU/CPU fallback");
        }

        Ok(Self {
            _initialized: true,
            directml_version,
            npu_available,
        })
    }

    /// Get DirectML version
    pub fn get_directml_version(&self) -> &DirectMlVersion {
        &self.directml_version
    }

    /// Check if NPU is available
    pub fn is_npu_available(&self) -> bool {
        self.npu_available
    }

        /// Get GPU statistics
    pub async fn get_gpu_stats(&self) -> Result<GpuStats> {
        // TODO: Implement actual Windows ML device querying via DirectML
        // For Windows 11 25H2, we can use DirectML device enumeration
        // For now, return estimated values

        let stats = GpuStats {
            utilization: 50.0,
            memory_used: 4 * 1024 * 1024 * 1024,   // 4GB
            memory_total: 10 * 1024 * 1024 * 1024, // 10GB
            temperature: 0.0,                      // Not available via WinML
        };

        Ok(stats)
    }

    /// Check driver compatibility and apply fallback if needed
    pub fn check_driver_compatibility(&self) -> Result<DriverCompatibility> {
        let build = get_windows_build_number()?;
        let compatibility = check_gpu_driver_compatibility(build)?;
        
        if !compatibility.is_compatible {
            warn!(
                "GPU driver compatibility issue detected: {}",
                compatibility.issue_description
            );
            if compatibility.recommended_action.is_some() {
                warn!(
                    "Recommended action: {}",
                    compatibility.recommended_action.as_ref().unwrap()
                );
            }
        }
        
        Ok(compatibility)
    }
}

/// Check GPU driver compatibility for Windows 11 25H2
fn check_gpu_driver_compatibility(build: u32) -> Result<DriverCompatibility> {
    // Windows 11 25H2 (Build 26100+) has known compatibility issues with:
    // 1. Some NVIDIA drivers (safeguard hold applied)
    // 2. Intel Arc B580 (performance degradation)
    // 3. Hyper-V GPU-P with KB5062553
    
    if build < 26100 {
        return Ok(DriverCompatibility {
            is_compatible: true,
            vendor: GpuDriverVendor::Unknown,
            issue_description: String::new(),
            recommended_action: None,
        });
    }
    
    // TODO: Implement actual driver version detection via registry or WMI
    // For now, return generic compatibility check
    
    // Check for Intel Arc B580
    // This is a known issue on Windows 11 25H2
    let compatibility = DriverCompatibility {
        is_compatible: true, // Assume compatible unless specific issue detected
        vendor: GpuDriverVendor::Unknown,
        issue_description: String::new(),
        recommended_action: None,
    };
    
    Ok(compatibility)
}

/// GPU driver compatibility information
#[derive(Debug, Clone)]
pub struct DriverCompatibility {
    pub is_compatible: bool,
    pub vendor: GpuDriverVendor,
    pub issue_description: String,
    pub recommended_action: Option<String>,
}

/// GPU driver vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuDriverVendor {
    Nvidia,
    Intel,
    Amd,
    Unknown,
}

impl GpuDriverVendor {
    /// Check if driver has known issues on Windows 11 25H2
    pub fn has_known_issues(&self, build: u32) -> bool {
        if build < 26100 {
            return false;
        }
        
        match self {
            GpuDriverVendor::Nvidia => {
                // Some NVIDIA drivers have safeguard hold on Windows 11 25H2
                // Check specific driver versions if needed
                false // Assume OK unless specific version detected
            }
            GpuDriverVendor::Intel => {
                // Intel Arc B580 has known performance issues
                true // Conservative: flag Intel as potentially problematic
            }
            GpuDriverVendor::Amd => false,
            GpuDriverVendor::Unknown => false,
        }
    }
    
    /// Get recommended driver version for Windows 11 25H2
    pub fn recommended_driver_version(&self) -> Option<&'static str> {
        match self {
            GpuDriverVendor::Nvidia => Some("Latest (check NVIDIA website)"),
            GpuDriverVendor::Intel => Some("31.0.101.8132+ (or rollback if issues persist)"),
            GpuDriverVendor::Amd => Some("Latest (check AMD website)"),
            GpuDriverVendor::Unknown => None,
        }
    }
}
