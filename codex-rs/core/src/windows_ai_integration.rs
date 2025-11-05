//! Windows 11 AI API integration layer for Codex Core
//!
//! This module provides integration between Codex and Windows 11's native AI APIs,
//! enabling OS-level optimizations and kernel driver acceleration.

use anyhow::Result;

#[cfg(target_os = "windows")]
use anyhow::Context;

#[cfg(all(target_os = "windows", feature = "windows-ai"))]
use tracing::{debug, info};

#[cfg(all(target_os = "windows", feature = "windows-ai"))]
use codex_windows_ai::{GpuStats, WindowsAiRuntime, kernel_driver::KernelBridge};

/// Windows AI execution options
#[derive(Debug, Clone)]
pub struct WindowsAiOptions {
    /// Use Windows AI API for optimization
    pub enabled: bool,
    /// Use kernel driver for additional acceleration
    pub kernel_accelerated: bool,
    /// Use GPU device
    pub use_gpu: bool,
}

impl Default for WindowsAiOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            kernel_accelerated: false,
            use_gpu: true,
        }
    }
}

/// Execute prompt with Windows AI optimization
#[cfg(all(target_os = "windows", feature = "windows-ai"))]
pub async fn execute_with_windows_ai(
    prompt: &str,
    options: &WindowsAiOptions,
) -> Result<String> {
    if !options.enabled {
        anyhow::bail!("Windows AI not enabled");
    }
    
    info!("Executing with Windows AI (kernel_accelerated: {})", options.kernel_accelerated);
    
    // Initialize Windows AI runtime
    let runtime = WindowsAiRuntime::new()
        .context("Failed to initialize Windows AI runtime")?;
    
    // Get GPU stats
    let stats = runtime.get_gpu_stats().await
        .context("Failed to get GPU stats")?;
    
    debug!("GPU Stats: utilization={:.1}%, memory={}/{}MB", 
        stats.utilization,
        stats.memory_used / 1024 / 1024,
        stats.memory_total / 1024 / 1024
    );
    
    // If kernel acceleration is enabled, use kernel driver
    if options.kernel_accelerated {
        execute_with_kernel_driver(prompt, &runtime).await
    } else {
        // Execute via Windows AI API only
        Ok(format!("Windows AI execution: {prompt} (placeholder)"))
    }
}

/// Execute with kernel driver acceleration
#[cfg(all(target_os = "windows", feature = "windows-ai"))]
async fn execute_with_kernel_driver(
    prompt: &str,
    _runtime: &WindowsAiRuntime,
) -> Result<String> {
    info!("Attempting kernel driver acceleration");
    
    // Open kernel driver
    let kernel = KernelBridge::open()
        .context("Failed to open AI kernel driver - is it installed?")?;
    
    // Get GPU stats from kernel
    let kernel_stats = kernel.get_gpu_stats()
        .context("Failed to get kernel GPU stats")?;
    
    info!("Kernel GPU Stats: utilization={:.1}%, memory={}/{}MB",
        kernel_stats.utilization,
        kernel_stats.memory_used / 1024 / 1024,
        kernel_stats.memory_total / 1024 / 1024
    );
    
    // TODO: Register Windows AI runtime with kernel driver for optimization
    // This would enable:
    // - Pinned memory allocation for faster GPU transfers
    // - GPU-aware thread scheduling
    // - Priority boosting for AI tasks
    
    Ok(format!("Kernel-accelerated execution: {prompt} (placeholder)"))
}

/// Stub for non-Windows or non-feature platforms
#[cfg(not(all(target_os = "windows", feature = "windows-ai")))]
pub async fn execute_with_windows_ai(
    _prompt: &str,
    _options: &WindowsAiOptions,
) -> Result<String> {
    anyhow::bail!("Windows AI is only available on Windows 11 25H2+ with windows-ai feature")
}

/// Placeholder GpuStats for when feature is disabled
#[cfg(not(all(target_os = "windows", feature = "windows-ai")))]
#[derive(Debug, Clone)]
pub struct GpuStats {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f32,
}

/// Get GPU statistics (Windows-only with feature)
#[cfg(all(target_os = "windows", feature = "windows-ai"))]
pub async fn get_gpu_statistics() -> Result<GpuStats> {
    let runtime = WindowsAiRuntime::new()?;
    runtime.get_gpu_stats().await
}

/// Get GPU statistics stub
#[cfg(not(all(target_os = "windows", feature = "windows-ai")))]
pub async fn get_gpu_statistics() -> Result<GpuStats> {
    anyhow::bail!("Windows AI is only available on Windows with windows-ai feature")
}

/// Check if Windows AI is available on this system
pub fn is_windows_ai_available() -> bool {
    #[cfg(all(target_os = "windows", feature = "windows-ai"))]
    {
        WindowsAiRuntime::is_available()
    }
    
    #[cfg(not(all(target_os = "windows", feature = "windows-ai")))]
    {
        false
    }
}

