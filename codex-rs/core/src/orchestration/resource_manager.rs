//! Dynamic resource management for parallel AI agent execution
//!
//! Manages system resources (CPU, memory) and controls concurrent execution
//! to prevent system overload while maximizing throughput.

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use sysinfo::CpuRefreshKind;
use sysinfo::MemoryRefreshKind;
use sysinfo::RefreshKind;
use sysinfo::System;
use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Resource capacity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCapacity {
    /// Maximum concurrent tasks allowed
    pub max_concurrent: usize,
    /// Currently active tasks
    pub active_tasks: usize,
    /// Available slots for new tasks
    pub available_slots: usize,
}

/// System resource statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    /// CPU usage percentage (0-100)
    pub cpu_usage_percent: f32,
    /// Memory used in bytes
    pub memory_used_bytes: u64,
    /// Total memory in bytes
    pub memory_total_bytes: u64,
    /// Memory usage percentage (0-100)
    pub memory_usage_percent: f32,
    /// Number of active agent tasks
    pub active_agents: usize,
    /// Number of CPU cores
    pub cpu_cores: usize,
}

/// RAII guard for resource slots
/// Automatically releases slot when dropped
pub struct ResourceGuard {
    _permit: tokio::sync::SemaphorePermit<'static>,
    active_tasks: Arc<RwLock<usize>>,
}

impl Drop for ResourceGuard {
    fn drop(&mut self) {
        // Note: We can't await in Drop, so we spawn a task
        let active_tasks = Arc::clone(&self.active_tasks);
        tokio::spawn(async move {
            let mut tasks = active_tasks.write().await;
            if *tasks > 0 {
                *tasks -= 1;
            }
        });
    }
}

/// Manages dynamic resource allocation for agent orchestration
pub struct ResourceManager {
    /// Maximum concurrent tasks (CPU cores * 2)
    max_concurrent: usize,
    /// Currently active task count
    active_tasks: Arc<RwLock<usize>>,
    /// Semaphore to limit concurrent execution
    semaphore: &'static Semaphore,
    /// System information tracker
    system: Arc<RwLock<System>>,
    /// CPU core count
    cpu_cores: usize,
}

impl ResourceManager {
    /// Create a new resource manager with auto-detected CPU capacity
    pub fn new() -> Self {
        Self::with_multiplier(2)
    }

    /// Create a resource manager with custom CPU multiplier
    ///
    /// # Arguments
    /// * `multiplier` - Multiply CPU cores by this value for max concurrent tasks
    pub fn with_multiplier(multiplier: usize) -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        // Refresh to get initial data
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let cpu_cores = sys.cpus().len();
        let max_concurrent = cpu_cores * multiplier;

        info!(
            "Initializing ResourceManager: {} CPU cores, max {} concurrent tasks",
            cpu_cores, max_concurrent
        );

        // Leak semaphore to get 'static lifetime (safe for long-lived resource)
        let semaphore = Box::leak(Box::new(Semaphore::new(max_concurrent)));

