//! Windows AI Integration Tests

use codex_windows_ai::*;

#[test]
fn test_windows_ai_availability() {
    // This test always passes but logs availability
    let available = WindowsAiRuntime::is_available();
    println!("Windows AI available: {available}");
    
    #[cfg(target_os = "windows")]
    {
        if !available {
            println!("⚠️  Windows AI not available (requires Windows 11 25H2+)");
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        assert!(!available, "Windows AI should not be available on non-Windows platforms");
    }
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_runtime_creation() {
    // Try to create runtime
    // This may fail on older Windows versions
    match WindowsAiRuntime::new() {
        Ok(_runtime) => {
            println!("✓ Windows AI Runtime created successfully");
        }
        Err(e) => {
            println!("⚠️  Runtime creation failed (expected on Windows < 25H2): {e}");
        }
    }
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_kernel_driver_bridge() {
    use codex_windows_ai::kernel_driver::KernelBridge;
    
    // Try to open kernel driver
    match KernelBridge::open() {
        Ok(bridge) => {
            println!("✓ Kernel driver opened successfully");
            
            // Try to get GPU stats
            match bridge.get_gpu_stats() {
                Ok(stats) => {
                    println!("✓ GPU Stats: util={:.1}%, mem={}/{}MB",
                        stats.utilization,
                        stats.memory_used / 1024 / 1024,
                        stats.memory_total / 1024 / 1024
                    );
                    
                    assert!(stats.memory_total > 0, "GPU memory should be > 0");
                }
                Err(e) => {
                    println!("⚠️  Failed to get GPU stats: {e}");
                }
            }
        }
        Err(e) => {
            println!("⚠️  Kernel driver not available (expected if not installed): {e}");
        }
    }
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_gpu_stats() {
    if !WindowsAiRuntime::is_available() {
        println!("⚠️  Skipping GPU stats test (Windows AI not available)");
        return;
    }
    
    match WindowsAiRuntime::new() {
        Ok(runtime) => {
            match runtime.get_gpu_stats().await {
                Ok(stats) => {
                    println!("✓ GPU Stats from Windows AI:");
                    println!("  Utilization: {:.1}%", stats.utilization);
                    println!("  Memory: {}/{}MB",
                        stats.memory_used / 1024 / 1024,
                        stats.memory_total / 1024 / 1024
                    );
                    
                    assert!(stats.utilization >= 0.0 && stats.utilization <= 100.0);
                    assert!(stats.memory_total > 0);
                }
                Err(e) => {
                    println!("⚠️  Failed to get GPU stats: {e}");
                }
            }
        }
        Err(e) => {
            println!("⚠️  Runtime creation failed: {e}");
        }
    }
}

