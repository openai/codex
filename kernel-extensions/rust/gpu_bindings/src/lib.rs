//! GPU Bindings for Rust
//! 
//! Type-safe Rust bindings for GPU operations

#![deny(warnings)]
#![deny(clippy::all)]

/// GPU device handle
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuDevice(pub u32);

impl GpuDevice {
    /// Create new GPU device handle
    pub const fn new(id: u32) -> Self {
        Self(id)
    }
    
    /// Get device ID
    pub const fn id(&self) -> u32 {
        self.0
    }
}

/// GPU memory address
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpuMemoryAddress(pub u64);

impl GpuMemoryAddress {
    /// Create new GPU memory address
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }
    
    /// Get raw address
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
    
    /// Check if address is aligned
    pub const fn is_aligned(&self, align: u64) -> bool {
        self.0 % align == 0
    }
}

/// DMA transfer direction
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaDirection {
    ToDevice = 1,
    FromDevice = 2,
    Bidirectional = 3,
}

/// GPU memory allocation flags
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuAllocFlags(pub u32);

impl GpuAllocFlags {
    pub const NONE: Self = Self(0);
    pub const PINNED: Self = Self(1 << 0);
    pub const ZERO_COPY: Self = Self(1 << 1);
    pub const WRITE_COMBINED: Self = Self(1 << 2);
    
    /// Check if flags contain specific flag
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
    
    /// Combine flags
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// GPU statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuStats {
    pub utilization_percent: u32,
    pub memory_used_bytes: u64,
    pub temperature_celsius: u32,
    pub power_draw_watts: u32,
    pub compute_units_active: u32,
}

impl Default for GpuStats {
    fn default() -> Self {
        Self {
            utilization_percent: 0,
            memory_used_bytes: 0,
            temperature_celsius: 0,
            power_draw_watts: 0,
            compute_units_active: 0,
        }
    }
}

impl GpuStats {
    /// Check if GPU is idle (< 10% utilization)
    pub const fn is_idle(&self) -> bool {
        self.utilization_percent < 10
    }
    
    /// Check if GPU is busy (> 80% utilization)
    pub const fn is_busy(&self) -> bool {
        self.utilization_percent > 80
    }
    
    /// Check if temperature is critical (> 85°C)
    pub const fn is_temperature_critical(&self) -> bool {
        self.temperature_celsius > 85
    }
}

/// Inference request
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InferenceRequest {
    pub model_id: u32,
    pub batch_size: u32,
    pub input_size: u64,
    pub output_size: u64,
    pub timeout_ms: u32,
}

impl InferenceRequest {
    /// Create new inference request
    pub const fn new(
        model_id: u32,
        batch_size: u32,
        input_size: u64,
        output_size: u64,
    ) -> Self {
        Self {
            model_id,
            batch_size,
            input_size,
            output_size,
            timeout_ms: 5000, // Default 5 seconds
        }
    }
    
    /// Set timeout
    pub const fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
    
    /// Validate request parameters
    pub const fn is_valid(&self) -> bool {
        self.batch_size > 0 &&
        self.input_size > 0 &&
        self.output_size > 0 &&
        self.timeout_ms > 0
    }
}

/// Result type for GPU operations
pub type GpuResult<T> = Result<T, GpuError>;

/// GPU operation errors
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuError {
    NotInitialized = 1,
    DeviceNotFound = 2,
    OutOfMemory = 3,
    InvalidParameter = 4,
    TransferFailed = 5,
    LaunchFailed = 6,
    Timeout = 7,
}

impl GpuError {
    /// Get error code
    pub const fn code(&self) -> u32 {
        *self as u32
    }
    
    /// Get error message
    pub const fn message(&self) -> &'static str {
        match self {
            Self::NotInitialized => "GPU not initialized",
            Self::DeviceNotFound => "GPU device not found",
            Self::OutOfMemory => "GPU out of memory",
            Self::InvalidParameter => "Invalid parameter",
            Self::TransferFailed => "DMA transfer failed",
            Self::LaunchFailed => "Kernel launch failed",
            Self::Timeout => "Operation timeout",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_device() {
        let dev = GpuDevice::new(0);
        assert_eq!(dev.id(), 0);
    }

    #[test]
    fn test_gpu_memory_address() {
        let addr = GpuMemoryAddress::new(0x1000);
        assert!(addr.is_aligned(256));
        assert!(addr.is_aligned(512));
        assert!(addr.is_aligned(0x1000));
        
        let addr2 = GpuMemoryAddress::new(0x1234);
        assert!(addr2.is_aligned(4));
        assert!(!addr2.is_aligned(256));
    }

    #[test]
    fn test_gpu_alloc_flags() {
        let flags = GpuAllocFlags::PINNED.with(GpuAllocFlags::ZERO_COPY);
        assert!(flags.contains(GpuAllocFlags::PINNED));
        assert!(flags.contains(GpuAllocFlags::ZERO_COPY));
        assert!(!flags.contains(GpuAllocFlags::WRITE_COMBINED));
    }

    #[test]
    fn test_gpu_stats() {
        let stats = GpuStats {
            utilization_percent: 5,
            temperature_celsius: 90,
            ..Default::default()
        };
        
        assert!(stats.is_idle());
        assert!(!stats.is_busy());
        assert!(stats.is_temperature_critical());
    }

    #[test]
    fn test_inference_request() {
        let req = InferenceRequest::new(1, 32, 1024, 2048)
            .with_timeout(10000);
        
        assert_eq!(req.model_id, 1);
        assert_eq!(req.batch_size, 32);
        assert_eq!(req.timeout_ms, 10000);
        assert!(req.is_valid());
    }
}

