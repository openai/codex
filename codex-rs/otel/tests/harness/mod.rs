use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;

#[derive(Debug)]
pub(crate) struct CapturedRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) body: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct ParsedEnvelope {
    pub(crate) header: Value,
    pub(crate) item_header: Value,
    pub(crate) payload: String,
}

#[derive(Debug)]
pub(crate) struct ParsedStatsdLine {
    pub(crate) name: String,
    pub(crate) value: i64,
    pub(crate) kind: String,
    pub(crate) tags: BTreeMap<String, String>,
}

/// Spawn a simple HTTP server that captures one request and responds with `status`.
pub(crate) fn spawn_server(status: u16) -> (String, thread::JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let dsn = format!("http://public:@{addr}/123");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let request = read_http_request(&mut stream);
        let reason = match status {
            200 => "OK",
            500 => "Internal Server Error",
            _ => "OK",
        };
        let response =
            format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        request
    });

    (dsn, handle)
}

// Read a single HTTP request from the stream and return the parsed data.
fn read_http_request(stream: &mut TcpStream) -> CapturedRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let mut header_end = None;
    while header_end.is_none() {
        let read = stream.read(&mut chunk).expect("read request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = find_header_end(&buffer);
    }
    let header_end = header_end.expect("request headers");
    let headers_bytes = &buffer[..header_end];
    let headers_str = std::str::from_utf8(headers_bytes).expect("headers utf-8");
    let mut lines = headers_str.split("\r\n");
    let request_line = lines.next().expect("request line");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().expect("method").to_string();
    let path = request_parts.next().expect("path").to_string();

    let mut headers = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).expect("read body");
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }

    CapturedRequest {
        method,
        path,
        headers,
        body,
    }
}

// Locate the end of the HTTP headers in a buffered request.
fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Parse a Sentry envelope payload into headers and statsd payload text.
pub(crate) fn parse_envelope(body: &[u8]) -> ParsedEnvelope {
    let mut parts = body.splitn(3, |byte| *byte == b'\n');
    let header_line = parts.next().expect("envelope header");
    let item_header_line = parts.next().expect("item header");
    let payload = parts.next().unwrap_or(&[]);

    let header = serde_json::from_slice(header_line).expect("parse envelope header");
    let item_header = serde_json::from_slice(item_header_line).expect("parse item header");
    let payload = std::str::from_utf8(payload)
        .expect("payload utf-8")
        .trim_end_matches('\n')
        .to_string();

    ParsedEnvelope {
        header,
        item_header,
        payload,
    }
}

/// Parse a single statsd line (with optional tags) into components.
pub(crate) fn parse_statsd_line(line: &str) -> ParsedStatsdLine {
    let (metric, tags_part) = line
        .split_once("|#")
        .map(|(metric, tags)| (metric, Some(tags)))
        .unwrap_or((line, None));
    let (name_value, kind) = metric.split_once('|').expect("metric kind");
    let (name, value) = name_value.split_once(':').expect("metric value");
    let value = value.parse::<i64>().expect("metric value parse");

    let mut tags = BTreeMap::new();
    if let Some(tags_part) = tags_part
        && !tags_part.is_empty()
    {
        for tag in tags_part.split(',') {
            let (key, value) = tag.split_once(':').expect("tag");
            tags.insert(key.to_string(), value.to_string());
        }
    }

    ParsedStatsdLine {
        name: name.to_string(),
        value,
        kind: kind.to_string(),
        tags,
    }
}
