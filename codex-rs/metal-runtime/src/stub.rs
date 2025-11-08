//! Stub implementation for non-macOS platforms

use anyhow::Result;
use crate::{ChipInfo, ChipType, MetalGpuStats};

/// Metal Runtime stub
pub struct MetalRuntimeImpl;

impl MetalRuntimeImpl {
    pub fn new() -> Result<Self> {
        anyhow::bail!("Metal is only available on macOS")
    }

    pub fn get_chip_info(&self) -> Result<ChipInfo> {
        anyhow::bail!("Metal is only available on macOS")
    }

    pub fn is_mps_available(&self) -> bool {
        false
    }

    pub async fn get_gpu_stats(&self) -> Result<MetalGpuStats> {
        anyhow::bail!("Metal is only available on macOS")
    }
}

pub fn check_metal_available() -> bool {
    false
}











