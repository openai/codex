use winnow::combinator::repeat;
use winnow::prelude::*;
use winnow::Result;

mod parser;
mod parser_str;

fn bench_ini(c: &mut criterion::Criterion) {
    let str = "[owner]
name=John Doe
organization=Acme Widgets Inc.

[database]
server=192.0.2.62
port=143
file=payroll.dat
\0";

    let mut group = c.benchmark_group("ini");
    group.throughput(criterion::Throughput::Bytes(str.len() as u64));
    group.bench_function(criterion::BenchmarkId::new("bytes", str.len()), |b| {
        b.iter(|| parser::categories.parse_peek(str.as_bytes()).unwrap());
    });
    group.bench_function(criterion::BenchmarkId::new("str", str.len()), |b| {
        b.iter(|| parser_str::categories.parse_peek(str).unwrap());
    });
}

fn bench_ini_keys_and_values(c: &mut criterion::Criterion) {
    let str = "server=192.0.2.62
port=143
file=payroll.dat
\0";

    fn acc<'s>(i: &mut parser::Stream<'s>) -> Result<Vec<(&'s str, &'s str)>> {
        repeat(0.., parser::key_value).parse_next(i)
    }

    let mut group = c.benchmark_group("ini keys and values");
    group.throughput(criterion::Throughput::Bytes(str.len() as u64));
    group.bench_function(criterion::BenchmarkId::new("bytes", str.len()), |b| {
        b.iter(|| acc.parse_peek(str.as_bytes()).unwrap());
    });
}

fn bench_ini_key_value(c: &mut criterion::Criterion) {
    let str = "server=192.0.2.62\n";

    let mut group = c.benchmark_group("ini key value");
    group.throughput(criterion::Throughput::Bytes(str.len() as u64));
    group.bench_function(criterion::BenchmarkId::new("bytes", str.len()), |b| {
        b.iter(|| parser::key_value.parse_peek(str.as_bytes()).unwrap());
    });
}

criterion::criterion_group!(
    benches,
    bench_ini,
    bench_ini_keys_and_values,
    bench_ini_key_value
);
criterion::criterion_main!(benches);
