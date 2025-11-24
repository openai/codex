use criterion::{black_box, criterion_group, criterion_main, Criterion};
use image::{GenericImage, ImageBuffer, Rgba};

pub fn bench_copy_from(c: &mut Criterion) {
    let src = ImageBuffer::from_pixel(2048, 2048, Rgba([255u8, 0, 0, 255]));
    let mut dst = ImageBuffer::from_pixel(2048, 2048, Rgba([0u8, 0, 0, 255]));

    c.bench_function("copy_from", |b| {
        b.iter(|| dst.copy_from(black_box(&src), 0, 0));
    });
}

criterion_group!(benches, bench_copy_from);
criterion_main!(benches);
