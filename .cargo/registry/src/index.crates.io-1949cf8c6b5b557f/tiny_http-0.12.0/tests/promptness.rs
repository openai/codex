extern crate tiny_http;

use std::io::{copy, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::ops::Deref;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use tiny_http::{Response, Server};

/// Stream that produces bytes very slowly
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct SlowByteSrc {
    val: u8,
    len: usize,
}
impl<'b> Read for SlowByteSrc {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        sleep(Duration::from_millis(100));
        let l = self.len.min(buf.len()).min(1000);
        for v in buf[..l].iter_mut() {
            *v = self.val;
        }
        self.len -= l;
        Ok(l)
    }
}

/// crude impl of http `Transfer-Encoding: chunked`
fn encode_chunked(data: &mut dyn Read, output: &mut dyn Write) {
    let mut buf = [0u8; 4096];
    loop {
        let l = data.read(&mut buf).unwrap();
        write!(output, "{:X}\r\n", l).unwrap();
        output.write_all(&buf[..l]).unwrap();
        write!(output, "\r\n").unwrap();
        if l == 0 {
            break;
        }
    }
}

mod prompt_pipelining {
    use super::*;

    /// Check that pipelined requests on the same connection are received promptly.
    fn assert_requests_parsed_promptly(
        req_cnt: usize,
        req_body: &'static [u8],
        timeout: Duration,
        req_writer: impl FnOnce(&mut dyn Write) + Send + 'static,
    ) {
        let resp_body = SlowByteSrc {
            val: 42,
            len: 1000_000,
        }; // very slow response body

        let server = Server::http("0.0.0.0:0").unwrap();
        let mut client = TcpStream::connect(server.server_addr().to_ip().unwrap()).unwrap();
        let (svr_send, svr_rcv) = channel();

        spawn(move || {
            for _ in 0..req_cnt {
                let mut req = server.recv().unwrap();
                // read the whole body of the request
                let mut body = Vec::new();
                req.as_reader().read_to_end(&mut body).unwrap();
                assert_eq!(req_body, body.as_slice());
                // The next pipelined request should now be available for parsing,
                // while we send the (possibly slow) response in another thread
                spawn(move || {
                    req.respond(Response::empty(200).with_data(resp_body, Some(resp_body.len)))
                });
            }
            svr_send.send(()).unwrap();
        });

        spawn(move || req_writer(&mut client));

        // requests must be sent and received quickly (before timeout expires)
        svr_rcv
            .recv_timeout(timeout)
            .expect("Server did not finish reading pipelined requests quickly enough");
    }

    #[test]
    fn empty() {
        assert_requests_parsed_promptly(5, &[], Duration::from_millis(200), move |wr| {
            for _ in 0..5 {
                write!(wr, "GET / HTTP/1.1\r\n").unwrap();
                write!(wr, "Connection: keep-alive\r\n\r\n").unwrap();
            }
        });
    }

    #[test]
    fn content_length_short() {
        let body = &[65u8; 100]; // short but not trivial
        assert_requests_parsed_promptly(5, body, Duration::from_millis(200), move |wr| {
            for _ in 0..5 {
                write!(wr, "GET / HTTP/1.1\r\n").unwrap();
                write!(wr, "Connection: keep-alive\r\n").unwrap();
                write!(wr, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
                wr.write_all(body).unwrap();
            }
        });
    }

    #[test]
    fn content_length_long() {
        let body = &[65u8; 10000]; // long enough that it won't be buffered
        assert_requests_parsed_promptly(5, body, Duration::from_millis(200), move |wr| {
            for _ in 0..5 {
                write!(wr, "GET / HTTP/1.1\r\n").unwrap();
                write!(wr, "Connection: keep-alive\r\n").unwrap();
                write!(wr, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
                wr.write_all(body).unwrap();
            }
        });
    }

    #[test]
    fn chunked() {
        let body = &[65u8; 10000];
        assert_requests_parsed_promptly(5, body, Duration::from_millis(200), move |wr| {
            for _ in 0..5 {
                write!(wr, "GET / HTTP/1.1\r\n").unwrap();
                write!(wr, "Connection: keep-alive\r\n").unwrap();
                write!(wr, "Transfer-Encoding: chunked\r\n\r\n").unwrap();
                encode_chunked(&mut &body[..], wr);
            }
        });
    }
}

mod prompt_responses {
    use super::*;

    /// Check that response is sent promptly without waiting for full request body.
    fn assert_responds_promptly(
        timeout: Duration,
        req_writer: impl FnOnce(&mut dyn Write) + Send + 'static,
    ) {
        let server = Server::http("0.0.0.0:0").unwrap();
        let client = TcpStream::connect(server.server_addr().to_ip().unwrap()).unwrap();

        spawn(move || loop {
            // server attempts to respond immediately
            let req = server.recv().unwrap();
            req.respond(Response::empty(400)).unwrap();
        });

        let client = Arc::new(client);
        let client_write = Arc::clone(&client);
        // request written (possibly very slowly) in another thread
        spawn(move || req_writer(&mut client_write.deref()));

        // response should arrive quickly (before timeout expires)
        client.set_read_timeout(Some(timeout)).unwrap();
        let resp = client.deref().read(&mut [0u8; 4096]);
        client.shutdown(Shutdown::Both).unwrap();
        assert!(resp.is_ok(), "Server response was not sent promptly");
    }

    static SLOW_BODY: SlowByteSrc = SlowByteSrc {
        val: 65,
        len: 1000_000,
    };

    #[test]
    fn content_length_http11() {
        assert_responds_promptly(Duration::from_millis(200), move |wr| {
            write!(wr, "GET / HTTP/1.1\r\n").unwrap();
            write!(wr, "Content-Length: {}\r\n\r\n", SLOW_BODY.len).unwrap();
            copy(&mut SLOW_BODY.clone(), wr).unwrap();
        });
    }

    #[test]
    fn content_length_http10() {
        assert_responds_promptly(Duration::from_millis(200), move |wr| {
            write!(wr, "GET / HTTP/1.0\r\n").unwrap();
            write!(wr, "Content-Length: {}\r\n\r\n", SLOW_BODY.len).unwrap();
            copy(&mut SLOW_BODY.clone(), wr).unwrap();
        });
    }

    #[test]
    fn expect_continue() {
        assert_responds_promptly(Duration::from_millis(200), move |wr| {
            write!(wr, "GET / HTTP/1.1\r\n").unwrap();
            write!(wr, "Expect: 100 continue\r\n").unwrap();
            write!(wr, "Content-Length: {}\r\n\r\n", SLOW_BODY.len).unwrap();
            copy(&mut SLOW_BODY.clone(), wr).unwrap();
        });
    }

    #[test]
    fn chunked() {
        assert_responds_promptly(Duration::from_millis(200), move |wr| {
            write!(wr, "GET / HTTP/1.1\r\n").unwrap();
            write!(wr, "Transfer-Encoding: chunked\r\n\r\n").unwrap();
            encode_chunked(&mut SLOW_BODY.clone(), wr);
        });
    }
}
