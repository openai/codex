//! Type-safe FFI wrapper for Windows AI Driver
//!
//! This module provides a safe Rust interface to the Windows AI kernel driver,
//! replacing direct C FFI calls with type-safe wrappers that use Rust's ownership
//! system and error handling.

use anyhow::{Context, Result};
use std::ffi::c_void;
use std::mem;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

use crate::GpuStats;

/// Device name for the AI driver
const AI_DRIVER_DEVICE_NAME: &str = "\\\\.\\CodexAiDriver";

/// IOCTL codes (must match kernel driver definitions)
///
/// These are defined using CTL_CODE macro in the kernel driver:
/// CTL_CODE(FILE_DEVICE_UNKNOWN, function, METHOD_BUFFERED, FILE_ANY_ACCESS)
mod ioctl_codes {
    use super::*;

    /// Get GPU status
    pub const GET_GPU_STATUS: u32 = 0x222010; // CTL_CODE(..., 0x803, ...)

    /// Get memory pool statistics
    pub const GET_MEMORY_POOL: u32 = 0x222014; // CTL_CODE(..., 0x804, ...)

    /// Get scheduler statistics
    pub const GET_SCHEDULER_STATS: u32 = 0x222018; // CTL_CODE(..., 0x805, ...)

    /// Allocate pinned memory
    pub const ALLOC_PINNED: u32 = 0x22201C; // CTL_CODE(..., 0x806, ...)

    /// Free pinned memory
    pub const FREE_PINNED: u32 = 0x222020; // CTL_CODE(..., 0x807, ...)

    /// Register Windows AI runtime
    pub const REGISTER_WINAI: u32 = 0x222024; // CTL_CODE(..., 0x808, ...)

    /// Get optimized execution path
    pub const GET_OPTIMIZED_PATH: u32 = 0x222028; // CTL_CODE(..., 0x809, ...)
}

/// C-compatible GPU status structure (must match kernel driver)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuStatusC {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f32,
    pub power_usage: f32,
    pub clock_speed: u32,
}

/// C-compatible memory pool statistics (must match kernel driver)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryPoolStatsC {
    pub total_size: u64,
    pub used_size: u64,
    pub free_size: u64,
    pub allocation_count: u32,
    pub max_allocation_size: u64,
}

/// C-compatible scheduler statistics (must match kernel driver)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStatsC {
    pub ai_task_count: u32,
    pub boosted_thread_count: u32,
    pub total_gpu_time: u64,
    pub average_latency_ms: f32,
}

/// Type-safe wrapper for AI driver device handle
///
/// Uses RAII pattern to ensure the device handle is properly closed
pub struct AiDriverHandle {
    handle: HANDLE,
}

