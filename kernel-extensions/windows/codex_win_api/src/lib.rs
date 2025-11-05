//! Windows AI Driver API
//! 
//! Type-safe Rust bindings for Windows AI kernel driver

use windows::core::Error as WindowsError;
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING};
use windows::Win32::System::IO::DeviceIoControl;

/// AI Driver device path
const AI_DRIVER_DEVICE: &str = "\\\\.\\AIDriver";

/// IOCTL codes
const IOCTL_AI_GET_STATS: u32 = 0x222004;  // CTL_CODE(FILE_DEVICE_UNKNOWN, 0x801, METHOD_BUFFERED, FILE_ANY_ACCESS)
const IOCTL_AI_SET_GPU_UTIL: u32 = 0x222008;
const IOCTL_AI_BOOST_PRIORITY: u32 = 0x22200C;
const IOCTL_AI_GET_GPU_STATUS: u32 = 0x222010;
const IOCTL_AI_GET_MEMORY_POOL: u32 = 0x222014;
const IOCTL_AI_GET_SCHEDULER_STATS: u32 = 0x222018;
const IOCTL_AI_ALLOC_PINNED: u32 = 0x22201C;
const IOCTL_AI_FREE_PINNED: u32 = 0x222020;

/// AI Driver handle
#[derive(Debug)]
pub struct AiDriverHandle {
    handle: HANDLE,
}

impl AiDriverHandle {
    /// Open AI driver device
    pub fn open() -> Result<Self, WindowsError> {
        let device_path: Vec<u16> = AI_DRIVER_DEVICE
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        
        let handle = unsafe {
            CreateFileW(
                windows::core::PCWSTR::from_raw(device_path.as_ptr()),
                0x80000000 | 0x40000000,  // GENERIC_READ | GENERIC_WRITE
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                HANDLE::default(),
            )?
        };
        
        if handle == INVALID_HANDLE_VALUE {
            return Err(WindowsError::from_win32());
        }
        
        Ok(Self { handle })
    }
    
    /// Get driver statistics
    pub fn get_stats(&self) -> Result<DriverStats, WindowsError> {
        let mut stats = DriverStats::default();
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_GET_STATS,
                None,
                0,
                Some(&mut stats as *mut _ as *mut _),
                std::mem::size_of::<DriverStats>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(stats)
    }
    
    /// Set GPU utilization
    pub fn set_gpu_utilization(&self, util: u32) -> Result<(), WindowsError> {
        let util_clamped = util.min(100);
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_SET_GPU_UTIL,
                Some(&util_clamped as *const _ as *const _),
                std::mem::size_of::<u32>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(())
    }
    
    /// Boost thread priority for AI task
    pub fn boost_priority(&self, thread_id: u32) -> Result<(), WindowsError> {
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_BOOST_PRIORITY,
                Some(&thread_id as *const _ as *const _),
                std::mem::size_of::<u32>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(())
    }
    
    /// Get GPU status
    pub fn get_gpu_status(&self) -> Result<GpuStatus, WindowsError> {
        let mut status = GpuStatus::default();
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_GET_GPU_STATUS,
                None,
                0,
                Some(&mut status as *mut _ as *mut _),
                std::mem::size_of::<GpuStatus>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(status)
    }
    
    /// Get memory pool status
    pub fn get_memory_pool_status(&self) -> Result<MemoryPoolStatus, WindowsError> {
        let mut status = MemoryPoolStatus::default();
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_GET_MEMORY_POOL,
                None,
                0,
                Some(&mut status as *mut _ as *mut _),
                std::mem::size_of::<MemoryPoolStatus>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(status)
    }
    
    /// Get scheduler statistics
    pub fn get_scheduler_stats(&self) -> Result<SchedulerStats, WindowsError> {
        let mut stats = SchedulerStats::default();
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_GET_SCHEDULER_STATS,
                None,
                0,
                Some(&mut stats as *mut _ as *mut _),
                std::mem::size_of::<SchedulerStats>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(stats)
    }
    
    /// Allocate pinned memory
    pub fn alloc_pinned(&self, size: u64) -> Result<u64, WindowsError> {
        let mut address = 0u64;
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_ALLOC_PINNED,
                Some(&size as *const _ as *const _),
                std::mem::size_of::<u64>() as u32,
                Some(&mut address as *mut _ as *mut _),
                std::mem::size_of::<u64>() as u32,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(address)
    }
    
    /// Free pinned memory
    pub fn free_pinned(&self, address: u64) -> Result<(), WindowsError> {
        let mut bytes_returned = 0u32;
        
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_AI_FREE_PINNED,
                Some(&address as *const _ as *const _),
                std::mem::size_of::<u64>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            )?;
        }
        
        Ok(())
    }
}

impl Drop for AiDriverHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

/// Driver statistics
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DriverStats {
    pub ai_task_count: u32,
    pub gpu_utilization: u32,
    pub memory_pool_size: u64,
    pub memory_allocated: u64,
    pub priority_boosts: u64,
}

impl DriverStats {
    /// Print formatted statistics
    pub fn print(&self) {
        println!("📊 Windows AI Driver Statistics");
        println!("================================");
        println!("AI Tasks: {}", self.ai_task_count);
        println!("GPU Utilization: {}%", self.gpu_utilization);
        println!("Memory Pool: {} MB", self.memory_pool_size / 1024 / 1024);
        println!("Allocated: {} MB", self.memory_allocated / 1024 / 1024);
        println!("Priority Boosts: {}", self.priority_boosts);
    }
}

/// GPU Status
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuStatus {
    pub utilization: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: f32,
}

/// Memory Pool Status
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryPoolStatus {
    pub total_size: u64,
    pub used_size: u64,
    pub free_size: u64,
    pub block_count: u32,
    pub fragmentation_ratio: f32,
}

/// Scheduler Statistics
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SchedulerStats {
    pub ai_processes: u32,
    pub scheduled_tasks: u32,
    pub average_latency_ms: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_stats_default() {
        let stats = DriverStats::default();
        assert_eq!(stats.ai_task_count, 0);
        assert_eq!(stats.gpu_utilization, 0);
    }
    
    #[test]
    fn test_ioctl_codes() {
        assert_eq!(IOCTL_AI_GET_STATS, 0x222004);
        assert_eq!(IOCTL_AI_SET_GPU_UTIL, 0x222008);
        assert_eq!(IOCTL_AI_BOOST_PRIORITY, 0x22200C);
    }
}

