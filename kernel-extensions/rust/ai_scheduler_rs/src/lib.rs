//! AI Scheduler - Rust Implementation
//! 
//! GPU-aware process scheduler for AI workloads
//! User-space library for AI kernel module integration

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// GPU utilization (0-100%)
static GPU_UTILIZATION: AtomicU32 = AtomicU32::new(0);

/// GPU available flag
static GPU_AVAILABLE: AtomicU32 = AtomicU32::new(1);

/// AI task counter
static AI_TASK_COUNT: AtomicU64 = AtomicU64::new(0);

/// AI task priority
pub const AI_TASK_PRIORITY: i32 = 80;
pub const NORMAL_TASK_PRIORITY: i32 = 20;

/// Task classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    AiInference,
    Normal,
}

/// AI task information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AiTaskInfo {
    pub pid: u32,
    pub priority: i32,
    pub gpu_time_jiffies: u64,
    pub task_type: TaskType,
}

impl AiTaskInfo {
    /// Create new AI task info
    pub const fn new(pid: u32) -> Self {
        Self {
            pid,
            priority: AI_TASK_PRIORITY,
            gpu_time_jiffies: 0,
            task_type: TaskType::AiInference,
        }
    }
    
    /// Check if task should use GPU
    pub const fn should_use_gpu(&self) -> bool {
        matches!(self.task_type, TaskType::AiInference)
    }
}

/// Check if GPU is available for scheduling
#[inline]
pub fn is_gpu_available() -> bool {
    GPU_AVAILABLE.load(Ordering::Acquire) != 0
}

/// Get current GPU utilization
#[inline]
pub fn get_gpu_utilization() -> u32 {
    GPU_UTILIZATION.load(Ordering::Acquire)
}

/// Update GPU utilization
#[inline]
pub fn set_gpu_utilization(util: u32) {
    let clamped = util.min(100);
    GPU_UTILIZATION.store(clamped, Ordering::Release);
    
    // Update availability based on utilization
    if clamped < 50 {
        GPU_AVAILABLE.store(1, Ordering::Release);
    } else {
        GPU_AVAILABLE.store(0, Ordering::Release);
    }
}

/// Register AI task
pub fn register_ai_task(_pid: u32) -> Result<(), &'static str> {
    let count = AI_TASK_COUNT.fetch_add(1, Ordering::AcqRel);
    
    if count >= 1024 {
        AI_TASK_COUNT.fetch_sub(1, Ordering::AcqRel);
        return Err("Maximum AI tasks reached");
    }
    
    Ok(())
}

/// Unregister AI task
pub fn unregister_ai_task(_pid: u32) {
    AI_TASK_COUNT.fetch_sub(1, Ordering::AcqRel);
}

/// Get AI task count
#[inline]
pub fn get_ai_task_count() -> u64 {
    AI_TASK_COUNT.load(Ordering::Acquire)
}

/// Scheduling decision
pub fn should_schedule_on_gpu(task: &AiTaskInfo) -> bool {
    task.should_use_gpu() && is_gpu_available()
}


