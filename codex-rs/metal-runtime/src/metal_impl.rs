//! macOS Metal implementation

use anyhow::{Context, Result};
use tracing::{debug, info, warn};
use std::process::Command;

use crate::{ChipInfo, ChipType, MetalGpuStats};

/// Check if Metal is available
pub fn check_metal_available() -> bool {
    // Metal is available on all macOS systems
    // Check if we're on macOS
    #[cfg(target_os = "macos")]
    {
        info!("Metal available on macOS");
        true
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Metal Runtime Implementation
pub struct MetalRuntimeImpl {
    chip_info: ChipInfo,
    mps_available: bool,
}

impl MetalRuntimeImpl {
    /// Create new runtime
    pub fn new() -> Result<Self> {
        info!("Initializing Metal Runtime");

        // Detect Apple Silicon chip
        let chip_info = detect_apple_silicon()?;
        info!(
            "Detected chip: {} ({} CPU cores, {} GPU cores)",
            chip_info.chip_type.label(),
            chip_info.core_count,
            chip_info.gpu_core_count
        );

        // Check MPS availability
        let mps_available = check_mps_available(&chip_info);
        if mps_available {
            info!("Metal Performance Shaders (MPS) available");
        } else {
            debug!("MPS not available, using standard Metal");
        }

        Ok(Self {
            chip_info,
            mps_available,
        })
    }

    /// Get chip information
    pub fn get_chip_info(&self) -> Result<ChipInfo> {
        Ok(self.chip_info.clone())
    }

    /// Check if MPS is available
    pub fn is_mps_available(&self) -> bool {
        self.mps_available
    }

    /// Get GPU statistics
    pub async fn get_gpu_stats(&self) -> Result<MetalGpuStats> {
        // TODO: Implement actual Metal GPU statistics query
        // For now, return placeholder values
        
        debug!("Querying Metal GPU statistics");

        Ok(MetalGpuStats {
            utilization: 0.0,
            memory_used: 0,
            memory_total: 0,
            temperature: None,
        })
    }
}

/// Detect Apple Silicon chip type
fn detect_apple_silicon() -> Result<ChipInfo> {
    // Use sysctl to detect chip type
    let output = Command::new("sysctl")
        .arg("-n")
        .arg("machdep.cpu.brand_string")
        .output()
        .context("Failed to run sysctl")?;

    let brand_string = String::from_utf8_lossy(&output.stdout).trim().to_string();
    debug!("CPU brand string: {brand_string}");

    // Parse chip type from brand string
    let chip_type = if brand_string.contains("Apple M3 Ultra") {
        ChipType::M3Ultra
    } else if brand_string.contains("Apple M3 Max") {
        ChipType::M3Max
    } else if brand_string.contains("Apple M3 Pro") {
        ChipType::M3Pro
    } else if brand_string.contains("Apple M3") {
        ChipType::M3
    } else if brand_string.contains("Apple M2 Ultra") {
        ChipType::M2Ultra
    } else if brand_string.contains("Apple M2 Max") {
        ChipType::M2Max
    } else if brand_string.contains("Apple M2 Pro") {
        ChipType::M2Pro
    } else if brand_string.contains("Apple M2") {
        ChipType::M2
    } else if brand_string.contains("Apple M1 Ultra") {
        ChipType::M1Ultra
    } else if brand_string.contains("Apple M1 Max") {
        ChipType::M1Max
    } else if brand_string.contains("Apple M1 Pro") {
        ChipType::M1Pro
    } else if brand_string.contains("Apple M1") {
        ChipType::M1
    } else {
        warn!("Unknown Apple Silicon chip: {brand_string}");
        ChipType::Unknown
    };

    // Get core counts
    let core_count = get_cpu_core_count()?;
    let gpu_core_count = get_gpu_core_count(&chip_type)?;
    let neural_engine_cores = get_neural_engine_cores(&chip_type);

    Ok(ChipInfo {
        chip_type,
        core_count,
        gpu_core_count,
        neural_engine_cores,
    })
}

/// Get CPU core count
fn get_cpu_core_count() -> Result<u32> {
    let output = Command::new("sysctl")
        .arg("-n")
        .arg("hw.ncpu")
        .output()
        .context("Failed to get CPU core count")?;

    let count_str = String::from_utf8_lossy(&output.stdout).trim();
    count_str
        .parse::<u32>()
        .context("Failed to parse CPU core count")
}

/// Get GPU core count based on chip type
fn get_gpu_core_count(chip_type: &ChipType) -> Result<u32> {
    // GPU core counts for Apple Silicon chips
    let count = match chip_type {
        ChipType::M1 => 8,
        ChipType::M1Pro => 14,
        ChipType::M1Max => 32,
        ChipType::M1Ultra => 64,
        ChipType::M2 => 10,
        ChipType::M2Pro => 19,
        ChipType::M2Max => 38,
        ChipType::M2Ultra => 76,
        ChipType::M3 => 10,
        ChipType::M3Pro => 18,
        ChipType::M3Max => 40,
        ChipType::M3Ultra => 80,
        ChipType::Unknown => {
            warn!("Unknown chip type, assuming 8 GPU cores");
            8
        }
    };

    Ok(count)
}

/// Get Neural Engine core count
fn get_neural_engine_cores(chip_type: &ChipType) -> Option<u32> {
    match chip_type {
        ChipType::M1 | ChipType::M1Pro | ChipType::M1Max | ChipType::M1Ultra => Some(16),
        ChipType::M2 | ChipType::M2Pro | ChipType::M2Max | ChipType::M2Ultra => Some(16),
        ChipType::M3 | ChipType::M3Pro | ChipType::M3Max | ChipType::M3Ultra => Some(16),
        ChipType::Unknown => None,
    }
}

/// Check if MPS (Metal Performance Shaders) is available
fn check_mps_available(chip_info: &ChipInfo) -> bool {
    // MPS is available on all Apple Silicon chips
    chip_info.chip_type != ChipType::Unknown
}











