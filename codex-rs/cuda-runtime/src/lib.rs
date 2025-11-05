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
    pub fn copy_to_device<T: Clone>(&self, _data: &[T]) -> Result<DeviceBuffer<T>> {
        #[cfg(feature = "cuda")]
        {
            self.inner.copy_to_device(_data)
        }
        
        #[cfg(not(feature = "cuda"))]
        {
            anyhow::bail!("CUDA not available")
        }
    }
    
    /// Copy data from device
    pub fn copy_from_device<T: Clone>(&self, _buffer: &DeviceBuffer<T>) -> Result<Vec<T>> {
        #[cfg(feature = "cuda")]
        {
            self.inner.copy_from_device(_buffer)
        }
        
        #[cfg(not(feature = "cuda"))]
        {
            anyhow::bail!("CUDA not available")
        }
    }
    
    /// Allocate device memory
    pub fn allocate<T: Clone>(&self, _size: usize) -> Result<DeviceBuffer<T>> {
        #[cfg(feature = "cuda")]
        {
            self.inner.allocate(_size)
        }
        
        #[cfg(not(feature = "cuda"))]
        {
            anyhow::bail!("CUDA not available")
        }
    }
}

/// Device buffer (GPU memory)
pub struct DeviceBuffer<T> {
    #[cfg(feature = "cuda")]
    inner: cuda_impl::DeviceBufferImpl<T>,
    #[cfg(not(feature = "cuda"))]
    _phantom: std::marker::PhantomData<T>,
}

impl<T> DeviceBuffer<T> {
    /// Get size in elements
    pub fn len(&self) -> usize {
        #[cfg(feature = "cuda")]
        {
            self.inner.len()
        }
        
        #[cfg(not(feature = "cuda"))]
        {
            0
        }
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
