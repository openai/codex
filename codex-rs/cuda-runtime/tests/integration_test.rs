//! CUDA Runtime Integration Tests

use codex_cuda_runtime::*;

#[test]
fn test_cuda_availability() {
    let available = CudaRuntime::is_available();
    println!("CUDA available: {available}");

    if available {
        let count = CudaRuntime::device_count();
        println!("CUDA devices: {count}");

        assert!(count > 0, "CUDA available but no devices found");
    }
}

#[test]
fn test_runtime_creation() {
    if !CudaRuntime::is_available() {
        println!("⚠️  CUDA not available, skipping test");
        return;
    }

    match CudaRuntime::new(0) {
        Ok(runtime) => {
            println!("✓ CUDA Runtime created");

            let info = runtime.get_device_info().unwrap();
            println!("Device: {info}");

            assert!(!info.name.is_empty());
            assert!(info.total_memory > 0);
        }
        Err(e) => {
            panic!("Failed to create CUDA runtime: {e}");
        }
    }
}

#[test]
fn test_memory_allocation() {
    if !CudaRuntime::is_available() {
        return;
    }

    let cuda = CudaRuntime::new(0).unwrap();

    // Allocate 1MB
    let buffer = cuda.allocate::<f32>(256 * 1024);

    match buffer {
        Ok(buf) => {
            println!("✓ Allocated {} elements", buf.len());
            assert_eq!(buf.len(), 256 * 1024);
        }
        Err(e) => {
            panic!("Memory allocation failed: {e}");
        }
    }
}

#[test]
fn test_host_device_copy() {
    if !CudaRuntime::is_available() {
        return;
    }

    let cuda = CudaRuntime::new(0).unwrap();

    // Test data
    let data: Vec<f32> = (0..1000).map(|i| i as f32).collect();

    // Copy to device
    let d_data = cuda.copy_to_device(&data).unwrap();
    println!("✓ Copied {} elements to device", d_data.len());

    // Copy back
    let h_data = cuda.copy_from_device(&d_data).unwrap();
    println!("✓ Copied {} elements from device", h_data.len());

    // Verify data
    assert_eq!(data.len(), h_data.len());
    for (i, (&expected, &actual)) in data.iter().zip(h_data.iter()).enumerate() {
        assert_eq!(
            expected, actual,
            "Data mismatch at index {i}: expected {expected}, got {actual}"
        );
    }

    println!("✓ Data integrity verified");
}
