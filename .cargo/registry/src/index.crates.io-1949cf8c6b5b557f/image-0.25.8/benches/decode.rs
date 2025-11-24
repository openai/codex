use std::{fs, iter, path};

use criterion::{criterion_group, criterion_main, Criterion};
use image::ImageFormat;

#[derive(Clone, Copy)]
struct BenchDef {
    dir: &'static [&'static str],
    files: &'static [&'static str],
    format: ImageFormat,
}

fn load_all(c: &mut Criterion) {
    const BENCH_DEFS: &[BenchDef] = &[
        BenchDef {
            dir: &["bmp", "images"],
            files: &[
                "Core_1_Bit.bmp",
                "Core_4_Bit.bmp",
                "Core_8_Bit.bmp",
                "rgb16.bmp",
                "rgb24.bmp",
                "rgb32.bmp",
                "pal4rle.bmp",
                "pal8rle.bmp",
                "rgb16-565.bmp",
                "rgb32bf.bmp",
            ],
            format: ImageFormat::Bmp,
        },
        BenchDef {
            dir: &["gif", "simple"],
            files: &["alpha_gif_a.gif", "sample_1.gif"],
            format: ImageFormat::Gif,
        },
        BenchDef {
            dir: &["hdr", "images"],
            files: &["image1.hdr", "rgbr4x4.hdr"],
            format: ImageFormat::Hdr,
        },
        BenchDef {
            dir: &["ico", "images"],
            files: &[
                "bmp-24bpp-mask.ico",
                "bmp-32bpp-alpha.ico",
                "png-32bpp-alpha.ico",
                "smile.ico",
            ],
            format: ImageFormat::Ico,
        },
        BenchDef {
            dir: &["jpg", "progressive"],
            files: &["3.jpg", "cat.jpg", "test.jpg"],
            format: ImageFormat::Jpeg,
        },
        // TODO: pnm
        // TODO: png
        BenchDef {
            dir: &["tga", "testsuite"],
            files: &["cbw8.tga", "ctc24.tga", "ubw8.tga", "utc24.tga"],
            format: ImageFormat::Tga,
        },
        BenchDef {
            dir: &["tiff", "testsuite"],
            files: &[
                "hpredict.tiff",
                "hpredict_packbits.tiff",
                "mandrill.tiff",
                "rgb-3c-16b.tiff",
                "rgb32f_bw.tiff",
                "rgb32f_color.tiff",
            ],
            format: ImageFormat::Tiff,
        },
        BenchDef {
            dir: &["webp", "lossy_images"],
            files: &["simple-gray.webp", "simple-rgb.webp"],
            format: ImageFormat::WebP,
        },
    ];

    for bench in BENCH_DEFS {
        bench_load(c, bench);
    }
}

criterion_group!(benches, load_all);
criterion_main!(benches);

fn bench_load(c: &mut Criterion, def: &BenchDef) {
    let group_name = format!("load-{:?}", def.format);
    let mut group = c.benchmark_group(&group_name);
    let paths = IMAGE_DIR.iter().chain(def.dir);

    for file_name in def.files {
        let path: path::PathBuf = paths.clone().chain(iter::once(file_name)).collect();
        let buf = fs::read(path).unwrap();
        group.bench_function(file_name.to_owned(), |b| {
            b.iter(|| {
                image::load_from_memory_with_format(&buf, def.format).unwrap();
            });
        });
    }
}

const IMAGE_DIR: [&str; 3] = [".", "tests", "images"];
