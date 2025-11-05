//! Windows 11 AI API Integration for Codex
//!
//! This crate provides integration with Windows 11's native AI APIs,
//! enabling OS-level optimizations for AI inference workloads.
//!
//! # Features
//!
//! - Windows.AI.MachineLearning: DirectML-based inference
//! - GPU statistics and optimization
//! - Integration with Codex kernel driver
//!
//! # Example
//!
//! ```no_run
//! use codex_windows_ai::WindowsAiRuntime;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = WindowsAiRuntime::new()?;
//!     let stats = runtime.get_gpu_stats().await?;
//!     println!("GPU Utilization: {}%", stats.utilization);
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(target_os = "windows")]
pub use windows_impl::*;

#[cfg(not(target_os = "windows"))]
mod stub;

#[cfg(not(target_os = "windows"))]
pub use stub::*;

/// GPU Statistics
#[derive(Debug, Clone)]
pub struct GpuStats {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f32,
}

/// Windows AI Runtime
pub struct WindowsAiRuntime {
    #[cfg(target_os = "windows")]
    inner: windows_impl::WindowsAiRuntimeImpl,
}

impl WindowsAiRuntime {
    /// Create a new Windows AI Runtime
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            let inner = windows_impl::WindowsAiRuntimeImpl::new()?;
            Ok(Self { inner })
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI is only available on Windows 11 25H2+")
        }
    }
    
    /// Get GPU statistics
    pub async fn get_gpu_stats(&self) -> Result<GpuStats> {
        #[cfg(target_os = "windows")]
        {
            self.inner.get_gpu_stats().await
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI is only available on Windows")
        }
    }
    
    /// Check if Windows AI is available on this system
    pub fn is_available() -> bool {
        #[cfg(target_os = "windows")]
        {
            windows_impl::check_windows_ai_available()
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }
}

/// Kernel Driver integration
pub mod kernel_driver {
    use super::*;
    
    /// Kernel Driver Bridge
    pub struct KernelBridge {
        #[cfg(target_os = "windows")]
        handle: windows::Win32::Foundation::HANDLE,
    }
    
    impl KernelBridge {
        /// Open connection to AI kernel driver
        pub fn open() -> Result<Self> {
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::Foundation::*;
                use windows::Win32::Storage::FileSystem::*;
                
                let device_path = windows::core::w!("\\\\.\\AI_Driver");
                
                unsafe {
                    let handle = CreateFileW(
                        device_path,
                        FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                        FILE_SHARE_NONE,
                        None,
                        OPEN_EXISTING,
                        FILE_ATTRIBUTE_NORMAL,
                        None,
                    )?;
                    
                    if handle.is_invalid() {
                        anyhow::bail!("Failed to open AI kernel driver");
                    }
                    
                    info!("AI Kernel Driver opened successfully");
                    Ok(Self { handle })
                }
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                anyhow::bail!("Kernel driver is only available on Windows")
            }
        }
        
        /// Get GPU stats from kernel driver
        pub fn get_gpu_stats(&self) -> Result<GpuStats> {
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::System::IO::DeviceIoControl;
                
                const IOCTL_AI_GET_GPU_STATUS: u32 = 0x222010;
                
                #[repr(C)]
                struct RawGpuStatus {
                    utilization: f32,
                    memory_used: u64,
                    memory_total: u64,
                    temperature: f32,
                }
                
                let mut output = RawGpuStatus {
                    utilization: 0.0,
                    memory_used: 0,
                    memory_total: 0,
                    temperature: 0.0,
                };
                
                let mut bytes_returned: u32 = 0;
                
                unsafe {
                    let success = DeviceIoControl(
                        self.handle,
                        IOCTL_AI_GET_GPU_STATUS,
                        None,
                        0,
                        Some(&mut output as *mut _ as *mut _),
                        std::mem::size_of::<RawGpuStatus>() as u32,
                        Some(&mut bytes_returned),
                        None,
                    );
                    
                    if success.as_bool() {
                        Ok(GpuStats {
                            utilization: output.utilization,
                            memory_used: output.memory_used,
                            memory_total: output.memory_total,
                            temperature: output.temperature,
                        })
                    } else {
                        anyhow::bail!("DeviceIoControl failed")
                    }
                }
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                anyhow::bail!("Kernel driver is only available on Windows")
            }
        }
    }
    
    impl Drop for KernelBridge {
        fn drop(&mut self) {
            #[cfg(target_os = "windows")]
            {
                unsafe {
                    windows::Win32::Foundation::CloseHandle(self.handle);
                }
            }
        }
    }
}