impl AiDriverHandle {
    /// Open connection to the AI kernel driver
    ///
    /// # Safety
    ///
    /// This function is safe, but the underlying CreateFileW call may fail
    /// if the driver is not installed or accessible.
    pub fn open() -> Result<Self> {
        let device_name = windows::core::w!(AI_DRIVER_DEVICE_NAME);

        // SAFETY: CreateFileW is a Windows API call. We ensure:
        // - Device name is a valid null-terminated wide string
        // - Handle is properly managed via RAII
        let handle = unsafe {
            CreateFileW(
                device_name,
                0, // No access flags needed for IOCTL
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }?;

        if handle == INVALID_HANDLE_VALUE {
            anyhow::bail!("Failed to open AI driver device (driver may not be installed)");
        }

        info!("Successfully opened AI driver device handle");
        Ok(Self { handle })
    }

    /// Get GPU status from kernel driver
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - IOCTL call fails
    /// - Driver returns invalid data
    pub fn get_gpu_status(&self) -> Result<GpuStats> {
        let mut status = GpuStatusC {
            utilization: 0.0,
            memory_used: 0,
            memory_total: 0,
            temperature: 0.0,
            power_usage: 0.0,
            clock_speed: 0,
        };

        let mut bytes_returned = 0u32;

        // SAFETY: DeviceIoControl is a Windows API call. We ensure:
        // - Buffer is properly aligned and sized for GpuStatusC
        // - Output buffer is valid and writable
        // - IOCTL code matches kernel driver definition
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                ioctl_codes::GET_GPU_STATUS,
                None,
                0,
                Some(&mut status as *mut _ as *mut c_void),
                mem::size_of::<GpuStatusC>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if result.is_err() {
            let error = result.unwrap_err();
            error!("IOCTL GET_GPU_STATUS failed: {error}");
            anyhow::bail!("Failed to get GPU status from kernel driver: {error}");
        }

        if bytes_returned != mem::size_of::<GpuStatusC>() as u32 {
            warn!(
                "Unexpected bytes returned: expected {}, got {}",
                mem::size_of::<GpuStatusC>(),
                bytes_returned
            );
        }

        debug!(
            "GPU Status: {}% utilization, {}MB/{}MB memory, {}°C",
            status.utilization,
            status.memory_used / 1024 / 1024,
            status.memory_total / 1024 / 1024,
            status.temperature
        );

        Ok(GpuStats {
            utilization: status.utilization.clamp(0.0, 100.0),
            memory_used: status.memory_used,
            memory_total: status.memory_total,
            temperature: status.temperature,
        })
    }

    /// Get memory pool statistics
    pub fn get_memory_pool_stats(&self) -> Result<MemoryPoolStatsC> {
        let mut stats = MemoryPoolStatsC {
            total_size: 0,
            used_size: 0,
            free_size: 0,
            allocation_count: 0,
            max_allocation_size: 0,
        };

        let mut bytes_returned = 0u32;

        // SAFETY: Same safety guarantees as get_gpu_status
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                ioctl_codes::GET_MEMORY_POOL,
                None,
                0,
                Some(&mut stats as *mut _ as *mut c_void),
                mem::size_of::<MemoryPoolStatsC>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if result.is_err() {
            let error = result.unwrap_err();
            error!("IOCTL GET_MEMORY_POOL failed: {error}");
            anyhow::bail!("Failed to get memory pool stats: {error}");
        }

        Ok(stats)
    }

    /// Get scheduler statistics
    pub fn get_scheduler_stats(&self) -> Result<SchedulerStatsC> {
        let mut stats = SchedulerStatsC {
            ai_task_count: 0,
            boosted_thread_count: 0,
            total_gpu_time: 0,
            average_latency_ms: 0.0,
        };

        let mut bytes_returned = 0u32;

        // SAFETY: Same safety guarantees as get_gpu_status
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                ioctl_codes::GET_SCHEDULER_STATS,
                None,
                0,
                Some(&mut stats as *mut _ as *mut c_void),
                mem::size_of::<SchedulerStatsC>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if result.is_err() {
            let error = result.unwrap_err();
            error!("IOCTL GET_SCHEDULER_STATS failed: {error}");
            anyhow::bail!("Failed to get scheduler stats: {error}");
        }

        Ok(stats)
    }

    /// Allocate pinned memory from kernel driver pool
    ///
    /// # Arguments
    ///
    /// * `size` - Size in bytes to allocate
    ///
    /// # Returns
    ///
    /// Physical address of allocated memory (for GPU DMA)
    pub fn alloc_pinned_memory(&self, size: u64) -> Result<u64> {
        let mut address: u64 = 0;
        let mut bytes_returned = 0u32;

        // SAFETY: Input buffer contains size, output buffer receives address
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                ioctl_codes::ALLOC_PINNED,
                Some(&size as *const _ as *const c_void),
                mem::size_of::<u64>() as u32,
                Some(&mut address as *mut _ as *mut c_void),
                mem::size_of::<u64>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if result.is_err() {
            let error = result.unwrap_err();
            error!("IOCTL ALLOC_PINNED failed: {error}");
            anyhow::bail!("Failed to allocate pinned memory: {error}");
        }

        if address == 0 {
            anyhow::bail!("Kernel driver returned null address for pinned memory");
        }

        info!("Allocated {} bytes of pinned memory at address 0x{:X}", size, address);
        Ok(address)
    }

    /// Free pinned memory allocated from kernel driver pool
    ///
    /// # Arguments
    ///
    /// * `address` - Physical address returned from alloc_pinned_memory
    pub fn free_pinned_memory(&self, address: u64) -> Result<()> {
        let mut bytes_returned = 0u32;

        // SAFETY: Input buffer contains address to free
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                ioctl_codes::FREE_PINNED,
                Some(&address as *const _ as *const c_void),
                mem::size_of::<u64>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            )
        };

        if result.is_err() {
            let error = result.unwrap_err();
            error!("IOCTL FREE_PINNED failed: {error}");
            anyhow::bail!("Failed to free pinned memory: {error}");
        }

        info!("Freed pinned memory at address 0x{:X}", address);
        Ok(())
    }

    /// Get the underlying Windows handle (for advanced use cases)
    ///
    /// # Safety
    ///
    /// The returned handle is managed by this struct and will be closed
    /// when the struct is dropped. Do not close it manually.
    pub fn as_raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for AiDriverHandle {
    fn drop(&mut self) {
        // SAFETY: CloseHandle is a Windows API call. The handle is valid
        // because we only create it in open() and check for INVALID_HANDLE_VALUE.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
        debug!("Closed AI driver device handle");
    }
}

/// Type-safe wrapper for pinned memory allocation
///
/// Automatically frees memory when dropped
pub struct PinnedMemory {
    handle: Arc<AiDriverHandle>,
    address: u64,
    size: u64,
}

impl PinnedMemory {
    /// Allocate pinned memory
    pub fn new(handle: Arc<AiDriverHandle>, size: u64) -> Result<Self> {
        let address = handle.alloc_pinned_memory(size)?;
        Ok(Self {
            handle,
            address,
            size,
        })
    }

    /// Get the physical address of pinned memory
    pub fn address(&self) -> u64 {
        self.address
    }

    /// Get the size of allocated memory
    pub fn size(&self) -> u64 {
        self.size
    }
}

impl Drop for PinnedMemory {
    fn drop(&mut self) {
        if let Err(e) = self.handle.free_pinned_memory(self.address) {
            error!("Failed to free pinned memory at 0x{:X}: {}", self.address, e);
        }
    }
}

