//! Codex AI-Native OS Integration
//! 
//! User-space library for interacting with AI kernel extensions

use std::fs;
use std::io;

/// AI kernel module statistics
#[derive(Debug, Clone, Default)]
pub struct KernelModuleStats {
    pub scheduler: Option<SchedulerStats>,
    pub memory: Option<MemoryStats>,
    pub gpu: Option<GpuStats>,
}

/// AI Scheduler statistics
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub gpu_utilization_percent: u32,
    pub gpu_available: bool,
    pub ai_task_count: u32,
}

/// AI Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_pool_mb: u64,
    pub block_size_kb: u64,
    pub total_blocks: u32,
    pub allocated_bytes: u64,
}

/// GPU statistics
#[derive(Debug, Clone)]
pub struct GpuStats {
    pub device_vendor: u16,
    pub device_id: u16,
    pub dma_buffer_mb: u64,
    pub transfers_to_gpu: u64,
    pub transfers_from_gpu: u64,
    pub bytes_to_gpu_mb: u64,
    pub bytes_from_gpu_mb: u64,
    pub kernel_launches: u64,
}

impl KernelModuleStats {
    /// Read statistics from kernel modules via /proc
    pub fn read() -> io::Result<Self> {
        Ok(Self {
            scheduler: Self::read_scheduler().ok(),
            memory: Self::read_memory().ok(),
            gpu: Self::read_gpu().ok(),
        })
    }
    
    fn read_scheduler() -> io::Result<SchedulerStats> {
        let content = fs::read_to_string("/proc/ai_scheduler")?;
        
        // Parse simple format
        let mut stats = SchedulerStats {
            gpu_utilization_percent: 0,
            gpu_available: false,
            ai_task_count: 0,
        };
        
        for line in content.lines() {
            if line.contains("GPU Utilization:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.gpu_utilization_percent = val as u32;
                }
            } else if line.contains("GPU Available:") {
                stats.gpu_available = line.contains("Yes");
            } else if line.contains("AI Tasks:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.ai_task_count = val as u32;
                }
            }
        }
        
        Ok(stats)
    }
    
    fn read_memory() -> io::Result<MemoryStats> {
        let content = fs::read_to_string("/proc/ai_memory")?;
        
        let mut stats = MemoryStats {
            total_pool_mb: 0,
            block_size_kb: 0,
            total_blocks: 0,
            allocated_bytes: 0,
        };
        
        for line in content.lines() {
            if line.contains("Total Pool Size:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.total_pool_mb = val;
                }
            } else if line.contains("Block Size:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.block_size_kb = val;
                }
            } else if line.contains("Total Blocks:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.total_blocks = val as u32;
                }
            } else if line.contains("Allocated:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.allocated_bytes = val;
                }
            }
        }
        
        Ok(stats)
    }
    
    fn read_gpu() -> io::Result<GpuStats> {
        let content = fs::read_to_string("/proc/ai_gpu")?;
        
        let mut stats = GpuStats {
            device_vendor: 0,
            device_id: 0,
            dma_buffer_mb: 0,
            transfers_to_gpu: 0,
            transfers_from_gpu: 0,
            bytes_to_gpu_mb: 0,
            bytes_from_gpu_mb: 0,
            kernel_launches: 0,
        };
        
        for line in content.lines() {
            if line.contains("DMA Buffer:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.dma_buffer_mb = val;
                }
            } else if line.contains("Transfers to GPU:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.transfers_to_gpu = val;
                }
            } else if line.contains("Transfers from GPU:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.transfers_from_gpu = val;
                }
            } else if line.contains("Kernel launches:") {
                if let Some(val) = Self::extract_number(line) {
                    stats.kernel_launches = val;
                }
            }
        }
        
        Ok(stats)
    }
    
    fn extract_number(line: &str) -> Option<u64> {
        line.split_whitespace()
            .find_map(|s| {
                // Remove trailing % if present
                let cleaned = s.trim_end_matches('%');
                cleaned.parse::<u64>().ok()
            })
    }
    
    /// Check if any kernel module is loaded
    pub fn is_available(&self) -> bool {
        self.scheduler.is_some() || self.memory.is_some() || self.gpu.is_some()
    }
    
    /// Print formatted statistics
    pub fn print(&self) {
        println!("🔧 AI Kernel Module Statistics\n");
        
        if let Some(ref sched) = self.scheduler {
            println!("📊 AI Scheduler:");
            println!("  GPU Utilization: {}%", sched.gpu_utilization_percent);
            println!("  GPU Available: {}", sched.gpu_available);
            println!("  AI Tasks: {}", sched.ai_task_count);
            println!();
        }
        
        if let Some(ref mem) = self.memory {
            println!("💾 AI Memory:");
            println!("  Total Pool: {} MB", mem.total_pool_mb);
            println!("  Block Size: {} KB", mem.block_size_kb);
            println!("  Total Blocks: {}", mem.total_blocks);
            println!("  Allocated: {} MB", mem.allocated_bytes / 1024 / 1024);
            println!();
        }
        
        if let Some(ref gpu) = self.gpu {
            println!("⚡ GPU Direct:");
            println!("  Device: {:04x}:{:04x}", gpu.device_vendor, gpu.device_id);
            println!("  DMA Buffer: {} MB", gpu.dma_buffer_mb);
            println!("  Transfers to GPU: {}", gpu.transfers_to_gpu);
            println!("  Transfers from GPU: {}", gpu.transfers_from_gpu);
            println!("  Kernel Launches: {}", gpu.kernel_launches);
            println!();
        }
        
        if !self.is_available() {
            println!("⚠️  No AI kernel modules loaded");
            println!("   Load with: sudo insmod ai_scheduler.ko");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_number() {
        assert_eq!(
            KernelModuleStats::extract_number("GPU Utilization: 75%"),
            Some(75)
        );
        assert_eq!(
            KernelModuleStats::extract_number("Total Pool Size: 256 MB"),
            Some(256)
        );
    }
}

