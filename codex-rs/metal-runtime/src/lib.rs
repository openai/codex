//! macOS Metal Runtime Integration for Codex
//!
//! This crate provides integration with macOS Metal API for GPU acceleration,
//! supporting Metal 3 and Metal Performance Shaders (MPS) on Apple Silicon.

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

#[cfg(target_os = "macos")]
mod metal_impl;

#[cfg(target_os = "macos")]
pub use metal_impl::*;

#[cfg(not(target_os = "macos"))]
mod stub;

#[cfg(not(target_os = "macos"))]
pub use stub::*;

/// GPU Statistics for Metal
#[derive(Debug, Clone)]
pub struct MetalGpuStats {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: Option<f32>,
}

/// Metal Runtime
pub struct MetalRuntime {
    #[cfg(target_os = "macos")]
    inner: metal_impl::MetalRuntimeImpl,
}

impl MetalRuntime {
    /// Create a new Metal Runtime
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let inner = metal_impl::MetalRuntimeImpl::new()?;
            Ok(Self { inner })
        }

        #[cfg(not(target_os = "macos"))]
        {
            anyhow::bail!("Metal is only available on macOS")
        }
    }

    /// Get GPU statistics
    pub async fn get_gpu_stats(&self) -> Result<MetalGpuStats> {
        #[cfg(target_os = "macos")]
        {
            self.inner.get_gpu_stats().await
        }

        #[cfg(not(target_os = "macos"))]
        {
            anyhow::bail!("Metal is only available on macOS")
        }
    }

    /// Check if Metal is available on this system
    pub fn is_available() -> bool {
        #[cfg(target_os = "macos")]
        {
            metal_impl::check_metal_available()
        }

        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// Get Apple Silicon chip information
    #[cfg(target_os = "macos")]
    pub fn get_chip_info(&self) -> Result<ChipInfo> {
        self.inner.get_chip_info()
    }

    /// Check if MPS (Metal Performance Shaders) is available
    #[cfg(target_os = "macos")]
    pub fn is_mps_available(&self) -> bool {
        self.inner.is_mps_available()
    }
}

/// Apple Silicon chip information
#[derive(Debug, Clone)]
pub struct ChipInfo {
    pub chip_type: ChipType,
    pub core_count: u32,
    pub gpu_core_count: u32,
    pub neural_engine_cores: Option<u32>,
}

/// Apple Silicon chip type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipType {
    M1,
    M1Pro,
    M1Max,
    M1Ultra,
    M2,
    M2Pro,
    M2Max,
    M2Ultra,
    M3,
    M3Pro,
    M3Max,
    M3Ultra,
    Unknown,
}

impl ChipType {
    pub fn label(&self) -> &'static str {
        match self {
            ChipType::M1 => "Apple M1",
            ChipType::M1Pro => "Apple M1 Pro",
            ChipType::M1Max => "Apple M1 Max",
            ChipType::M1Ultra => "Apple M1 Ultra",
            ChipType::M2 => "Apple M2",
            ChipType::M2Pro => "Apple M2 Pro",
            ChipType::M2Max => "Apple M2 Max",
            ChipType::M2Ultra => "Apple M2 Ultra",
            ChipType::M3 => "Apple M3",
            ChipType::M3Pro => "Apple M3 Pro",
            ChipType::M3Max => "Apple M3 Max",
            ChipType::M3Ultra => "Apple M3 Ultra",
            ChipType::Unknown => "Unknown Apple Silicon",
        }
    }
}











