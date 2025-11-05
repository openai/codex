//! Git analysis benchmarks
//!
//! Measures performance of Git parsing and 3D coordinate calculation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::path::PathBuf;

fn benchmark_commit_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_analysis");
    
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("commits", size),
            size,
            |b, &size| {
                b.iter(|| {
                    // Simulate commit parsing
                    let mut commits = Vec::with_capacity(size);
                    for i in 0..size {
                        commits.push(black_box((
                            format!("sha-{}", i),
                            i as f32,
                            i as f32,
                            i as f32,
                        )));
                    }
                    commits
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_coordinate_calculation(c: &mut Criterion) {
    c.bench_function("3d_coordinates_1000", |b| {
        b.iter(|| {
            let mut positions = Vec::with_capacity(1000);
            for i in 0..1000 {
                let x = (i as f32) % 10.0;
                let y = (i as f32) / 10.0;
                let z = (i as f32).sqrt();
                positions.push(black_box((x, y, z)));
            }
            positions
        });
    });
}

fn benchmark_color_generation(c: &mut Criterion) {
    c.bench_function("author_color_generation", |b| {
        let emails: Vec<String> = (0..100)
            .map(|i| format!("user{}@example.com", i))
            .collect();
        
        b.iter(|| {
            for email in &emails {
                let hash = email.bytes().fold(0u32, |acc, byte| {
                    acc.wrapping_mul(31).wrapping_add(byte as u32)
                });
                let hue = (hash % 360) as f32;
                let _color = format!("hsl({}, 70%, 60%)", hue);
                black_box(_color);
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_commit_parsing,
    benchmark_coordinate_calculation,
    benchmark_color_generation
);
criterion_main!(benches);
