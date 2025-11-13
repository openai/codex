//! CUDA implementation using cust (Rust-CUDA)
//! https://github.com/Rust-GPU/Rust-CUDA

use anyhow::{Context, Result};
use cust::memory::DeviceCopy;
use cust::prelude::*;
use tracing::{debug, info};

use crate::CudaDeviceInfo;

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

        // NOTE: cust 0.3 API may not have compute_capability() method
        // Use device attributes or default values
        // TODO: Implement via cuDeviceGetAttribute when cust API is confirmed
        let compute_capability = (0, 0); // Placeholder until API is confirmed

        let total_memory = self
            .device
            .total_memory()
            .context("Failed to get total memory")? as usize;

        // NOTE: cust 0.3 API may not have num_multiprocessors() method
        // Use device attributes or default values
        // TODO: Implement via cuDeviceGetAttribute when cust API is confirmed
        let multiprocessor_count = 0; // Placeholder until API is confirmed

        Ok(CudaDeviceInfo {
            name,
            compute_capability,
            total_memory,
            multiprocessor_count,
        })
    }

    /// Copy data to device
    pub fn copy_to_device<T: DeviceCopy>(&self, data: &[T]) -> Result<DeviceBufferImpl<T>> {
        debug!("Copying {} elements to device", data.len());

        let device_buffer =
            DeviceBuffer::from_slice(data).context("Failed to allocate device memory")?;

        Ok(DeviceBufferImpl {
            buffer: device_buffer,
            len: data.len(),
        })
    }

    /// Copy data from device
    pub fn copy_from_device<T: DeviceCopy + Clone + Default>(
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
    /// 
    /// NOTE: DeviceBuffer::zeroed requires Zeroable trait in cust 0.3
    /// For now, allocate by creating default data and copying to device
    pub fn allocate<T: DeviceCopy + Default>(&self, size: usize) -> Result<DeviceBufferImpl<T>> {
        debug!("Allocating {size} elements on device");

        // Allocate by creating default data and copying to device
        // This avoids Zeroable trait requirement
        let default_data: Vec<T> = (0..size).map(|_| T::default()).collect();
        let device_buffer =
            DeviceBuffer::from_slice(&default_data).context("Failed to allocate device memory")?;

        Ok(DeviceBufferImpl {
            buffer: device_buffer,
            len: size,
        })
    }
}

/// Device buffer implementation
pub struct DeviceBufferImpl<T> {
    buffer: DeviceBuffer<T>,
    len: usize, // Store length separately as DeviceBuffer may not have len() method
}

impl<T: DeviceCopy> DeviceBufferImpl<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
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
