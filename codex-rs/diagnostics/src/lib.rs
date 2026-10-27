use std::sync::Mutex;
use std::sync::Once;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

static GAUGES: Mutex<Vec<&'static Gauge>> = Mutex::new(Vec::new());

/// A process-wide gauge that registers itself the first time it is used.
pub struct Gauge {
    name: &'static str,
    value: AtomicU64,
    registered: Once,
}

impl Gauge {
    /// Creates a gauge suitable for use in a `static` declaration.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            value: AtomicU64::new(0),
            registered: Once::new(),
        }
    }

    /// Increments the gauge and registers it if needed.
    pub fn increment(&'static self) {
        self.register();
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrements the gauge without allowing an underflow.
    pub fn decrement(&self) {
        let _ = self
            .value
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_sub(1))
            });
    }

    /// Increments the gauge for the lifetime of the returned guard.
    pub fn track(&'static self) -> GaugeGuard {
        self.increment();
        GaugeGuard { gauge: self }
    }

    fn register(&'static self) {
        self.registered.call_once(|| {
            GAUGES
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self);
        });
    }
}

/// Decrements its gauge when the measured object is dropped.
pub struct GaugeGuard {
    gauge: &'static Gauge,
}

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.gauge.decrement();
    }
}

/// The current value of one registered diagnostic gauge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GaugeSnapshot {
    pub name: &'static str,
    pub value: u64,
}

/// Best-effort operating-system measurements for the current process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessSnapshot {
    pub id: u32,
    pub resident_memory_bytes: Option<u64>,
    pub physical_footprint_bytes: Option<u64>,
}

/// Content-free diagnostic values contributed by this process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsSnapshot {
    pub process: ProcessSnapshot,
    pub gauges: Vec<GaugeSnapshot>,
}

/// Collects built-in process measurements and every registered gauge.
pub fn snapshot() -> DiagnosticsSnapshot {
    let mut gauges = GAUGES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|gauge| GaugeSnapshot {
            name: gauge.name,
            value: gauge.value.load(Ordering::Relaxed),
        })
        .collect::<Vec<_>>();
    gauges.sort_unstable_by_key(|gauge| gauge.name);

    DiagnosticsSnapshot {
        process: process_snapshot(),
        gauges,
    }
}

#[cfg(target_os = "macos")]
fn process_snapshot() -> ProcessSnapshot {
    let usage = unsafe {
        let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v0>::zeroed();
        // SAFETY: the kernel initializes this correctly sized buffer on success.
        if libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V0,
            usage.as_mut_ptr().cast(),
        ) != 0
        {
            return empty_process_snapshot();
        }
        usage.assume_init()
    };

    ProcessSnapshot {
        id: std::process::id(),
        resident_memory_bytes: Some(usage.ri_resident_size),
        physical_footprint_bytes: Some(usage.ri_phys_footprint),
    }
}

#[cfg(target_os = "linux")]
fn process_snapshot() -> ProcessSnapshot {
    // SAFETY: querying the system page size does not access caller-owned memory.
    let page_size = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) })
        .ok()
        .filter(|page_size| *page_size > 0);
    let resident_pages = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|statm| statm.split_whitespace().nth(1)?.parse::<u64>().ok());

    ProcessSnapshot {
        id: std::process::id(),
        resident_memory_bytes: resident_pages
            .zip(page_size)
            .map(|(pages, page_size)| pages.saturating_mul(page_size)),
        physical_footprint_bytes: None,
    }
}

#[cfg(target_os = "windows")]
fn process_snapshot() -> ProcessSnapshot {
    #[repr(C)]
    struct ProcessMemoryCounters {
        size: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }

    let counters = unsafe {
        let mut counters = std::mem::MaybeUninit::<ProcessMemoryCounters>::zeroed();
        let size = u32::try_from(std::mem::size_of::<ProcessMemoryCounters>()).unwrap_or(u32::MAX);
        // SAFETY: the pseudo-handle is valid and the kernel initializes this
        // correctly sized writable buffer on success.
        if K32GetProcessMemoryInfo(GetCurrentProcess(), counters.as_mut_ptr(), size) == 0 {
            return empty_process_snapshot();
        }
        counters.assume_init()
    };

    ProcessSnapshot {
        id: std::process::id(),
        resident_memory_bytes: u64::try_from(counters.working_set_size).ok(),
        physical_footprint_bytes: None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn process_snapshot() -> ProcessSnapshot {
    empty_process_snapshot()
}

#[cfg(not(target_os = "linux"))]
fn empty_process_snapshot() -> ProcessSnapshot {
    ProcessSnapshot {
        id: std::process::id(),
        resident_memory_bytes: None,
        physical_footprint_bytes: None,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
