use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput, BatchSize};

const REQ_SHORT: &[u8] = b"\
GET / HTTP/1.0\r\n\
Host: example.com\r\n\
Cookie: session=60; user_id=1\r\n\r\n";

const REQ: &[u8] = b"\
GET /wp-content/uploads/2010/03/hello-kitty-darth-vader-pink.jpg HTTP/1.1\r\n\
Host: www.kittyhell.com\r\n\
User-Agent: Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10.6; ja-JP-mac; rv:1.9.2.3) Gecko/20100401 Firefox/3.6.3 Pathtraq/0.9\r\n\
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
Accept-Language: ja,en-us;q=0.7,en;q=0.3\r\n\
Accept-Encoding: gzip,deflate\r\n\
Accept-Charset: Shift_JIS,utf-8;q=0.7,*;q=0.7\r\n\
Keep-Alive: 115\r\n\
Connection: keep-alive\r\n\
Cookie: wp_ozh_wsa_visits=2; wp_ozh_wsa_visit_lasttime=xxxxxxxxxx; __utma=xxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.x; __utmz=xxxxxxxxx.xxxxxxxxxx.x.x.utmccn=(referral)|utmcsr=reader.livedoor.com|utmcct=/reader/|utmcmd=referral|padding=under256\r\n\r\n";

fn req(c: &mut Criterion) {
    c.benchmark_group("req")
        .throughput(Throughput::Bytes(REQ.len() as u64))
        .bench_function("req", |b| b.iter_batched_ref(|| {
            [httparse::Header {
                name: "",
                value: &[],
            }; 16]
        },|headers| {
            let mut req = httparse::Request::new(headers);
            assert_eq!(
                black_box(req.parse(REQ).unwrap()),
                httparse::Status::Complete(REQ.len())
            );
        }, BatchSize::SmallInput));
}

fn req_short(c: &mut Criterion) {
    c.benchmark_group("req_short")
        .throughput(Throughput::Bytes(REQ_SHORT.len() as u64))
        .bench_function("req_short", |b| b.iter_batched_ref(|| {
            [httparse::Header {
                name: "",
                value: &[],
            }; 16]
        },|headers| {
            let mut req = httparse::Request::new(headers);
            assert_eq!(
                req.parse(black_box(REQ_SHORT)).unwrap(),
                httparse::Status::Complete(REQ_SHORT.len())
            );
        }, BatchSize::SmallInput));
}

const RESP_SHORT: &[u8] = b"\
HTTP/1.0 200 OK\r\n\
Date: Wed, 21 Oct 2015 07:28:00 GMT\r\n\
Set-Cookie: session=60; user_id=1\r\n\r\n";

// These particular headers don't all make semantic sense for a response, but they're syntactically valid.
const RESP: &[u8] = b"\
HTTP/1.1 200 OK\r\n\
Date: Wed, 21 Oct 2015 07:28:00 GMT\r\n\
Host: www.kittyhell.com\r\n\
User-Agent: Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10.6; ja-JP-mac; rv:1.9.2.3) Gecko/20100401 Firefox/3.6.3 Pathtraq/0.9\r\n\
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
Accept-Language: ja,en-us;q=0.7,en;q=0.3\r\n\
Accept-Encoding: gzip,deflate\r\n\
Accept-Charset: Shift_JIS,utf-8;q=0.7,*;q=0.7\r\n\
Keep-Alive: 115\r\n\
Connection: keep-alive\r\n\
Cookie: wp_ozh_wsa_visits=2; wp_ozh_wsa_visit_lasttime=xxxxxxxxxx; __utma=xxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.xxxxxxxxxx.x; __utmz=xxxxxxxxx.xxxxxxxxxx.x.x.utmccn=(referral)|utmcsr=reader.livedoor.com|utmcct=/reader/|utmcmd=referral|padding=under256\r\n\r\n";

fn resp(c: &mut Criterion) {
    c.benchmark_group("resp")
        .throughput(Throughput::Bytes(RESP.len() as u64))
        .bench_function("resp", |b| b.iter_batched_ref(|| {
            [httparse::Header {
                name: "",
                value: &[],
            }; 16]
        }, |headers| {
            let mut resp = httparse::Response::new(headers);
            assert_eq!(
                resp.parse(black_box(RESP)).unwrap(),
                httparse::Status::Complete(RESP.len())
            );
        }, BatchSize::SmallInput));
}

fn resp_short(c: &mut Criterion) {
    c.benchmark_group("resp_short")
        .throughput(Throughput::Bytes(RESP_SHORT.len() as u64))
        .bench_function("resp_short", |b| b.iter_batched_ref(|| {
            [httparse::Header {
                name: "",
                value: &[],
            }; 16]
        },
        |headers| {
            let mut resp = httparse::Response::new(headers);
            assert_eq!(
                resp.parse(black_box(RESP_SHORT)).unwrap(),
                httparse::Status::Complete(RESP_SHORT.len())
            );
        }, BatchSize::SmallInput));
}

