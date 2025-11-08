//! CUDA implementation using cust (Rust-CUDA)
//! https://github.com/Rust-GPU/Rust-CUDA

use anyhow::Context as _;
use anyhow::Result;
use cust::prelude::*;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::CudaDeviceInfo;

/// Check if running in WSL2
pub fn is_wsl2() -> bool {
    // Check for WSL2 by looking at /proc/version
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        version.contains("microsoft") || version.contains("WSL2")
    } else {
        false
    }
}

/// Get WSL2 CUDA information
pub fn get_wsl2_cuda_info() -> Result<Wsl2CudaInfo> {
    if !is_wsl2() {
        anyhow::bail!("Not running in WSL2");
    }

    // Check for CUDA in WSL2
    let cuda_available = Path::new("/usr/local/cuda").exists()
        || std::env::var("CUDA_PATH").is_ok();

    if !cuda_available {
        warn!("WSL2 detected but CUDA not found. Install NVIDIA CUDA Toolkit for WSL2.");
        warn!("See: https://docs.nvidia.com/cuda/wsl-user-guide/index.html");
    }

    Ok(Wsl2CudaInfo {
        is_wsl2: true,
        cuda_available,
        performance_warning: true, // WSL2 CUDA has some performance overhead
    })
}

/// WSL2 CUDA information
#[derive(Debug, Clone)]
pub struct Wsl2CudaInfo {
    pub is_wsl2: bool,
    pub cuda_available: bool,
    pub performance_warning: bool,
}

/// CUDA Runtime implementation
pub struct CudaRuntimeImpl {
    _context: Context,
    device: Device,
    stream: Stream,
}

impl CudaRuntimeImpl {
    /// Create new CUDA runtime
    pub fn new(device_id: usize) -> Result<Self> {
        info!("Initializing CUDA with cust (Rust-CUDA)");

        // Check if running in WSL2
        if is_wsl2() {
            if let Ok(wsl_info) = get_wsl2_cuda_info() {
                if wsl_info.performance_warning {
                    warn!("WSL2 CUDA detected - performance may be reduced compared to native Linux");
                    warn!("Consider using native Linux for maximum CUDA performance");
                }
            }
        }

        // Initialize CUDA
        cust::init(CudaFlags::empty()).context("Failed to initialize CUDA")?;

        // Get device
        let device = Device::get_device(device_id as u32)
            .context(format!("Failed to get device {device_id}"))?;

        // Create context
        let _context =
            Context::create_and_push(ContextFlags::MAP_HOST | ContextFlags::SCHED_AUTO, device)
                .context("Failed to create CUDA context")?;

        // Create stream
        let stream =
            Stream::new(StreamFlags::NON_BLOCKING, None).context("Failed to create CUDA stream")?;

        info!("CUDA initialized successfully");

        Ok(Self {
            _context,
            device,
            stream,
        })
    }

    /// Get device information
    pub fn get_device_info(&self) -> Result<CudaDeviceInfo> {
        let name = self.device.name().context("Failed to get device name")?;

        let (major, minor) = self
            .device
            .compute_capability()
            .context("Failed to get compute capability")?;

        let total_memory = self
            .device
            .total_memory()
            .context("Failed to get total memory")? as usize;

        let multiprocessor_count = self
            .device
            .num_multiprocessors()
            .context("Failed to get SM count")? as i32;

        Ok(CudaDeviceInfo {
            name,
            compute_capability: (major as i32, minor as i32),
            total_memory,
            multiprocessor_count,
        })
    }

    /// Copy data to device
    pub fn copy_to_device<T: cust::memory::DeviceCopy>(
        &self,
        data: &[T],
    ) -> Result<DeviceBufferImpl<T>> {
        debug!("Copying {} elements to device", data.len());

        let mut device_buffer =
            DeviceBuffer::from_slice(data).context("Failed to allocate device memory")?;

        Ok(DeviceBufferImpl {
            buffer: device_buffer,
        })
    }

    /// Copy data from device
    pub fn copy_from_device<T: cust::memory::DeviceCopy + Clone>(
        &self,
        buffer: &DeviceBufferImpl<T>,
    ) -> Result<Vec<T>> {
        debug!("Copying {} elements from device", buffer.len());

        let mut host_data = vec![T::default(); buffer.len()];
        buffer
            .buffer
            .copy_to(&mut host_data)
            .context("Failed to copy from device")?;

        Ok(host_data)
    }

    /// Allocate device memory
    pub fn allocate<T: cust::memory::DeviceCopy>(
        &self,
        size: usize,
    ) -> Result<DeviceBufferImpl<T>> {
        debug!("Allocating {size} elements on device");

        let device_buffer =
            DeviceBuffer::zeroed(size).context("Failed to allocate device memory")?;

        Ok(DeviceBufferImpl {
            buffer: device_buffer,
        })
    }
}

/// Device buffer implementation
pub struct DeviceBufferImpl<T: cust::memory::DeviceCopy> {
    buffer: DeviceBuffer<T>,
}

impl<T: cust::memory::DeviceCopy> DeviceBufferImpl<T> {
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.len() == 0
    }
}

/// Check if CUDA is available
pub fn is_cuda_available() -> bool {
    cust::init(CudaFlags::empty()).is_ok()
}

/// Get number of CUDA devices
pub fn get_device_count() -> usize {
    match Device::num_devices() {
        Ok(count) => count as usize,
        Err(_) => 0,
    }
}
