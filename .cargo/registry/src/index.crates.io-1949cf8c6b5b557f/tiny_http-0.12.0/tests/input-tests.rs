extern crate tiny_http;

use std::io::{Read, Write};
use std::net::Shutdown;
use std::sync::mpsc;
use std::thread;

#[allow(dead_code)]
mod support;

#[test]
fn basic_string_input() {
    let (server, client) = support::new_one_server_one_client();

    {
        let mut client = client;
        (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 5\r\n\r\nhello")).unwrap();
    }

    let mut request = server.recv().unwrap();

    let mut output = String::new();
    request.as_reader().read_to_string(&mut output).unwrap();
    assert_eq!(output, "hello");
}

#[test]
fn wrong_content_length() {
    let (server, client) = support::new_one_server_one_client();

    {
        let mut client = client;
        (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 3\r\n\r\nhello")).unwrap();
    }

    let mut request = server.recv().unwrap();

    let mut output = String::new();
    request.as_reader().read_to_string(&mut output).unwrap();
    assert_eq!(output, "hel");
}

#[test]
fn expect_100_continue() {
    let (server, client) = support::new_one_server_one_client();

    let mut client = client;
    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nExpect: 100-continue\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 5\r\n\r\n")).unwrap();
    client.flush().unwrap();

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut request = server.recv().unwrap();
        let mut output = String::new();
        request.as_reader().read_to_string(&mut output).unwrap();
        assert_eq!(output, "hello");
        tx.send(()).unwrap();
    });

    // client.set_keepalive(Some(3)).unwrap(); FIXME: reenable this
    let mut content = vec![0; 12];
    client.read_exact(&mut content).unwrap();
    assert!(&content[9..].starts_with(b"100")); // 100 status code

    (write!(client, "hello")).unwrap();
    client.flush().unwrap();
    client.shutdown(Shutdown::Write).unwrap();

    rx.recv().unwrap();
}

#[test]
fn unsupported_expect_header() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nExpect: 189-dummy\r\nContent-Type: text/plain; charset=utf8\r\n\r\n")).unwrap();

    // client.set_keepalive(Some(3)).unwrap(); FIXME: reenable this
    let mut content = String::new();
    client.read_to_string(&mut content).unwrap();
    assert!(&content[9..].starts_with("417")); // 417 status code
}

#[test]
fn invalid_header_name() {
    let mut client = support::new_client_to_hello_world_server();

    // note the space hidden in the Content-Length, which is invalid
    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length : 5\r\n\r\nhello")).unwrap();

    let mut content = String::new();
    client.read_to_string(&mut content).unwrap();
    assert!(&content[9..].starts_with("400 Bad Request")); // 400 status code
}

#[test]
fn custom_content_type_response_header() {
    let (server, mut stream) = support::new_one_server_one_client();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();
    request
        .respond(
            tiny_http::Response::from_string("{\"custom\": \"Content-Type\"}").with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            ),
        )
        .unwrap();

    let mut content = String::new();
    stream.read_to_string(&mut content).unwrap();

    assert!(content.ends_with("{\"custom\": \"Content-Type\"}"));
    assert_ne!(content.find("Content-Type: application/json"), None);
}
