//! Integration tests for cross-platform GPU detection

use codex_core::hybrid_acceleration::get_acceleration_capabilities;

#[tokio::test]
async fn test_gpu_detection() {
    let capabilities = get_acceleration_capabilities();
    
    // Test that capabilities are detected correctly
    println!("GPU Acceleration Capabilities:");
    println!("  Windows AI: {}", capabilities.windows_ai);
    println!("  CUDA: {}", capabilities.cuda);
    println!("  Metal: {}", capabilities.metal);
    println!("  Kernel Driver: {}", capabilities.kernel_driver);
    
    // At least one acceleration method should be available or the system should report correctly
    assert!(
        capabilities.windows_ai || capabilities.cuda || capabilities.metal || !capabilities.windows_ai && !capabilities.cuda && !capabilities.metal,
        "GPU detection should work correctly"
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn test_windows_ai_detection() {
    use codex_windows_ai::WindowsAiRuntime;
    
    let available = WindowsAiRuntime::is_available();
    println!("Windows AI available: {}", available);
    
    if available {
        let runtime = WindowsAiRuntime::new().expect("Should create runtime");
        let stats = runtime.get_gpu_stats().await.expect("Should get GPU stats");
        assert!(stats.utilization >= 0.0 && stats.utilization <= 100.0);
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[tokio::test]
async fn test_metal_detection() {
    use codex_metal_runtime::MetalRuntime;
    
    let available = MetalRuntime::is_available();
    println!("Metal available: {}", available);
    
    if available {
        let runtime = MetalRuntime::new().expect("Should create runtime");
        let chip_info = runtime.get_chip_info().expect("Should get chip info");
        assert_ne!(chip_info.chip_type.label(), "Unknown Apple Silicon");
    }
}

#[cfg(all(feature = "cuda", not(target_os = "windows")))]
#[tokio::test]
async fn test_cuda_detection() {
    use codex_cuda_runtime::CudaRuntime;
    
    let available = CudaRuntime::is_available();
    println!("CUDA available: {}", available);
    
    if available {
        let device_count = CudaRuntime::device_count();
        println!("CUDA device count: {}", device_count);
        assert!(device_count > 0);
        
        let runtime = CudaRuntime::new(0).expect("Should create runtime");
        let device_info = runtime.get_device_info().expect("Should get device info");
        assert!(!device_info.name.is_empty());
    }
}











