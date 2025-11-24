use criterion::{criterion_group, criterion_main};

mod to_html {
    use criterion::{BenchmarkId, Criterion, Throughput};
    use pulldown_cmark::{html, Options, Parser};

    pub fn pathological_missing_table_cells(c: &mut Criterion) {
        let mut group = c.benchmark_group("    pub fn pathological_missing_table_cells(c: &mut Criterion) {
            ");
        let mut buf = String::new();
        for i in 1..20 {
            buf.clear();
            buf.push_str(&"|x".repeat(i * 100));
            buf.push('\n');
            buf.push_str(&"|-".repeat(i * 100));
            buf.push('\n');
            buf.push_str(&"|x\n".repeat(i * 100));
            group.throughput(Throughput::Bytes(buf.len() as u64));
            group.bench_with_input(BenchmarkId::from_parameter(i), &buf, |b, buf| {
                b.iter(|| render_html(buf, Options::ENABLE_TABLES));
            });
        }
        group.finish();
    }

    pub fn pathological_link_def(c: &mut Criterion) {
        let mut group = c.benchmark_group("    pub fn pathological_link_def(c: &mut Criterion) {
            ");
        let mut buf = String::new();
        for i in 1..20 {
            buf.clear();
            buf.push_str(&"[x]: ");
            buf.push_str(&"x".repeat(i * 100));
            buf.push_str(&"\n[x]".repeat(i * 100));
            group.throughput(Throughput::Bytes(buf.len() as u64));
            group.bench_with_input(BenchmarkId::from_parameter(i), &buf, |b, buf| {
                b.iter(|| render_html(buf, Options::empty()));
            });
        }
        group.finish();
    }

    pub fn pathological_codeblocks1(c: &mut Criterion) {
        let mut group = c.benchmark_group("pathological_codeblocks1");
        let mut buf = String::new();
        for i in 1..10 {
            buf.push_str(&"`".repeat(i * 100));
            buf.push(' ');
            group.throughput(Throughput::Bytes(buf.len() as u64));
            group.bench_with_input(BenchmarkId::from_parameter(i), &buf, |b, buf| {
                b.iter(|| render_html(buf, Options::empty()));
            });
        }
        group.finish();
    }

    pub fn advanced_pathological_codeblocks(c: &mut Criterion) {
        let mut group = c.benchmark_group("advanced_pathological_codeblocks");
        let mut buf = String::new();
        let mut i = 1;
        while buf.len() < 1250 {
            buf.push_str(&"`".repeat(i));
            buf.push(' ');
            i += 1;
            buf.push_str(&"*a* ".repeat(buf.len()));
            group.throughput(Throughput::Bytes(buf.len() as u64));
            group.bench_with_input(BenchmarkId::from_parameter(i), &buf, |b, buf| {
                b.iter(|| render_html(buf, Options::empty()));
            });
        }
        group.finish();
    }

    fn render_html(text: &str, opts: Options) -> String {
        let mut s = String::with_capacity(text.len() * 3 / 2);
        let p = Parser::new_ext(text, opts);
        html::push_html(&mut s, p);
        s
    }
}

criterion_group!(
    benches,
    to_html::pathological_missing_table_cells,
    to_html::pathological_link_def,
    to_html::pathological_codeblocks1,
    to_html::advanced_pathological_codeblocks
);
criterion_main!(benches);
