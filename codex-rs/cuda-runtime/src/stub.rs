//! Stub implementation for non-CUDA builds

use anyhow::Result;

/// Stub CUDA Runtime
pub struct CudaRuntime;

impl CudaRuntime {
    pub fn new(_device_id: usize) -> Result<Self> {
        anyhow::bail!("CUDA support not compiled (use --features cuda)")
    }

    pub fn is_available() -> bool {
        false
    }

    pub fn device_count() -> usize {
        0
    }
}

/// Stub device info
#[derive(Debug, Clone)]
pub struct CudaDeviceInfo {
    pub name: String,
    pub compute_capability: (i32, i32),
    pub total_memory: usize,
    pub multiprocessor_count: i32,
}
