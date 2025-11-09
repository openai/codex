//! GPU statistics polling for the TUI overlay.

use std::time::Instant;

/// GPU statistics snapshot consumed by the TUI.
#[derive(Debug, Clone)]
pub(crate) struct GpuStatsSnapshot {
    pub(crate) utilization: f32,
    pub(crate) memory_used: u64,
    pub(crate) memory_total: u64,
    pub(crate) temperature: Option<f32>,
    pub(crate) source: GpuStatsSource,
    pub(crate) timestamp: Instant,
}

/// Source of GPU statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GpuStatsSource {
    WindowsAi,
}

impl GpuStatsSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            GpuStatsSource::WindowsAi => "Windows AI",
        }
    }
}

/// Periodic GPU statistics provider.
pub(crate) struct GpuStatsProvider {
    #[cfg(all(target_os = "windows", feature = "windows-ai"))]
    runtime: codex_windows_ai::WindowsAiRuntime,
}

impl GpuStatsProvider {
    /// Try to create a new GPU statistics provider. Returns `None` when GPU
    /// monitoring is not supported on the current platform or feature set.
    #[cfg(all(target_os = "windows", feature = "windows-ai"))]
    pub(crate) fn new() -> Option<Self> {
        if !codex_windows_ai::WindowsAiRuntime::is_available() {
            return None;
        }

        match codex_windows_ai::WindowsAiRuntime::new() {
            Ok(runtime) => Some(Self { runtime }),
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "failed to initialise Windows AI runtime for GPU stats"
                );
                None
            }
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "windows-ai")))]
    pub(crate) fn new() -> Option<Self> {
        None
    }

    /// Sample the current GPU statistics. Returns `None` when the provider is
    /// no longer able to fetch stats (e.g. device removal).
    #[cfg(all(target_os = "windows", feature = "windows-ai"))]
    pub(crate) async fn sample(&mut self) -> Option<GpuStatsSnapshot> {
        match self.runtime.get_gpu_stats().await {
            Ok(stats) => Some(GpuStatsSnapshot {
                utilization: stats.utilization.clamp(0.0, 100.0),
                memory_used: stats.memory_used,
                memory_total: stats.memory_total,
                temperature: if stats.temperature > 0.0 {
                    Some(stats.temperature)
                } else {
                    None
                },
                source: GpuStatsSource::WindowsAi,
                timestamp: Instant::now(),
            }),
            Err(error) => {
                tracing::debug!(?error, "failed to fetch Windows AI GPU stats");
                None
            }
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "windows-ai")))]
    pub(crate) async fn sample(&mut self) -> Option<GpuStatsSnapshot> {
        let _ = self; // suppress unused warnings when compiled out.
        None
    }
}












