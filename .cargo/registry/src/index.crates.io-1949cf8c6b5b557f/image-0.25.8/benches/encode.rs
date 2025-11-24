extern crate criterion;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use image::ExtendedColorType;
use image::{codecs::bmp::BmpEncoder, codecs::jpeg::JpegEncoder, ColorType};

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

trait Encoder {
    fn encode_raw(&self, into: &mut Vec<u8>, im: &[u8], dims: u32, color: ExtendedColorType);
    fn encode_bufvec(&self, into: &mut Vec<u8>, im: &[u8], dims: u32, color: ExtendedColorType);
    fn encode_file(&self, file: &File, im: &[u8], dims: u32, color: ExtendedColorType);
}

#[derive(Clone, Copy)]
struct BenchDef {
    with: &'static dyn Encoder,
    name: &'static str,
    sizes: &'static [u32],
    colors: &'static [ColorType],
}

fn encode_all(c: &mut Criterion) {
    const BENCH_DEFS: &[BenchDef] = &[
        BenchDef {
            with: &Bmp,
            name: "bmp",
            sizes: &[100u32, 200, 400],
            colors: &[ColorType::L8, ColorType::Rgb8, ColorType::Rgba8],
        },
        BenchDef {
            with: &Jpeg,
            name: "jpeg",
            sizes: &[64u32, 128, 256],
            colors: &[ColorType::L8, ColorType::Rgb8, ColorType::Rgba8],
        },
    ];

    for definition in BENCH_DEFS {
        encode_definition(c, definition);
    }
}

criterion_group!(benches, encode_all);
criterion_main!(benches);

type BenchGroup<'a> = criterion::BenchmarkGroup<'a, criterion::measurement::WallTime>;

/// Benchmarks encoding a zeroed image.
///
/// For compressed formats this is surely not representative of encoding a normal image but it's a
/// start for benchmarking.
fn encode_zeroed(group: &mut BenchGroup, with: &dyn Encoder, size: u32, color: ExtendedColorType) {
    let im = vec![0; (color.bits_per_pixel() as usize * size as usize).div_ceil(8) * size as usize];

    group.bench_with_input(
        BenchmarkId::new(format!("zero-{color:?}-rawvec"), size),
        &im,
        |b, image| {
            let mut v = vec![];
            with.encode_raw(&mut v, &im, size, color);
            b.iter(|| with.encode_raw(&mut v, image, size, color));
        },
    );
    group.bench_with_input(
        BenchmarkId::new(format!("zero-{color:?}-bufvec"), size),
        &im,
        |b, image| {
            let mut v = vec![];
            with.encode_raw(&mut v, &im, size, color);
            b.iter(|| with.encode_bufvec(&mut v, image, size, color));
        },
    );
    group.bench_with_input(
        BenchmarkId::new(format!("zero-{color:?}-file"), size),
        &im,
        |b, image| {
            let file = File::create("temp.bmp").unwrap();
            b.iter(|| with.encode_file(&file, image, size, color));
        },
    );
}

fn encode_definition(criterion: &mut Criterion, def: &BenchDef) {
    let mut group = criterion.benchmark_group(format!("encode-{}", def.name));

    for &color in def.colors {
        for &size in def.sizes {
            encode_zeroed(&mut group, def.with, size, color.into());
        }
    }
}

struct Bmp;

struct Jpeg;

trait EncoderBase {
    fn encode(&self, into: impl Write, im: &[u8], dims: u32, color: ExtendedColorType);
}

impl<T: EncoderBase> Encoder for T {
    fn encode_raw(&self, into: &mut Vec<u8>, im: &[u8], dims: u32, color: ExtendedColorType) {
        into.clear();
        self.encode(into, im, dims, color);
    }

    fn encode_bufvec(&self, into: &mut Vec<u8>, im: &[u8], dims: u32, color: ExtendedColorType) {
        into.clear();
        let buf = BufWriter::new(into);
        self.encode(buf, im, dims, color);
    }

    fn encode_file(&self, mut file: &File, im: &[u8], dims: u32, color: ExtendedColorType) {
        file.seek(SeekFrom::Start(0)).unwrap();
        let buf = BufWriter::new(file);
        self.encode(buf, im, dims, color);
    }
}

impl EncoderBase for Bmp {
    fn encode(&self, mut into: impl Write, im: &[u8], size: u32, color: ExtendedColorType) {
        let mut x = BmpEncoder::new(&mut into);
        x.encode(im, size, size, color).unwrap();
    }
}

impl EncoderBase for Jpeg {
    fn encode(&self, mut into: impl Write, im: &[u8], size: u32, color: ExtendedColorType) {
        let mut x = JpegEncoder::new(&mut into);
        x.encode(im, size, size, color).unwrap();
    }
}
