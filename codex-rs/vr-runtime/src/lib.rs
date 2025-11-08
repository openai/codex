//! VR/AR Runtime Abstraction Layer for Codex
//!
//! Provides unified interface for VR/AR devices via OpenXR
//! Supports: Quest, Vive, Index, and other OpenXR-compatible devices

use anyhow::Result;
use tracing::{debug, info};

#[cfg(feature = "openxr")]
mod openxr_impl;

#[cfg(feature = "openxr")]
pub use openxr_impl::*;

#[cfg(not(feature = "openxr"))]
mod stub;

#[cfg(not(feature = "openxr"))]
pub use stub::*;

/// VR device statistics
#[derive(Debug, Clone)]
pub struct VrDeviceStats {
    pub fps: f32,
    pub latency_ms: f32,
    pub frame_drops: u32,
    pub gpu_utilization: f32,
}

/// VR device information
#[derive(Debug, Clone)]
pub struct VrDeviceInfo {
    pub device_type: VrDeviceType,
    pub name: String,
    pub resolution: (u32, u32),
    pub refresh_rate: f32,
}

/// VR device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrDeviceType {
    Quest,
    Quest2,
    Quest3,
    Vive,
    VivePro,
    Index,
    Unknown,
}

impl VrDeviceType {
    pub fn label(&self) -> &'static str {
        match self {
            VrDeviceType::Quest => "Meta Quest",
            VrDeviceType::Quest2 => "Meta Quest 2",
            VrDeviceType::Quest3 => "Meta Quest 3",
            VrDeviceType::Vive => "HTC Vive",
            VrDeviceType::VivePro => "HTC Vive Pro",
            VrDeviceType::Index => "Valve Index",
            VrDeviceType::Unknown => "Unknown VR Device",
        }
    }
}

/// VR Runtime
pub struct VrRuntime {
    #[cfg(feature = "openxr")]
    inner: openxr_impl::VrRuntimeImpl,
}

impl VrRuntime {
    /// Create a new VR Runtime
    pub fn new() -> Result<Self> {
        #[cfg(feature = "openxr")]
        {
            let inner = openxr_impl::VrRuntimeImpl::new()?;
            Ok(Self { inner })
        }

        #[cfg(not(feature = "openxr"))]
        {
            anyhow::bail!("VR Runtime requires OpenXR feature (use --features openxr)")
        }
    }

    /// Get device information
    pub fn get_device_info(&self) -> Result<VrDeviceInfo> {
        #[cfg(feature = "openxr")]
        {
            self.inner.get_device_info()
        }

        #[cfg(not(feature = "openxr"))]
        {
            anyhow::bail!("VR Runtime requires OpenXR feature")
        }
    }

    /// Get device statistics
    pub async fn get_device_stats(&self) -> Result<VrDeviceStats> {
        #[cfg(feature = "openxr")]
        {
            self.inner.get_device_stats().await
        }

        #[cfg(not(feature = "openxr"))]
        {
            anyhow::bail!("VR Runtime requires OpenXR feature")
        }
    }

    /// Check if VR is available
    pub fn is_available() -> bool {
        #[cfg(feature = "openxr")]
        {
            openxr_impl::check_vr_available()
        }

        #[cfg(not(feature = "openxr"))]
        {
            false
        }
    }
}

pub mod virtual_desktop;

#[cfg(target_os = "windows")]
pub mod openxr_registry;

