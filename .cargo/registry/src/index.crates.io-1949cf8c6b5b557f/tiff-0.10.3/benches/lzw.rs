extern crate criterion;
extern crate tiff;

use criterion::{
    black_box, measurement::Measurement, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};
use tiff::decoder::Decoder;

fn read_image(image: &[u8]) {
    let image = std::io::Cursor::new(image);
    let decoder = Decoder::new(black_box(image));
    let mut reader = decoder.unwrap();

    while {
        reader.read_image().unwrap();
        reader.more_images()
    } {}
}

fn main() {
    struct BenchDef {
        data: &'static [u8],
        id: &'static str,
        sample_size: usize,
    }

    fn run_bench_def<M: Measurement>(group: &mut BenchmarkGroup<M>, def: BenchDef) {
        group
            .sample_size(def.sample_size)
            .throughput(Throughput::Bytes(def.data.len() as u64))
            .bench_with_input(
                BenchmarkId::new(def.id, def.data.len()),
                def.data,
                |b, input| b.iter(|| read_image(input)),
            );
    }

    let mut c = Criterion::default().configure_from_args();
    let mut group = c.benchmark_group("tiff-lzw");

    macro_rules! data_file {
        ($name:literal) => {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), $name))
        };
    }

    run_bench_def(
        &mut group,
        BenchDef {
            data: data_file!("/tests/images/issue_69_lzw.tiff"),
            id: "issue-69-lzw.tif",
            sample_size: 500,
        },
    );

    run_bench_def(
        &mut group,
        BenchDef {
            data: data_file!("/tests/benches/kodim02-lzw.tif"),
            id: "kodim02-lzw.tif",
            sample_size: 20,
        },
    );

    run_bench_def(
        &mut group,
        BenchDef {
            data: data_file!("/tests/benches/kodim07-lzw.tif"),
            id: "kodim07-lzw.tif",
            sample_size: 20,
        },
    );

    run_bench_def(
        &mut group,
        BenchDef {
            data: data_file!("/tests/benches/Transparency-lzw.tif"),
            id: "Transparency-lzw.tif",
            sample_size: 20,
        },
    );
}
