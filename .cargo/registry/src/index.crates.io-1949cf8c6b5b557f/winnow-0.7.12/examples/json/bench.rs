use winnow::prelude::*;
use winnow::Partial;

mod json;
mod parser_alt;
mod parser_dispatch;
mod parser_partial;

fn json_bench(c: &mut criterion::Criterion) {
    let data = [("small", SMALL), ("canada", CANADA)];
    let mut group = c.benchmark_group("json");
    for (name, sample) in data {
        let len = sample.len();
        group.throughput(criterion::Throughput::Bytes(len as u64));

        group.bench_with_input(
            criterion::BenchmarkId::new("dispatch", name),
            &len,
            |b, _| {
                type Error = winnow::error::ErrMode<winnow::error::ContextError>;

                b.iter(|| parser_dispatch::json::<Error>.parse_peek(sample).unwrap());
            },
        );
        group.bench_with_input(
            criterion::BenchmarkId::new("modeless", name),
            &len,
            |b, _| {
                type Error = winnow::error::ContextError;

                b.iter(|| parser_dispatch::json::<Error>.parse_peek(sample).unwrap());
            },
        );
        group.bench_with_input(
            criterion::BenchmarkId::new("empty-error", name),
            &len,
            |b, _| {
                type Error<'i> = winnow::error::EmptyError;

                b.iter(|| {
                    parser_dispatch::json::<Error<'_>>
                        .parse_peek(sample)
                        .unwrap()
                });
            },
        );
        group.bench_with_input(criterion::BenchmarkId::new("alt", name), &len, |b, _| {
            type Error = winnow::error::ContextError;

            b.iter(|| parser_alt::json::<Error>.parse_peek(sample).unwrap());
        });
        group.bench_with_input(
            criterion::BenchmarkId::new("streaming", name),
            &len,
            |b, _| {
                type Error = winnow::error::ContextError;

                b.iter(|| {
                    parser_partial::json::<Error>
                        .parse_peek(Partial::new(sample))
                        .unwrap()
                });
            },
        );
    }
    group.finish();
}

const SMALL: &str = "  { \"a\"\t: 42,
  \"b\": [ \"x\", \"y\", 12 ,\"\\u2014\", \"\\uD83D\\uDE10\"] ,
  \"c\": { \"hello\" : \"world\"
  }
  }  ";

const CANADA: &str = include_str!("../../third_party/nativejson-benchmark/data/canada.json");

criterion::criterion_group!(benches, json_bench,);
criterion::criterion_main!(benches);
