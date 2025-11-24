#![feature(test)]

extern crate fdlimit;
extern crate test;
extern crate tiny_http;

use std::io::Write;
use std::process::Command;
use tiny_http::Method;

#[test]
#[ignore]
// TODO: obtain time
fn curl_bench() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let num_requests = 10usize;

    match Command::new("curl")
        .arg("-s")
        .arg(format!("http://localhost:{}/?[1-{}]", port, num_requests))
        .output()
    {
        Ok(p) => p,
        Err(_) => return, // ignoring test
    };

    drop(server);
}

#[bench]
fn sequential_requests(bencher: &mut test::Bencher) {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();

    bencher.iter(|| {
        (write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();

        let request = server.recv().unwrap();

        assert_eq!(request.method(), &Method::Get);

        request.respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)));
    });
}

#[bench]
fn parallel_requests(bencher: &mut test::Bencher) {
    fdlimit::raise_fd_limit();

    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    bencher.iter(|| {
        let mut streams = Vec::new();

        for _ in 0..1000usize {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            (write!(
                stream,
                "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            ))
            .unwrap();
            streams.push(stream);
        }

        loop {
            let request = match server.try_recv().unwrap() {
                None => break,
                Some(rq) => rq,
            };

            assert_eq!(request.method(), &Method::Get);

            request.respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)));
        }
    });
}
