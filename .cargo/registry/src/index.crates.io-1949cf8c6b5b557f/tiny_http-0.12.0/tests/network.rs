extern crate tiny_http;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::thread;
use std::time::Duration;

#[allow(dead_code)]
mod support;

#[test]
fn connection_close_header() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\n")).unwrap();
    thread::sleep(Duration::from_millis(1000));

    (write!(client, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n")).unwrap();

    // if the connection was not closed, this will err with timeout
    // client.set_keepalive(Some(1)).unwrap(); FIXME: reenable this
    let mut out = Vec::new();
    client.read_to_end(&mut out).unwrap();
}

#[test]
fn http_1_0_connection_close() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")).unwrap();

    // if the connection was not closed, this will err with timeout
    // client.set_keepalive(Some(1)).unwrap(); FIXME: reenable this
    let mut out = Vec::new();
    client.read_to_end(&mut out).unwrap();
}

#[test]
fn detect_connection_closed() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\n")).unwrap();
    thread::sleep(Duration::from_millis(1000));

    client.shutdown(Shutdown::Write).unwrap();

    // if the connection was not closed, this will err with timeout
    // client.set_keepalive(Some(1)).unwrap(); FIXME: reenable this
    let mut out = Vec::new();
    client.read_to_end(&mut out).unwrap();
}

#[test]
fn poor_network_test() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "G")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "ET /he")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "llo HT")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "TP/1.")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "1\r\nHo")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "st: localho")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "st\r\nConnec")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "tion: close\r")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (write!(client, "\n\r")).unwrap();
    thread::sleep(Duration::from_millis(100));
    (writeln!(client)).unwrap();

    // client.set_keepalive(Some(2)).unwrap(); FIXME: reenable this
    let mut data = String::new();
    client.read_to_string(&mut data).unwrap();
    assert!(data.ends_with("hello world"));
}

#[test]
fn pipelining_test() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
    (write!(client, "GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
    (write!(
        client,
        "GET /world HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    ))
    .unwrap();

    // client.set_keepalive(Some(2)).unwrap(); FIXME: reenable this
    let mut data = String::new();
    client.read_to_string(&mut data).unwrap();
    assert_eq!(data.split("hello world").count(), 4);
}

#[test]
fn server_crash_results_in_response() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();

    thread::spawn(move || {
        server.recv().unwrap();
        // oops, server crash
    });

    (write!(
        client,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    ))
    .unwrap();

    // client.set_keepalive(Some(2)).unwrap(); FIXME: reenable this
    let mut content = String::new();
    client.read_to_string(&mut content).unwrap();
    assert!(&content[9..].starts_with('5')); // 5xx status code
}

#[test]
fn responses_reordered() {
    let (server, mut client) = support::new_one_server_one_client();

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
    (write!(
        client,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    ))
    .unwrap();

    thread::spawn(move || {
        let rq1 = server.recv().unwrap();
        let rq2 = server.recv().unwrap();

        thread::spawn(move || {
            rq2.respond(tiny_http::Response::from_string(
                "second request".to_owned(),
            ))
            .unwrap();
        });

        thread::sleep(Duration::from_millis(100));

        thread::spawn(move || {
            rq1.respond(tiny_http::Response::from_string("first request".to_owned()))
                .unwrap();
        });
    });

    // client.set_keepalive(Some(2)).unwrap(); FIXME: reenable this
    let mut content = String::new();
    client.read_to_string(&mut content).unwrap();
    assert!(content.ends_with("second request"));
}

#[test]
fn no_transfer_encoding_on_204() {
    let (server, mut client) = support::new_one_server_one_client();

    (write!(
        client,
        "GET / HTTP/1.1\r\nHost: localhost\r\nTE: chunked\r\nConnection: close\r\n\r\n"
    ))
    .unwrap();

    thread::spawn(move || {
        let rq = server.recv().unwrap();

        let resp = tiny_http::Response::empty(tiny_http::StatusCode(204));
        rq.respond(resp).unwrap();
    });

    let mut content = String::new();
    client.read_to_string(&mut content).unwrap();

    assert!(content.starts_with("HTTP/1.1 204"));
    assert!(!content.contains("Transfer-Encoding: chunked"));
}

/* FIXME: uncomment and fix
#[test]
fn connection_timeout() {
    let (server, mut client) = {
        let server = tiny_http::ServerBuilder::new()
            .with_client_connections_timeout(3000)
            .with_random_port().build().unwrap();
        let port = server.server_addr().port();
        let client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        (server, client)
    };

    let (tx_stop, rx_stop) = mpsc::channel();

    // executing server in parallel
    thread::spawn(move || {
        loop {
            server.try_recv();
            thread::sleep(Duration::from_millis(100));
            if rx_stop.try_recv().is_ok() { break }
        }
    });

    // waiting for the 408 response
    let mut content = String::new();
    client.read_to_string(&mut content).unwrap();
    assert!(&content[9..].starts_with("408"));

    // stopping server
    tx_stop.send(());
}
*/

#[test]
fn chunked_threshold() {
    let resp = tiny_http::Response::from_string("test".to_string());
    assert_eq!(resp.chunked_threshold(), 32768);
    assert_eq!(resp.with_chunked_threshold(42).chunked_threshold(), 42);
}
