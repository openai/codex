//! Hybrid Acceleration Layer
//!
//! Coordinates Windows AI API and CUDA for maximum performance

use anyhow::Context;
use anyhow::Result;
use tracing::debug;
use tracing::info;

/// Acceleration mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelerationMode {
    /// CPU only (no acceleration)
    None,
    /// Windows AI API (DirectML)
    WindowsAI,
    /// CUDA direct
    CUDA,
    /// macOS Metal API
    Metal,
    /// Hybrid (automatically select best)
    Hybrid,
}

/// Acceleration options
#[derive(Debug, Clone)]
pub struct AccelerationOptions {
    pub mode: AccelerationMode,
    pub use_kernel_driver: bool,
    pub cuda_device: Option<i32>,
}

impl Default for AccelerationOptions {
    fn default() -> Self {
        Self {
            mode: AccelerationMode::None,
            use_kernel_driver: false,
            cuda_device: None,
        }
    }
}

/// Execute with hybrid acceleration
pub async fn execute_with_acceleration(
    prompt: &str,
    options: &AccelerationOptions,
) -> Result<String> {
    let mode = match options.mode {
        AccelerationMode::Hybrid => select_best_acceleration_mode()?,
        other => other,
    };

    info!("Executing with acceleration mode: {mode:?}");

    match mode {
        AccelerationMode::None => {
            // CPU fallback
            Ok(format!("CPU execution: {prompt}"))
        }

        #[cfg(target_os = "windows")]
        AccelerationMode::WindowsAI => {
            use crate::windows_ai_integration::WindowsAiOptions;
            use crate::windows_ai_integration::execute_with_windows_ai;

            let win_opts = WindowsAiOptions {
                enabled: true,
                kernel_accelerated: options.use_kernel_driver,
                use_gpu: true,
            };

            execute_with_windows_ai(prompt, &win_opts).await
        }

        #[cfg(feature = "cuda")]
        AccelerationMode::CUDA => execute_with_cuda(prompt, options).await,

        #[cfg(all(target_os = "macos", feature = "metal"))]
        AccelerationMode::Metal => execute_with_metal(prompt, options).await,

        AccelerationMode::Hybrid => {
            unreachable!("Hybrid should be resolved to specific mode")
        }

        #[cfg(not(target_os = "windows"))]
        AccelerationMode::WindowsAI => {
            anyhow::bail!("Windows AI is only available on Windows")
        }

        #[cfg(not(feature = "cuda"))]
        AccelerationMode::CUDA => {
            anyhow::bail!("CUDA support not compiled")
        }

        #[cfg(not(all(target_os = "macos", feature = "metal")))]
        AccelerationMode::Metal => {
            anyhow::bail!("Metal is only available on macOS with metal feature")
        }
    }
}

/// Select best acceleration mode based on system capabilities
fn select_best_acceleration_mode() -> Result<AccelerationMode> {
    // Priority order: CUDA > Windows AI > Metal > CPU
    
    #[cfg(feature = "cuda")]
    {
        use codex_cuda_runtime::CudaRuntime;

        if CudaRuntime::is_available() {
            info!("CUDA available, using CUDA acceleration");
            return Ok(AccelerationMode::CUDA);
        }
    }

    #[cfg(target_os = "windows")]
    {
        use codex_windows_ai::WindowsAiRuntime;

        if WindowsAiRuntime::is_available() {
            info!("Windows AI available, using Windows AI acceleration");
            return Ok(AccelerationMode::WindowsAI);
        }
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        use codex_metal_runtime::MetalRuntime;

        if MetalRuntime::is_available() {
            info!("Metal available, using Metal acceleration");
            return Ok(AccelerationMode::Metal);
        }
    }

    debug!("No acceleration available, falling back to CPU");
    Ok(AccelerationMode::None)
}

/// Execute with CUDA
#[cfg(feature = "cuda")]
async fn execute_with_cuda(prompt: &str, _options: &AccelerationOptions) -> Result<String> {
    use codex_cuda_runtime::CudaRuntime;

    let cuda = CudaRuntime::new(0).context("Failed to initialize CUDA")?;

    let device_info = cuda.get_device_info()?;
    info!("Executing with CUDA on {}", device_info.name);

    // TODO: Actual CUDA-accelerated inference
    Ok(format!("CUDA execution on {}: {prompt}", device_info.name))
}

/// Execute with Metal
#[cfg(all(target_os = "macos", feature = "metal"))]
async fn execute_with_metal(prompt: &str, _options: &AccelerationOptions) -> Result<String> {
    use codex_metal_runtime::MetalRuntime;

    let metal = MetalRuntime::new().context("Failed to initialize Metal")?;

    let chip_info = metal.get_chip_info()?;
    info!(
        "Executing with Metal on {} ({} GPU cores)",
        chip_info.chip_type.label(),
        chip_info.gpu_core_count
    );

    // TODO: Actual Metal-accelerated inference
    Ok(format!(
        "Metal execution on {}: {prompt}",
        chip_info.chip_type.label()
    ))
}

/// Get acceleration capabilities
pub fn get_acceleration_capabilities() -> AccelerationCapabilities {
    AccelerationCapabilities {
        windows_ai: check_windows_ai(),
        cuda: check_cuda(),
        metal: check_metal(),
        kernel_driver: check_kernel_driver(),
    }
}

#[cfg(target_os = "windows")]
fn check_windows_ai() -> bool {
    codex_windows_ai::WindowsAiRuntime::is_available()
}

#[cfg(not(target_os = "windows"))]
fn check_windows_ai() -> bool {
    false
}

#[cfg(feature = "cuda")]
fn check_cuda() -> bool {
    codex_cuda_runtime::CudaRuntime::is_available()
}

#[cfg(not(feature = "cuda"))]
fn check_cuda() -> bool {
    false
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn check_metal() -> bool {
    codex_metal_runtime::MetalRuntime::is_available()
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
fn check_metal() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn check_kernel_driver() -> bool {
    use codex_windows_ai::kernel_driver::KernelBridge;
    KernelBridge::open().is_ok()
}

#[cfg(not(target_os = "windows"))]
fn check_kernel_driver() -> bool {
    false
}

/// Acceleration capabilities
#[derive(Debug, Clone)]
pub struct AccelerationCapabilities {
    pub windows_ai: bool,
    pub cuda: bool,
    pub metal: bool,
    pub kernel_driver: bool,
}

impl std::fmt::Display for AccelerationCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut caps = Vec::new();

        if self.windows_ai {
            caps.push("Windows AI");
        }
        if self.cuda {
            caps.push("CUDA");
        }
        if self.metal {
            caps.push("Metal");
        }
        if self.kernel_driver {
            caps.push("Kernel Driver");
        }

        if caps.is_empty() {
            write!(f, "No acceleration")
        } else {
            write!(f, "{}", caps.join(" + "))
        }
    }
}