fn uri(c: &mut Criterion) {
    fn _uri(c: &mut Criterion, name: &str, input: &'static [u8]) {
        c.benchmark_group("uri")
        .throughput(Throughput::Bytes(input.len() as u64))
        .bench_function(name, |b| b.iter(|| {
            let mut b = httparse::_benchable::Bytes::new(black_box(input));
            httparse::_benchable::parse_uri(&mut b).unwrap()
        }));
    }

    const S: &[u8] = b" ";
    const CHUNK64: &[u8] = b"/wp-content/uploads/2022/08/31/hello-kitty-darth-vader-pink.webp";
    let chunk_4k = CHUNK64.repeat(64);

    // 1b to 4096b
    for p in 0..=12 {
        let n = 1 << p;
        _uri(c, &format!("uri_{:04}b", n), [chunk_4k[..n].to_vec(), S.into()].concat().leak());
    }
}

fn header(c: &mut Criterion) {
    fn _header(c: &mut Criterion, name: &str, input: &'static [u8]) {
        c.benchmark_group("header")
        .throughput(Throughput::Bytes(input.len() as u64))
        .bench_function(name, |b| b.iter_batched_ref(|| [httparse::EMPTY_HEADER; 128],|headers| {
            let status = httparse::parse_headers(black_box(input), headers).unwrap();
            black_box(status.unwrap()).0
        }, BatchSize::SmallInput));
    }

    const RN: &[u8] = b"\r\n";
    const RNRN: &[u8] = b"\r\n\r\n";
    const TINY_RN: &[u8] = b"a: b\r\n"; // minimal header line
    const XFOOBAR: &[u8] = b"X-Foobar";
    let xfoobar_4k = XFOOBAR.repeat(4096/XFOOBAR.len());

    // header names 1b to 4096b
    for p in 0..=12 {
        let n = 1 << p;
        let payload = [&xfoobar_4k[..n], b": b", RNRN].concat().leak();
        _header(c, &format!("name_{:04}b", n), payload);
    }

    // header values 1b to 4096b
    for p in 0..=12 {
        let n = 1 << p;
        let payload = [b"a: ", &xfoobar_4k[..n], RNRN].concat().leak();
        _header(c, &format!("value_{:04}b", n), payload);
    }

    // 1 to 128
    for p in 0..=7 {
        let n = 1 << p;
        _header(c, &format!("count_{:03}", n), [TINY_RN.repeat(n), RN.into()].concat().leak());
    }
}

fn version(c: &mut Criterion) {
    fn _version(c: &mut Criterion, name: &str, input: &'static [u8]) {
        c.benchmark_group("version")
        .throughput(Throughput::Bytes(input.len() as u64))
        .bench_function(name, |b| b.iter(|| {
            let mut b = httparse::_benchable::Bytes::new(black_box(input));
            httparse::_benchable::parse_version(&mut b).unwrap()
        }));
    }

    _version(c, "http10", b"HTTP/1.0\r\n");
    _version(c, "http11", b"HTTP/1.1\r\n");
    _version(c, "partial", b"HTTP/1.");
}

fn method(c: &mut Criterion) {
    fn _method(c: &mut Criterion, name: &str, input: &[u8]) {
        c.benchmark_group("method")
        .throughput(Throughput::Bytes(input.len() as u64))
        .bench_function(name, |b| b.iter(|| {
            let mut b = httparse::_benchable::Bytes::new(black_box(input));
            httparse::_benchable::parse_method(&mut b).unwrap()
        }));
    }

    // Common methods should be fast-pathed
    const COMMON_METHODS: &[&str] = &["GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH"];
    for method in COMMON_METHODS {
        _method(c, &method.to_lowercase(), format!("{} / HTTP/1.1\r\n", method).as_bytes());
    }
    // Custom methods should be infrequent and thus not worth optimizing
    _method(c, "custom", b"CUSTOM / HTTP/1.1\r\n");
    _method(c, "w3!rd", b"w3!rd / HTTP/1.1\r\n");
}

fn many_requests(c: &mut Criterion) {
    use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
    let mut requests = [
        ("GET", 500),
        ("POST", 300),
        ("OPTIONS", 100),
        ("HEAD", 50),
        ("w3!r`d", 20),
    ]
    .iter()
    .flat_map(|&(method, count)| std::iter::repeat(method).take(count))
    .map(|method| format!("{method} / HTTP/1.1\r\n\r\n"))
    .collect::<Vec<_>>();
    SliceRandom::shuffle(&mut *requests, &mut StdRng::seed_from_u64(0));

    let total_bytes: usize = requests.iter().map(String::len).sum();

    c.benchmark_group("many_requests")
        .throughput(Throughput::Bytes(total_bytes as u64))
        .measurement_time(Duration::from_secs(1))
        .sample_size(1000)
        .bench_function("_", |b| {
            b.iter(|| {
                requests.iter().for_each(|req| {
                    let mut b = httparse::_benchable::Bytes::new(black_box(req.as_bytes()));
                    httparse::_benchable::parse_method(&mut b).unwrap();
                });
            })
        });
}

const WARMUP: Duration = Duration::from_millis(100);
const MTIME: Duration = Duration::from_millis(100);
const SAMPLES: usize = 200;
criterion_group!{
    name = benches;
    config = Criterion::default().sample_size(SAMPLES).warm_up_time(WARMUP).measurement_time(MTIME);
    targets = req, req_short, resp, resp_short, uri, header, version, method, many_requests
}
criterion_main!(benches);
