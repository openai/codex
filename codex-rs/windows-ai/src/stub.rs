//! Stub implementation for non-Windows platforms

use anyhow::Result;

use crate::GpuStats;

pub struct WindowsAiRuntimeImpl;

impl WindowsAiRuntimeImpl {
    pub fn new() -> Result<Self> {
        anyhow::bail!("Windows AI is only available on Windows 11 25H2+")
    }
    
    pub async fn get_gpu_stats(&self) -> Result<GpuStats> {
        anyhow::bail!("Windows AI is only available on Windows")
    }
}

pub fn check_windows_ai_available() -> bool {
    false
}

