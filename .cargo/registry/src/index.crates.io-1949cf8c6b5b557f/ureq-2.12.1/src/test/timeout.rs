use crate::testserver::*;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use std::{
    error::Error,
    io::{self, Write},
};

use super::super::*;

// Send an HTTP response on the TcpStream at a rate of two bytes every 10
// milliseconds, for a total of 600 bytes.
fn dribble_body_respond(mut stream: TcpStream, contents: &[u8]) -> io::Result<()> {
    read_request(&stream);
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
        contents.len() * 2
    );
    stream.write_all(headers.as_bytes())?;
    for i in 0..contents.len() {
        stream.write_all(&contents[i..i + 1])?;
        stream.write_all(&[b'\n'; 1])?;
        stream.flush()?;
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn get_and_expect_timeout(url: String) {
    let timeout = Duration::from_millis(500);
    let agent = builder().timeout(timeout).build();
    let resp = agent.get(&url).call().unwrap();

    match resp.into_string() {
        Err(io_error) => match io_error.kind() {
            io::ErrorKind::TimedOut => Ok(()),
            _ => Err(format!("{:?}", io_error)),
        },
        Ok(_) => Err("successful response".to_string()),
    }
    .expect("expected timeout but got something else");
}

#[test]
fn overall_timeout_during_body() {
    // Start a test server on an available port, that dribbles out a response at 1 write per 10ms.
    let server = TestServer::new(|stream| dribble_body_respond(stream, &[b'a'; 300]));
    let url = format!("http://localhost:{}/", server.port);
    get_and_expect_timeout(url);
}

#[test]
fn read_timeout_during_body() {
    let server = TestServer::new(|stream| dribble_body_respond(stream, &[b'a'; 300]));
    let url = format!("http://localhost:{}/", server.port);
    let agent = builder().timeout_read(Duration::from_millis(70)).build();
    let resp = match agent.get(&url).call() {
        Ok(r) => r,
        Err(e) => panic!("got error during headers, not body: {:?}", e),
    };
    match resp.into_string() {
        Err(io_error) => match io_error.kind() {
            io::ErrorKind::TimedOut => Ok(()),
            _ => Err(format!("{:?}", io_error)),
        },
        Ok(_) => Err("successful response".to_string()),
    }
    .expect("expected timeout but got something else");
}

// Send HTTP headers on the TcpStream at a rate of one header every 100
// milliseconds, for a total of 30 headers.
fn dribble_headers_respond(mut stream: TcpStream) -> io::Result<()> {
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n")?;
    for _ in 0..30 {
        stream.write_all(b"a: b\r\n")?;
        stream.flush()?;
        thread::sleep(Duration::from_millis(100));
    }
    stream.write_all(b"\r\n")?;

    Ok(())
}

#[test]
fn read_timeout_during_headers() {
    let server = TestServer::new(dribble_headers_respond);
    let url = format!("http://localhost:{}/", server.port);
    let agent = builder().timeout_read(Duration::from_millis(10)).build();
    let result = agent.get(&url).call();
    match result {
        Ok(_) => Err("successful response".to_string()),
        Err(e) if e.kind() == ErrorKind::Io => {
            let ioe: Option<&io::Error> = e.source().and_then(|s| s.downcast_ref());
            match ioe {
                Some(e) if e.kind() == io::ErrorKind::TimedOut => Ok(()),
                _ => Err(format!("wrong error type {:?}", e)),
            }
        }
        Err(e) => Err(format!("Unexpected error type: {:?}", e)),
    }
    .expect("expected timeout but got something else");
}

#[test]
fn overall_timeout_during_headers() {
    // Start a test server on an available port, that dribbles out a response at 1 write per 10ms.
    let server = TestServer::new(dribble_headers_respond);
    let url = format!("http://localhost:{}/", server.port);
    let agent = builder().timeout(Duration::from_millis(500)).build();
    let result = agent.get(&url).call();
    match result {
        Ok(_) => Err("successful response".to_string()),
        Err(e) if e.kind() == ErrorKind::Io => {
            let ioe: Option<&io::Error> = e.source().and_then(|s| s.downcast_ref());
            match ioe {
                Some(e) if e.kind() == io::ErrorKind::TimedOut => Ok(()),
                _ => Err(format!("wrong error type {:?}", e)),
            }
        }
        Err(e) => Err(format!("Unexpected error type: {:?}", e)),
    }
    .expect("expected timeout but got something else");
}

#[test]
fn overall_timeout_override_during_headers() {
    // Start a test server on an available port, that dribbles out a response at 1 write per 10ms.
    let server = TestServer::new(dribble_headers_respond);
    let url = format!("http://localhost:{}/", server.port);
    let agent = builder().timeout(Duration::from_secs(90000)).build();
    let result = agent.get(&url).timeout(Duration::from_millis(500)).call();
    match result {
        Ok(_) => Err("successful response".to_string()),
        Err(e) if e.kind() == ErrorKind::Io => {
            let ioe: Option<&io::Error> = e.source().and_then(|s| s.downcast_ref());
            match ioe {
                Some(e) if e.kind() == io::ErrorKind::TimedOut => Ok(()),
                _ => Err(format!("wrong error type {:?}", e)),
            }
        }
        Err(e) => Err(format!("Unexpected error type: {:?}", e)),
    }
    .expect("expected timeout but got something else");
}

#[test]
#[cfg(feature = "json")]
fn overall_timeout_reading_json() {
    // Start a test server on an available port, that dribbles out a response at 1 write per 10ms.
    let server = TestServer::new(|stream| {
        dribble_body_respond(
            stream,
            b"[1,1,1,1,1,1,1,1,1,1,1,1,1,
        1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
        1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
        1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
        1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]",
        )
    });
    let url = format!("http://localhost:{}/", server.port);

    let timeout = Duration::from_millis(500);
    let agent = builder().timeout(timeout).build();
    let result: Result<serde_json::Value, io::Error> = agent.get(&url).call().unwrap().into_json();

    match result {
        Ok(_) => Err("successful response".to_string()),
        Err(e) => match e.kind() {
            io::ErrorKind::TimedOut => Ok(()),
            _ => Err(format!("Unexpected io::ErrorKind: {:?}", e)),
        },
    }
    .expect("expected timeout but got something else");
}
