//! Git Analysis CUDA vs CPU Benchmark

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codex_cuda_runtime::CudaRuntime;

fn bench_commit_position_calculation_cpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("commit_positions");
    
    for size in [1000, 10000, 100000] {
        group.bench_with_input(BenchmarkId::new("cpu", size), &size, |b, &size| {
            let timestamps: Vec<f32> = (0..size).map(|i| i as f32).collect();
            let branch_ids: Vec<i32> = (0..size).map(|i| (i % 10) as i32).collect();
            let parent_counts: Vec<i32> = (0..size).map(|i| (i % 3) as i32).collect();
            
            b.iter(|| {
                let mut x = Vec::with_capacity(size);
                let mut y = Vec::with_capacity(size);
                let mut z = Vec::with_capacity(size);
                
                for i in 0..size {
                    x.push(black_box(branch_ids[i] as f32 * 10.0));
                    y.push(black_box(timestamps[i]));
                    z.push(black_box(parent_counts[i] as f32 * 5.0));
                }
                
                (x, y, z)
            });
        });
    }
    
    group.finish();
}

fn bench_commit_position_calculation_cuda(c: &mut Criterion) {
    if !CudaRuntime::is_available() {
        println!("⚠️  CUDA not available, skipping CUDA benchmarks");
        return;
    }
    
    let mut group = c.benchmark_group("commit_positions");
    let cuda = CudaRuntime::new(0).unwrap();
    
    for size in [1000, 10000, 100000] {
        group.bench_with_input(BenchmarkId::new("cuda", size), &size, |b, &size| {
            let timestamps: Vec<f32> = (0..size).map(|i| i as f32).collect();
            let branch_ids: Vec<i32> = (0..size).map(|i| (i % 10) as i32).collect();
            let parent_counts: Vec<i32> = (0..size).map(|i| (i % 3) as i32).collect();
            
            b.iter(|| {
                // Copy to GPU
                let d_timestamps = cuda.copy_to_device(&timestamps).unwrap();
                let d_branch_ids = cuda.copy_to_device(&branch_ids).unwrap();
                let d_parent_counts = cuda.copy_to_device(&parent_counts).unwrap();
                
                // Allocate output
                let d_x = cuda.allocate::<f32>(size).unwrap();
                let d_y = cuda.allocate::<f32>(size).unwrap();
                let d_z = cuda.allocate::<f32>(size).unwrap();
                
                // TODO: Launch CUDA kernel
                // For now, just copy back
                
                let x = cuda.copy_from_device(&d_x).unwrap();
                let y = cuda.copy_from_device(&d_y).unwrap();
                let z = cuda.copy_from_device(&d_z).unwrap();
                
                black_box((x, y, z))
            });
        });
    }
    
    group.finish();
}

criterion_group!(benches, bench_commit_position_calculation_cpu, bench_commit_position_calculation_cuda);
criterion_main!(benches);

