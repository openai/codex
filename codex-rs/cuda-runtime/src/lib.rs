//! CUDA Runtime Integration for Codex
//!
//! Based on Rust-CUDA ecosystem (https://github.com/Rust-GPU/Rust-CUDA)
//! Provides GPU acceleration for:
//! - AI inference (MCP tool)
//! - Git analysis parallelization (100-1000x speedup)
//! - 3D/4D visualization
//!
//! # Example
//!
//! ```no_run
//! use codex_cuda_runtime::CudaRuntime;
//!
//! let cuda = CudaRuntime::new(0)?;
//! let device_info = cuda.get_device_info()?;
//! println!("GPU: {}", device_info.name);
//! ```

use anyhow::Result;

#[cfg(feature = "cuda")]
pub mod cuda_impl;

#[cfg(not(feature = "cuda"))]
pub mod stub;

#[cfg(feature = "cuda")]
pub use cuda_impl::*;

#[cfg(not(feature = "cuda"))]
pub use stub::*;

/// CUDA device information
#[derive(Debug, Clone)]
pub struct CudaDeviceInfo {
    pub name: String,
    pub compute_capability: (i32, i32),
    pub total_memory: usize,
    pub multiprocessor_count: i32,
}

impl std::fmt::Display for CudaDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (SM {}.{}, {}MB, {}SMs)",
            self.name,
            self.compute_capability.0,
            self.compute_capability.1,
            self.total_memory / 1024 / 1024,
            self.multiprocessor_count
        )
    }
}

/// CUDA Runtime wrapper
pub struct CudaRuntime {
    #[cfg(feature = "cuda")]
    inner: cuda_impl::CudaRuntimeImpl,
}

impl CudaRuntime {
    /// Create new CUDA runtime on specified device
    pub fn new(device_id: usize) -> Result<Self> {
        #[cfg(feature = "cuda")]
        {
            let inner = cuda_impl::CudaRuntimeImpl::new(device_id)?;
            Ok(Self { inner })
        }

        #[cfg(not(feature = "cuda"))]
        {
            let _ = device_id;
            anyhow::bail!("CUDA support not compiled (use --features cuda)")
        }
    }

    /// Get device information
    pub fn get_device_info(&self) -> Result<CudaDeviceInfo> {
        #[cfg(feature = "cuda")]
        {
            self.inner.get_device_info()
        }

        #[cfg(not(feature = "cuda"))]
        {
            anyhow::bail!("CUDA not available")
        }
    }

    /// Check if CUDA is available
    pub fn is_available() -> bool {
        #[cfg(feature = "cuda")]
        {
            cuda_impl::is_cuda_available()
        }

        #[cfg(not(feature = "cuda"))]
        {
            false
        }
    }

    /// Get number of CUDA devices
    pub fn device_count() -> usize {
        #[cfg(feature = "cuda")]
        {
            cuda_impl::get_device_count()
        }

        #[cfg(not(feature = "cuda"))]
        {
            0
        }
    }

    /// Copy data to device
    /// 
    /// NOTE: Requires `DeviceCopy` trait from cust crate
    #[cfg(feature = "cuda")]
    pub fn copy_to_device<T: cust::memory::DeviceCopy>(&self, _data: &[T]) -> Result<DeviceBuffer<T>> {
        let buffer_impl = self.inner.copy_to_device(_data)?;
        Ok(DeviceBuffer {
            inner: buffer_impl,
        })
    }

    #[cfg(not(feature = "cuda"))]
    pub fn copy_to_device<T>(&self, _data: &[T]) -> Result<DeviceBuffer<T>> {
        anyhow::bail!("CUDA not available")
    }

    /// Copy data from device
    /// 
    /// NOTE: Requires `DeviceCopy`, `Clone`, and `Default` traits
    #[cfg(feature = "cuda")]
    pub fn copy_from_device<T: cust::memory::DeviceCopy + Clone + Default>(&self, _buffer: &DeviceBuffer<T>) -> Result<Vec<T>> {
        self.inner.copy_from_device(&_buffer.inner)
    }

    #[cfg(not(feature = "cuda"))]
    pub fn copy_from_device<T>(&self, _buffer: &DeviceBuffer<T>) -> Result<Vec<T>> {
        anyhow::bail!("CUDA not available")
    }

    /// Allocate device memory
    /// 
    /// NOTE: Requires `DeviceCopy` and `Default` traits
    #[cfg(feature = "cuda")]
    pub fn allocate<T: cust::memory::DeviceCopy + Default>(&self, _size: usize) -> Result<DeviceBuffer<T>> {
        let buffer_impl = self.inner.allocate(_size)?;
        Ok(DeviceBuffer {
            inner: buffer_impl,
        })
    }

    #[cfg(not(feature = "cuda"))]
    pub fn allocate<T>(&self, _size: usize) -> Result<DeviceBuffer<T>> {
        anyhow::bail!("CUDA not available")
    }
}

/// Device buffer (GPU memory)
/// 
/// NOTE: T must implement DeviceCopy trait (required by cust::memory::DeviceBuffer)
pub struct DeviceBuffer<T: cust::memory::DeviceCopy> {
    #[cfg(feature = "cuda")]
    inner: cuda_impl::DeviceBufferImpl<T>,
    #[cfg(not(feature = "cuda"))]
    _phantom: std::marker::PhantomData<T>,
}

impl<T: cust::memory::DeviceCopy> DeviceBuffer<T> {
    /// Get size in elements
    #[cfg(feature = "cuda")]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[cfg(not(feature = "cuda"))]
    pub fn len(&self) -> usize {
        0
    }

    /// Check if empty
    #[cfg(feature = "cuda")]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(not(feature = "cuda"))]
    pub fn is_empty(&self) -> bool {
        true
    }
}
