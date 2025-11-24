#![allow(dead_code)]

use crate::error::Error;
use crate::testserver::{read_request, TestServer};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;

use super::super::*;

// Handler that answers with a simple HTTP response, and times
// out idle connections after 2 seconds.
fn idle_timeout_handler(mut stream: TcpStream) -> io::Result<()> {
    read_request(&stream);
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nresponse")?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    Ok(())
}

// Handler that answers with a simple HTTP response, and times
// out idle connections after 2 seconds, sending an HTTP 408 response
fn idle_timeout_handler_408(mut stream: TcpStream) -> io::Result<()> {
    read_request(&stream);
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nresponse")?;
    let twosec = Duration::from_secs(2);
    stream.set_read_timeout(Some(twosec))?;
    thread::sleep(twosec);
    stream.write_all(b"HTTP/1.1 408 Request Timeout\r\nContent-Length: 7\r\n\r\ntimeout")?;
    Ok(())
}

#[test]
fn connection_reuse() {
    let testserver = TestServer::new(idle_timeout_handler);
    let url = format!("http://localhost:{}", testserver.port);
    let agent = Agent::new();
    let resp = agent.get(&url).call().unwrap();

    // use up the connection so it gets returned to the pool
    assert_eq!(resp.status(), 200);
    resp.into_string().unwrap();

    assert!(agent.state.pool.len() > 0);

    // wait for the server to close the connection.
    std::thread::sleep(Duration::from_secs(3));

    // try and make a new request on the pool. this fails
    // when we discover that the TLS connection is dead
    // first when attempting to read from it.
    // Note: This test assumes the second  .call() actually
    // pulls from the pool. If for some reason the timed-out
    // connection wasn't in the pool, we won't be testing what
    // we thought we were testing.
    let resp = agent.get(&url).call().unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
fn connection_reuse_with_408() {
    let testserver = TestServer::new(idle_timeout_handler_408);
    let url = format!("http://localhost:{}", testserver.port);
    let agent = Agent::new();
    let resp = agent.get(&url).call().unwrap();

    // use up the connection so it gets returned to the pool
    assert_eq!(resp.status(), 200);
    resp.into_string().unwrap();

    assert!(agent.state.pool.len() > 0);

    // wait for the server to close the connection.
    std::thread::sleep(Duration::from_secs(3));

    // try and make a new request on the pool. this fails
    // when we discover that the TLS connection is dead
    // first when attempting to read from it.
    // Note: This test assumes the second  .call() actually
    // pulls from the pool. If for some reason the timed-out
    // connection wasn't in the pool, we won't be testing what
    // we thought we were testing.
    let resp = agent.post(&url).send_string("hello").unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
fn custom_resolver() {
    use std::io::Read;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();

    let local_addr = listener.local_addr().unwrap();

    let server = std::thread::spawn(move || {
        let (mut client, _) = listener.accept().unwrap();
        let mut buf = vec![0u8; 16];
        let read = client.read(&mut buf).unwrap();
        buf.truncate(read);
        buf
    });

    AgentBuilder::new()
        .resolver(move |_: &str| Ok(vec![local_addr]))
        .build()
        .get("http://cool.server/")
        .call()
        .ok();

    assert_eq!(&server.join().unwrap(), b"GET / HTTP/1.1\r\n");
}

#[test]
fn socket_addr_fail_over() {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();

    let local_addr = listener.local_addr().unwrap();
    let non_routable_ipv4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 255, 255, 1)), 9872);
    let server = std::thread::spawn(move || {
        let (mut client, _) = listener.accept().unwrap();
        let mut buf = vec![0u8; 16];
        let read = client.read(&mut buf).unwrap();
        buf.truncate(read);
        buf
    });

    AgentBuilder::new()
        .resolver(move |_: &str| Ok(vec![non_routable_ipv4, local_addr]))
        .timeout_connect(Duration::from_secs(2))
        .build()
        .get("http://cool.server/")
        .call()
        .ok();

    assert_eq!(&server.join().unwrap(), b"GET / HTTP/1.1\r\n");
}

#[cfg(feature = "cookies")]
#[cfg(test)]
fn cookie_and_redirect(mut stream: TcpStream) -> io::Result<()> {
    let headers = read_request(&stream);
    match headers.path() {
        "/first" => {
            stream.write_all(b"HTTP/1.1 302 Found\r\n")?;
            stream.write_all(b"Location: /second\r\n")?;
            stream.write_all(b"Set-Cookie: first=true\r\n")?;
            stream.write_all(b"Content-Length: 0\r\n\r\n")?;
        }
        "/second" => {
            if headers
                .headers()
                .iter()
                .find(|&x| x.contains("first=true"))
                .is_none()
            {
                panic!("request did not contain cookie 'first'");
            }
            stream.write_all(b"HTTP/1.1 302 Found\r\n")?;
            stream.write_all(b"Location: /third\r\n")?;
            stream.write_all(b"Set-Cookie: second=true\r\n")?;
            stream.write_all(b"Content-Length: 0\r\n\r\n")?;
        }
        "/third" => {
            if headers
                .headers()
                .iter()
                .find(|&x| x.contains("second=true"))
                .is_none()
            {
                panic!("request did not contain cookie 'second'");
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\n")?;
            stream.write_all(b"Set-Cookie: third=true\r\n")?;
            stream.write_all(b"Content-Length: 0\r\n\r\n")?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(feature = "cookies")]
#[test]
fn test_cookies_on_redirect() -> Result<(), Error> {
    let testserver = TestServer::new(cookie_and_redirect);
    let url = format!("http://localhost:{}/first", testserver.port);
    let agent = Agent::new();
    agent.post(&url).call()?;
    let cookies = agent.state.cookie_tin.get_request_cookies(
        &format!("https://localhost:{}/", testserver.port)
            .parse()
            .unwrap(),
    );
    let mut cookie_names: Vec<String> = cookies.iter().map(|c| c.name().to_string()).collect();
    cookie_names.sort();
    assert_eq!(cookie_names, vec!["first", "second", "third"]);
    Ok(())
}

#[test]
fn dirty_streams_not_returned() -> Result<(), Error> {
    let testserver = TestServer::new(|mut stream: TcpStream| -> io::Result<()> {
        read_request(&stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\n")?;
        stream.write_all(b"Transfer-Encoding: chunked\r\n")?;
        stream.write_all(b"\r\n")?;
        stream.write_all(b"5\r\n")?;
        stream.write_all(b"corgi\r\n")?;
        stream.write_all(b"9\r\n")?;
        stream.write_all(b"dachshund\r\n")?;
        stream.write_all(b"0\r\n")?;
        stream.write_all(b"\r\n")?;
        Ok(())
    });
    let url = format!("http://localhost:{}/", testserver.port);
    let agent = Agent::new();
    let resp = agent.get(&url).call()?;
    let resp_str = resp.into_string()?;
    assert_eq!(resp_str, "corgidachshund");

    // Now fetch it again, but only read part of the body.
    let resp_to_be_dropped = agent.get(&url).call()?;
    let mut reader = resp_to_be_dropped.into_reader();

    // Read 9 bytes of the response and then drop the reader.
    let mut buf = [0_u8; 4];
    let n = reader.read(&mut buf)?;
    assert_ne!(n, 0, "early EOF");
    assert_eq!(&buf, b"corg");
    drop(reader);

    let _resp_to_succeed = agent.get(&url).call()?;
    Ok(())
}
