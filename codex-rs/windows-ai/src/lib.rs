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

use anyhow::Result;

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

    /// Get DirectML version (Windows 11 25H2+)
    #[cfg(target_os = "windows")]
    pub fn get_directml_version(&self) -> Result<windows_impl::DirectMlVersion> {
        Ok(self.inner.get_directml_version().clone())
    }

    /// Check if NPU (Copilot+ PC) is available
    #[cfg(target_os = "windows")]
    pub fn is_npu_available(&self) -> bool {
        self.inner.is_npu_available()
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
#[cfg(target_os = "windows")]
pub mod kernel_driver {
    pub use crate::windows_impl::kernel_driver::*;
}

#[cfg(not(target_os = "windows"))]
pub mod kernel_driver {
    use super::*;

    /// Kernel Driver Bridge (stub for non-Windows)
    pub struct KernelBridge;

    impl KernelBridge {
        /// Open connection to AI kernel driver
        pub fn open() -> Result<Self> {
            anyhow::bail!("Kernel driver is only available on Windows")
        }

        /// Get GPU stats from kernel driver
        pub fn get_gpu_stats(&self) -> Result<GpuStats> {
            anyhow::bail!("Kernel driver is only available on Windows")
        }
    }
}
