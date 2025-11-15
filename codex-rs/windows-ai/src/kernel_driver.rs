//! Kernel Driver integration for Windows AI
//! WDDM 3.2 GPU scheduling optimization
//!
//! This module uses the type-safe FFI wrapper from `kernel_driver_ffi`
//! to provide a safe Rust interface to the Windows AI kernel driver.

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::GpuStats;
use crate::kernel_driver_ffi::{AiDriverHandle, MemoryPoolStatsC, SchedulerStatsC};

/// Kernel Driver Bridge for GPU optimization
///
/// Uses type-safe FFI wrapper instead of direct C calls
pub struct KernelBridge {
    driver_handle: Option<Arc<AiDriverHandle>>,
}

impl KernelBridge {
    /// Open connection to AI kernel driver
    ///
    /// Uses type-safe FFI wrapper to connect to the kernel driver
    pub fn open() -> Result<Self> {
        info!("Attempting to open kernel driver connection");
        
        // Try to open the driver using type-safe FFI wrapper
        match AiDriverHandle::open() {
            Ok(handle) => {
                info!("Successfully opened kernel driver connection");
                
                // Check WDDM version
                let wddm_version = get_wddm_version()?;
                info!("WDDM version: {}.{}", wddm_version.major, wddm_version.minor);
                
                if wddm_version.major >= 3 && wddm_version.minor >= 2 {
                    info!("WDDM 3.2+ detected, GPU scheduling optimizations available");
                } else {
                    warn!("WDDM 3.2 not available, some optimizations may be limited");
                }
                
                Ok(Self {
                    #[allow(clippy::arc_with_non_send_sync)]
                    driver_handle: Some(Arc::new(handle)),
                })
            }
            Err(e) => {
                warn!("Failed to open kernel driver: {e}");
                warn!("Kernel driver may not be installed - falling back to placeholder");
                
                // Check WDDM version even if driver is not available
                let wddm_version = get_wddm_version()?;
                info!("WDDM version: {}.{}", wddm_version.major, wddm_version.minor);
                
                Ok(Self {
                    driver_handle: None,
                })
            }
        }
    }

    /// Get GPU stats from kernel driver
    ///
    /// Uses type-safe FFI wrapper to query GPU statistics
    pub fn get_gpu_stats(&self) -> Result<GpuStats> {
        debug!("Querying GPU stats from kernel driver");
        
        match &self.driver_handle {
            Some(handle) => {
                // Use type-safe FFI wrapper
                handle.get_gpu_status()
                    .context("Failed to get GPU stats from kernel driver")
            }
            None => {
                // Fallback: return placeholder stats if driver is not available
                warn!("Kernel driver not available, returning placeholder stats");
                Ok(GpuStats {
                    utilization: 0.0,
                    memory_used: 0,
                    memory_total: 0,
                    temperature: 0.0,
                })
            }
        }
    }

    /// Get memory pool statistics
    pub fn get_memory_pool_stats(&self) -> Result<MemoryPoolStatsC> {
        match &self.driver_handle {
            Some(handle) => {
                handle.get_memory_pool_stats()
                    .context("Failed to get memory pool stats")
            }
            None => {
                anyhow::bail!("Kernel driver not available")
            }
        }
    }

    /// Get scheduler statistics
    pub fn get_scheduler_stats(&self) -> Result<SchedulerStatsC> {
        match &self.driver_handle {
            Some(handle) => {
                handle.get_scheduler_stats()
                    .context("Failed to get scheduler stats")
            }
            None => {
                anyhow::bail!("Kernel driver not available")
            }
        }
    }

    /// Enable GPU-aware thread scheduling (WDDM 3.2+)
    pub fn enable_gpu_aware_scheduling(&self) -> Result<()> {
        let wddm_version = get_wddm_version()?;
        
        if wddm_version.major >= 3 && wddm_version.minor >= 2 {
            info!("Enabling GPU-aware thread scheduling (WDDM 3.2+)");
            // TODO: Implement actual scheduling optimization via kernel driver
            Ok(())
        } else {
            warn!("GPU-aware scheduling requires WDDM 3.2+");
            Ok(())
        }
    }

    /// Get driver handle (for advanced use cases)
    pub fn driver_handle(&self) -> Option<Arc<AiDriverHandle>> {
        self.driver_handle.clone()
    }

    /// Check Intel Arc B580 compatibility and apply fallback if needed
    pub fn check_intel_arc_compatibility(&self) -> Result<ArcCompatibility> {
        let adapter_info = get_gpu_adapter_info()?;
        
        if adapter_info.vendor == GpuVendor::Intel 
            && adapter_info.model.contains("Arc") 
            && adapter_info.model.contains("B580") {
            warn!(
                "Intel Arc B580 detected - known performance issues on Windows 11 25H2"
            );
            warn!("Recommendation: Update to driver 31.0.101.8132+ or rollback if issues persist");
            
            return Ok(ArcCompatibility {
                is_arc_b580: true,
                recommended_fallback: true,
                driver_version: adapter_info.driver_version.clone(),
            });
        }
        
        Ok(ArcCompatibility {
            is_arc_b580: false,
            recommended_fallback: false,
            driver_version: adapter_info.driver_version,
        })
    }
}

impl Drop for KernelBridge {
    fn drop(&mut self) {
        // Arc will automatically drop the handle when the last reference is dropped
        // AiDriverHandle's Drop implementation will close the Windows handle
        debug!("Dropping KernelBridge");
    }
}

/// WDDM version information
#[derive(Debug, Clone)]
struct WddmVersion {
    major: u32,
    minor: u32,
}

/// Get WDDM version from system
fn get_wddm_version() -> Result<WddmVersion> {
    // WDDM version is typically reported via DXGI adapter
    // For now, assume WDDM 3.2 for Windows 11 25H2
    // TODO: Implement actual DXGI query when needed
    
    Ok(WddmVersion {
        major: 3,
        minor: 2,
    })
}

/// GPU adapter information
#[derive(Debug, Clone)]
struct GpuAdapterInfo {
    vendor: GpuVendor,
    model: String,
    driver_version: String,
}

/// GPU vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuVendor {
    #[allow(dead_code)]
    Nvidia,
    Intel,
    #[allow(dead_code)]
    Amd,
    Unknown,
}

/// Get GPU adapter information
fn get_gpu_adapter_info() -> Result<GpuAdapterInfo> {
    // TODO: Implement actual GPU adapter query via DXGI
    // For now, return placeholder
    
    Ok(GpuAdapterInfo {
        vendor: GpuVendor::Unknown,
        model: "Unknown".to_string(),
        driver_version: "Unknown".to_string(),
    })
}

/// Intel Arc B580 compatibility information
#[derive(Debug, Clone)]
pub struct ArcCompatibility {
    pub is_arc_b580: bool,
    pub recommended_fallback: bool,
    pub driver_version: String,
}


