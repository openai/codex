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
mod kernel_driver;
#[cfg(target_os = "windows")]
mod kernel_driver_ffi;

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
/// 
/// Re-export kernel_driver module (defined at line 32)
#[cfg(target_os = "windows")]
pub use kernel_driver::*;
