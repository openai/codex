use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn parse_imf_fixdate(c: &mut Criterion) {
    c.bench_function("parse_imf_fixdate", |b| {
        b.iter(|| {
            let d = black_box("Sun, 06 Nov 1994 08:49:37 GMT");
            black_box(httpdate::parse_http_date(d)).unwrap();
        })
    });
}

pub fn parse_rfc850_date(c: &mut Criterion) {
    c.bench_function("parse_rfc850_date", |b| {
        b.iter(|| {
            let d = black_box("Sunday, 06-Nov-94 08:49:37 GMT");
            black_box(httpdate::parse_http_date(d)).unwrap();
        })
    });
}

pub fn parse_asctime(c: &mut Criterion) {
    c.bench_function("parse_asctime", |b| {
        b.iter(|| {
            let d = black_box("Sun Nov  6 08:49:37 1994");
            black_box(httpdate::parse_http_date(d)).unwrap();
        })
    });
}

struct BlackBoxWrite;

impl std::fmt::Write for BlackBoxWrite {
    fn write_str(&mut self, s: &str) -> Result<(), std::fmt::Error> {
        black_box(s);
        Ok(())
    }
}

pub fn encode_date(c: &mut Criterion) {
    c.bench_function("encode_date", |b| {
        let d = "Wed, 21 Oct 2015 07:28:00 GMT";
        black_box(httpdate::parse_http_date(d)).unwrap();
        b.iter(|| {
            use std::fmt::Write;
            let _ = write!(BlackBoxWrite, "{}", d);
        })
    });
}

criterion_group!(
    benches,
    parse_imf_fixdate,
    parse_rfc850_date,
    parse_asctime,
    encode_date
);
criterion_main!(benches);
