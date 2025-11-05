//! Windows AI × Kernel Driver Bridge
//!
//! This module provides integration between Windows 11 AI API and
//! the Codex AI kernel driver for maximum performance.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{debug, info};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::DeviceIoControl;

/// IOCTL codes (must match ai_driver_ioctl.c)
const IOCTL_AI_GET_GPU_STATUS: u32 = 0x22300C;  // CTL_CODE(FILE_DEVICE_UNKNOWN, 0x803, ...)
const IOCTL_AI_GET_MEMORY_POOL: u32 = 0x223010;
const IOCTL_AI_GET_SCHEDULER_STATS: u32 = 0x223014;
const IOCTL_AI_ALLOC_PINNED: u32 = 0x223018;
const IOCTL_AI_FREE_PINNED: u32 = 0x22301C;
const IOCTL_AI_REGISTER_WINAI: u32 = 0x223020;
const IOCTL_AI_GET_OPTIMIZED_PATH: u32 = 0x223024;

/// GPU Statistics from kernel driver
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuStats {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f32,
}

/// Memory Pool Status
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryPoolStatus {
    pub total_size: u64,
    pub used_size: u64,
    pub free_size: u64,
    pub block_count: u32,
    pub fragmentation_ratio: f32,
}

/// Scheduler Statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    pub ai_processes: u32,
    pub scheduled_tasks: u32,
    pub average_latency_ms: f32,
}

/// Windows AI × Kernel Driver Bridge
pub struct WindowsAiBridge {
    #[cfg(target_os = "windows")]
    driver_handle: HANDLE,
}

impl WindowsAiBridge {
    /// Open connection to AI kernel driver
    pub fn open() -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            let device_path = windows::core::w!("\\\\.\\AI_Driver");
            
            unsafe {
                let handle = CreateFileW(
                    device_path,
                    FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )?;
                
                if handle.is_invalid() {
                    anyhow::bail!("Failed to open AI kernel driver - is it installed?");
                }
                
                info!("AI Kernel Driver bridge opened");
                Ok(Self {
                    driver_handle: handle,
                })
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI bridge is only available on Windows")
        }
    }
    
    /// Register Windows AI runtime with kernel driver
    pub fn register_windows_ai_runtime(&self, runtime_handle: usize) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let handle_value: u64 = runtime_handle as u64;
            let mut bytes_returned: u32 = 0;
            
            unsafe {
                let success = DeviceIoControl(
                    self.driver_handle,
                    IOCTL_AI_REGISTER_WINAI,
                    Some(&handle_value as *const _ as *const _),
                    std::mem::size_of::<u64>() as u32,
                    None,
                    0,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if success.as_bool() {
                    info!("Windows AI runtime registered with kernel driver");
                    Ok(())
                } else {
                    anyhow::bail!("Failed to register Windows AI runtime")
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            let _ = runtime_handle;
            anyhow::bail!("Windows AI bridge is only available on Windows")
        }
    }
    
    /// Get GPU statistics from kernel driver
    pub fn get_gpu_stats(&self) -> Result<GpuStats> {
        #[cfg(target_os = "windows")]
        {
            let mut stats = GpuStats {
                utilization: 0.0,
                memory_used: 0,
                memory_total: 0,
                temperature: 0.0,
            };
            
            let mut bytes_returned: u32 = 0;
            
            unsafe {
                let success = DeviceIoControl(
                    self.driver_handle,
                    IOCTL_AI_GET_GPU_STATUS,
                    None,
                    0,
                    Some(&mut stats as *mut _ as *mut _),
                    std::mem::size_of::<GpuStats>() as u32,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if success.as_bool() {
                    debug!("GPU Stats from kernel: util={:.1}%, mem={}/{}MB",
                        stats.utilization,
                        stats.memory_used / 1024 / 1024,
                        stats.memory_total / 1024 / 1024
                    );
                    Ok(stats)
                } else {
                    anyhow::bail!("Failed to get GPU stats from kernel driver")
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI bridge is only available on Windows")
        }
    }
    
    /// Get memory pool status
    pub fn get_memory_pool_status(&self) -> Result<MemoryPoolStatus> {
        #[cfg(target_os = "windows")]
        {
            let mut status = MemoryPoolStatus {
                total_size: 0,
                used_size: 0,
                free_size: 0,
                block_count: 0,
                fragmentation_ratio: 0.0,
            };
            
            let mut bytes_returned: u32 = 0;
            
            unsafe {
                let success = DeviceIoControl(
                    self.driver_handle,
                    IOCTL_AI_GET_MEMORY_POOL,
                    None,
                    0,
                    Some(&mut status as *mut _ as *mut _),
                    std::mem::size_of::<MemoryPoolStatus>() as u32,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if success.as_bool() {
                    Ok(status)
                } else {
                    anyhow::bail!("Failed to get memory pool status")
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI bridge is only available on Windows")
        }
    }
    
    /// Get scheduler statistics
    pub fn get_scheduler_stats(&self) -> Result<SchedulerStats> {
        #[cfg(target_os = "windows")]
        {
            let mut stats = SchedulerStats {
                ai_processes: 0,
                scheduled_tasks: 0,
                average_latency_ms: 0.0,
            };
            
            let mut bytes_returned: u32 = 0;
            
            unsafe {
                let success = DeviceIoControl(
                    self.driver_handle,
                    IOCTL_AI_GET_SCHEDULER_STATS,
                    None,
                    0,
                    Some(&mut stats as *mut _ as *mut _),
                    std::mem::size_of::<SchedulerStats>() as u32,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if success.as_bool() {
                    Ok(stats)
                } else {
                    anyhow::bail!("Failed to get scheduler stats")
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI bridge is only available on Windows")
        }
    }
    
    /// Get optimized execution path flags
    pub fn get_optimized_path(&self) -> Result<u32> {
        #[cfg(target_os = "windows")]
        {
            let mut flags: u32 = 0;
            let mut bytes_returned: u32 = 0;
            
            unsafe {
                let success = DeviceIoControl(
                    self.driver_handle,
                    IOCTL_AI_GET_OPTIMIZED_PATH,
                    None,
                    0,
                    Some(&mut flags as *mut _ as *mut _),
                    std::mem::size_of::<u32>() as u32,
                    Some(&mut bytes_returned),
                    None,
                );
                
                if success.as_bool() {
                    info!("Optimized path flags: 0x{flags:08X}");
                    Ok(flags)
                } else {
                    anyhow::bail!("Failed to get optimized path")
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows AI bridge is only available on Windows")
        }
    }
}

impl Drop for WindowsAiBridge {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                CloseHandle(self.driver_handle);
            }
            debug!("AI Kernel Driver bridge closed");
        }
    }
}