        Self {
            max_concurrent,
            active_tasks: Arc::new(RwLock::new(0)),
            semaphore,
            system: Arc::new(RwLock::new(sys)),
            cpu_cores,
        }
    }

    /// Acquire a resource slot, waiting if necessary
    ///
    /// Returns a guard that automatically releases the slot when dropped
    pub async fn acquire_slot(&self) -> Result<ResourceGuard> {
        debug!("Acquiring resource slot...");

        // Wait for available slot
        let permit = self
            .semaphore
            .acquire()
            .await
            .context("Failed to acquire semaphore permit")?;

        // Increment active task count
        {
            let mut tasks = self.active_tasks.write().await;
            *tasks += 1;
        }

        let active = self.get_active_tasks().await;
        debug!(
            "Resource slot acquired. Active tasks: {}/{}",
            active, self.max_concurrent
        );

        Ok(ResourceGuard {
            _permit: permit,
            active_tasks: Arc::clone(&self.active_tasks),
        })
    }

    /// Get number of available slots
    pub fn get_available_slots(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get current number of active tasks
    pub async fn get_active_tasks(&self) -> usize {
        *self.active_tasks.read().await
    }

    /// Get resource capacity information
    pub async fn get_capacity(&self) -> ResourceCapacity {
        let active = self.get_active_tasks().await;
        let available = self.get_available_slots();

        ResourceCapacity {
            max_concurrent: self.max_concurrent,
            active_tasks: active,
            available_slots: available,
        }
    }

    /// Get current system resource statistics
    pub async fn get_system_stats(&self) -> Result<SystemStats> {
        let mut sys = self.system.write().await;

        // Refresh system info
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        // Calculate average CPU usage across all cores
        let cpu_usage: f32 =
            sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32;

        let memory_used = sys.used_memory();
        let memory_total = sys.total_memory();
        let memory_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        let active_agents = self.get_active_tasks().await;

        debug!(
            "System stats: CPU {:.1}%, Memory {:.1}% ({} MB / {} MB), Active agents: {}",
            cpu_usage,
            memory_percent,
            memory_used / 1024 / 1024,
            memory_total / 1024 / 1024,
            active_agents
        );

        Ok(SystemStats {
            cpu_usage_percent: cpu_usage,
            memory_used_bytes: memory_used,
            memory_total_bytes: memory_total,
            memory_usage_percent: memory_percent,
            active_agents,
            cpu_cores: self.cpu_cores,
        })
    }

    /// Check if system resources are under high load
    ///
    /// Returns true if CPU > 90% or Memory > 90%
    pub async fn is_under_high_load(&self) -> bool {
        match self.get_system_stats().await {
            Ok(stats) => {
                let high_load = stats.cpu_usage_percent > 90.0 || stats.memory_usage_percent > 90.0;

                if high_load {
                    warn!(
                        "System under high load: CPU {:.1}%, Memory {:.1}%",
                        stats.cpu_usage_percent, stats.memory_usage_percent
                    );
                }

                high_load
            }
            Err(e) => {
                warn!("Failed to check system load: {}", e);
                false
            }
        }
    }

    /// Get maximum concurrent tasks allowed
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Get number of CPU cores
    pub fn cpu_cores(&self) -> usize {
        self.cpu_cores
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_manager_creation() {
        let manager = ResourceManager::new();
        assert!(manager.cpu_cores() > 0);
        assert!(manager.max_concurrent() > 0);
        assert_eq!(manager.max_concurrent(), manager.cpu_cores() * 2);
    }

    #[tokio::test]
    async fn test_acquire_and_release_slot() {
        let manager = ResourceManager::with_multiplier(1);
        let initial_available = manager.get_available_slots();

        {
            let _guard = manager.acquire_slot().await.unwrap();
            assert_eq!(manager.get_active_tasks().await, 1);
            assert_eq!(manager.get_available_slots(), initial_available - 1);
        }

        // Give time for Drop to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Slot should be released after guard is dropped
        assert_eq!(manager.get_active_tasks().await, 0);
        assert_eq!(manager.get_available_slots(), initial_available);
    }

    #[tokio::test]
    async fn test_get_capacity() {
        let manager = ResourceManager::with_multiplier(2);
        let capacity = manager.get_capacity().await;

        assert_eq!(capacity.max_concurrent, manager.cpu_cores() * 2);
        assert_eq!(capacity.active_tasks, 0);
        assert_eq!(capacity.available_slots, capacity.max_concurrent);
    }

    #[tokio::test]
    async fn test_get_system_stats() {
        let manager = ResourceManager::new();
        let stats = manager.get_system_stats().await.unwrap();

        assert!(stats.cpu_usage_percent >= 0.0);
        assert!(stats.cpu_usage_percent <= 100.0);
        assert!(stats.memory_total_bytes > 0);
        assert!(stats.memory_usage_percent >= 0.0);
        assert!(stats.memory_usage_percent <= 100.0);
        assert_eq!(stats.cpu_cores, manager.cpu_cores());
    }

    #[tokio::test]
    async fn test_concurrent_acquisitions() {
        let manager = Arc::new(ResourceManager::with_multiplier(1));
        let max = manager.max_concurrent();

        let mut guards = Vec::new();
        for _ in 0..max {
            let guard = manager.acquire_slot().await.unwrap();
            guards.push(guard);
        }

        assert_eq!(manager.get_active_tasks().await, max);
        assert_eq!(manager.get_available_slots(), 0);

        // Drop all guards
        drop(guards);

        // Give time for Drop to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(manager.get_active_tasks().await, 0);
        assert_eq!(manager.get_available_slots(), max);
    }
}
