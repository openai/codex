mod parser;
mod parser_streaming;

fn one_test(c: &mut criterion::Criterion) {
    let data = &b"GET / HTTP/1.1
Host: www.reddit.com
User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.8; rv:15.0) Gecko/20100101 Firefox/15.0.1
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8
Accept-Language: en-us,en;q=0.5
Accept-Encoding: gzip, deflate
Connection: keep-alive

"[..];

    let mut http_group = c.benchmark_group("http");
    http_group.throughput(criterion::Throughput::Bytes(data.len() as u64));
    http_group.bench_with_input(
        criterion::BenchmarkId::new("complete", data.len()),
        data,
        |b, data| {
            b.iter(|| parser::parse(data).unwrap());
        },
    );
    http_group.bench_with_input(
        criterion::BenchmarkId::new("streaming", data.len()),
        data,
        |b, data| {
            b.iter(|| parser_streaming::parse(data).unwrap());
        },
    );

    http_group.finish();
}

criterion::criterion_group!(http, one_test);
criterion::criterion_main!(http);
