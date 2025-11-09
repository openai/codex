//! Stub implementation when OpenXR is not available

use anyhow::Result;
use crate::{VrDeviceInfo, VrDeviceStats, VrDeviceType, VrRuntime};

impl VrRuntime {
    pub fn new() -> Result<Self> {
        anyhow::bail!("VR Runtime requires OpenXR feature")
    }

    pub fn get_device_info(&self) -> Result<VrDeviceInfo> {
        anyhow::bail!("VR Runtime requires OpenXR feature")
    }

    pub async fn get_device_stats(&self) -> Result<VrDeviceStats> {
        anyhow::bail!("VR Runtime requires OpenXR feature")
    }
}

pub fn check_vr_available() -> bool {
    false
}











