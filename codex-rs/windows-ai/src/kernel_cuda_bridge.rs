//! Kernel Driver ↔ CUDA Runtime Bridge
//!
//! This module provides integration between the Windows AI kernel driver
//! and the CUDA Runtime, enabling optimized GPU memory management and
//! scheduling.

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info};

use crate::kernel_driver::KernelBridge;
use crate::kernel_driver_ffi::{MemoryPoolStatsC, PinnedMemory};

/// CUDA Runtime trait for abstraction
pub trait CudaRuntimeTrait: Send + Sync {
    /// Get device ID
    fn device_id(&self) -> usize;

    /// Allocate device memory (bytes)
    fn allocate_bytes(&self, size: usize) -> Result<()>;

    /// Get allocated memory size
    fn allocated_size(&self) -> usize;
}

/// Kernel-CUDA Bridge for optimized GPU operations
pub struct KernelCudaBridge {
    kernel_bridge: Arc<KernelBridge>,
    cuda_runtime: Option<Arc<dyn CudaRuntimeTrait>>,
    pinned_memory: Option<PinnedMemory>,
}

impl KernelCudaBridge {
    /// Create a new bridge
    pub fn new(kernel_bridge: Arc<KernelBridge>) -> Result<Self> {
        info!("Creating Kernel-CUDA bridge");

        Ok(Self {
            kernel_bridge,
            cuda_runtime: None,
            pinned_memory: None,
        })
    }

    /// Attach CUDA runtime to the bridge
    pub fn attach_cuda_runtime(&mut self, runtime: Arc<dyn CudaRuntimeTrait>) {
        info!("Attaching CUDA runtime (device {})", runtime.device_id());
        self.cuda_runtime = Some(runtime);
    }

    /// Allocate pinned memory from kernel driver for CUDA
    ///
    /// Pinned memory enables zero-copy transfers between CPU and GPU
    pub fn allocate_pinned_memory(&mut self, size: u64) -> Result<()> {
        debug!("Allocating {} bytes of pinned memory", size);

        // Get kernel driver handle
        let handle = self
            .kernel_bridge
            .driver_handle()
            .ok_or_else(|| anyhow::anyhow!("Kernel driver not available"))?;

        // Allocate pinned memory via kernel driver
        let pinned = PinnedMemory::new(handle, size)
            .context("Failed to allocate pinned memory from kernel driver")?;

        info!(
            "Allocated {} bytes of pinned memory at 0x{:X}",
            pinned.size(),
            pinned.address()
        );

        // If CUDA runtime is attached, register the pinned memory
        if let Some(cuda) = &self.cuda_runtime {
            debug!("Registering pinned memory with CUDA runtime (device {})", cuda.device_id());
            // TODO: Register pinned memory with CUDA runtime
            // This would enable zero-copy transfers
        }

        self.pinned_memory = Some(pinned);
        Ok(())
    }

    /// Get memory pool statistics from kernel driver
    pub fn get_memory_pool_stats(&self) -> Result<MemoryPoolStatsC> {
        self.kernel_bridge
            .get_memory_pool_stats()
            .context("Failed to get memory pool stats")
    }

    /// Optimize GPU scheduling for AI workloads
    pub fn optimize_scheduling(&self) -> Result<()> {
        info!("Optimizing GPU scheduling for AI workloads");

        // Enable GPU-aware scheduling (WDDM 3.2+)
        self.kernel_bridge
            .enable_gpu_aware_scheduling()
            .context("Failed to enable GPU-aware scheduling")?;

        // Get scheduler stats
        let stats = self
            .kernel_bridge
            .get_scheduler_stats()
            .context("Failed to get scheduler stats")?;

        info!(
            "Scheduler stats: {} AI tasks, {} boosted threads, {:.2}ms avg latency",
            stats.ai_task_count,
            stats.boosted_thread_count,
            stats.average_latency_ms
        );

        Ok(())
    }

    /// Get GPU statistics (combined from kernel driver and CUDA)
    pub async fn get_combined_gpu_stats(&self) -> Result<CombinedGpuStats> {
        // Get stats from kernel driver
        let kernel_stats = self
            .kernel_bridge
            .get_gpu_stats()
            .context("Failed to get kernel driver GPU stats")?;

        // Get memory pool stats
        let pool_stats = self.get_memory_pool_stats()?;

        // Get scheduler stats
        let scheduler_stats = self
            .kernel_bridge
            .get_scheduler_stats()
            .context("Failed to get scheduler stats")?;

        Ok(CombinedGpuStats {
            utilization: kernel_stats.utilization,
            memory_used: kernel_stats.memory_used,
            memory_total: kernel_stats.memory_total,
            temperature: kernel_stats.temperature,
            pinned_memory_total: pool_stats.total_size,
            pinned_memory_used: pool_stats.used_size,
            pinned_memory_free: pool_stats.free_size,
            ai_task_count: scheduler_stats.ai_task_count,
            boosted_thread_count: scheduler_stats.boosted_thread_count,
            average_latency_ms: scheduler_stats.average_latency_ms,
        })
    }
}

/// Combined GPU statistics from kernel driver and CUDA
#[derive(Debug, Clone)]
pub struct CombinedGpuStats {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f32,
    pub pinned_memory_total: u64,
    pub pinned_memory_used: u64,
    pub pinned_memory_free: u64,
    pub ai_task_count: u32,
    pub boosted_thread_count: u32,
    pub average_latency_ms: f32,
}

impl std::fmt::Display for CombinedGpuStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GPU: {:.1}% util, {}MB/{}MB mem, {}°C, {} AI tasks, {:.2}ms latency",
            self.utilization,
            self.memory_used / 1024 / 1024,
            self.memory_total / 1024 / 1024,
            self.temperature,
            self.ai_task_count,
            self.average_latency_ms
        )
    }
}





