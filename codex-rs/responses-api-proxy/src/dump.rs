use std::fs;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use reqwest::header::HeaderMap;
use serde::Serialize;
use serde_json::Value;
use tiny_http::Header;
use tiny_http::Method;

const AUTHORIZATION_HEADER_NAME: &str = "authorization";
const REDACTED_HEADER_VALUE: &str = "[REDACTED]";

pub(crate) struct ExchangeDumper {
    dump_dir: PathBuf,
    next_sequence: AtomicU64,
}

impl ExchangeDumper {
    pub(crate) fn new(dump_dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dump_dir)?;

        Ok(Self {
            dump_dir,
            next_sequence: AtomicU64::new(1),
        })
    }

    pub(crate) fn dump_request(
        &self,
        method: &Method,
        url: &str,
        headers: &[Header],
        body: &[u8],
    ) -> io::Result<ExchangeDump> {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let prefix = format!("{sequence:06}-{timestamp_ms}");

        let request_path = self.dump_dir.join(format!("{prefix}-request.json"));
        let response_path = self.dump_dir.join(format!("{prefix}-response.json"));

        let request_dump = RequestDump {
            method: method.as_str().to_string(),
            url: url.to_string(),
            headers: headers.iter().map(HeaderDump::from).collect(),
            body: dump_body(body),
        };

        write_json_dump(&request_path, &request_dump)?;

        Ok(ExchangeDump { response_path })
    }
}

pub(crate) struct ExchangeDump {
    response_path: PathBuf,
}

impl ExchangeDump {
    pub(crate) fn tee_response_body<R: Read>(
        self,
        status: u16,
        headers: &HeaderMap,
        response_body: R,
    ) -> ResponseBodyDump<R> {
        ResponseBodyDump {
            response_body,
            response_path: self.response_path,
            status,
            headers: headers.iter().map(HeaderDump::from).collect(),
            body: Vec::new(),
            dump_written: false,
        }
    }
}

pub(crate) struct ResponseBodyDump<R> {
    response_body: R,
    response_path: PathBuf,
    status: u16,
    headers: Vec<HeaderDump>,
    body: Vec<u8>,
    dump_written: bool,
}

impl<R> ResponseBodyDump<R> {
    fn write_dump_if_needed(&mut self) {
        if self.dump_written {
            return;
        }

        self.dump_written = true;

        let response_dump = ResponseDump {
            status: self.status,
            headers: std::mem::take(&mut self.headers),
            body: dump_body(&self.body),
        };

        if let Err(err) = write_json_dump(&self.response_path, &response_dump) {
            eprintln!(
                "responses-api-proxy failed to write {}: {err}",
                self.response_path.display()
            );
        }
    }
}

impl<R: Read> Read for ResponseBodyDump<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.response_body.read(buf)?;
        if bytes_read == 0 {
            self.write_dump_if_needed();
            return Ok(0);
        }

        self.body.extend_from_slice(&buf[..bytes_read]);
        Ok(bytes_read)
    }
}

impl<R> Drop for ResponseBodyDump<R> {
    fn drop(&mut self) {
        self.write_dump_if_needed();
    }
}

#[derive(Serialize)]
struct RequestDump {
    method: String,
    url: String,
    headers: Vec<HeaderDump>,
    body: Value,
}

#[derive(Serialize)]
struct ResponseDump {
    status: u16,
    headers: Vec<HeaderDump>,
    body: Value,
}

#[derive(Debug, Serialize)]
struct HeaderDump {
    name: String,
    value: String,
}

impl From<&Header> for HeaderDump {
    fn from(header: &Header) -> Self {
        let name = header.field.as_str().to_string();
        let value = if name.eq_ignore_ascii_case(AUTHORIZATION_HEADER_NAME) {
            REDACTED_HEADER_VALUE.to_string()
        } else {
            header.value.as_str().to_string()
        };

        Self { name, value }
    }
}

impl From<(&reqwest::header::HeaderName, &reqwest::header::HeaderValue)> for HeaderDump {
    fn from(header: (&reqwest::header::HeaderName, &reqwest::header::HeaderValue)) -> Self {
        let name = header.0.as_str();
        let value = if name.eq_ignore_ascii_case(AUTHORIZATION_HEADER_NAME) {
            REDACTED_HEADER_VALUE.to_string()
        } else {
            String::from_utf8_lossy(header.1.as_bytes()).into_owned()
        };

        Self {
            name: name.to_string(),
            value,
        }
    }
}

fn dump_body(body: &[u8]) -> Value {
    serde_json::from_slice(body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body).into_owned()))
}

fn write_json_dump(path: &PathBuf, dump: &impl Serialize) -> io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(dump)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    bytes.push(b'\n');
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::io::Read;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    use pretty_assertions::assert_eq;
    use reqwest::header::AUTHORIZATION;
    use reqwest::header::CONTENT_TYPE;
    use reqwest::header::HeaderMap;
    use reqwest::header::HeaderValue;
    use serde_json::json;
    use tiny_http::Header;
    use tiny_http::Method;

    use super::ExchangeDumper;

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn dump_request_writes_redacted_headers_and_json_body() {
        let dump_dir = test_dump_dir();
        let dumper = ExchangeDumper::new(dump_dir.clone()).expect("create dumper");
        let headers = vec![
            Header::from_bytes(&b"Authorization"[..], &b"Bearer secret"[..])
                .expect("authorization header"),
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                .expect("content-type header"),
        ];

        let exchange_dump = dumper
            .dump_request(
                &Method::Post,
                "/v1/responses",
                &headers,
                br#"{"model":"gpt-5.4"}"#,
            )
            .expect("dump request");

        let request_dump = fs::read_to_string(dump_file_with_suffix(&dump_dir, "-request.json"))
            .expect("read request dump");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request_dump).expect("parse request dump"),
            json!({
                "method": "POST",
                "url": "/v1/responses",
                "headers": [
                    {
                        "name": "Authorization",
                        "value": "[REDACTED]"
                    },
                    {
                        "name": "Content-Type",
                        "value": "application/json"
                    }
                ],
                "body": {
                    "model": "gpt-5.4"
                }
            })
        );
        assert!(
            exchange_dump
                .response_path
                .file_name()
                .expect("response dump file name")
                .to_string_lossy()
                .ends_with("-response.json")
        );

        fs::remove_dir_all(dump_dir).expect("remove test dump dir");
    }

    #[test]
    fn response_body_dump_streams_body_and_writes_response_file() {
        let dump_dir = test_dump_dir();
        let dumper = ExchangeDumper::new(dump_dir.clone()).expect("create dumper");
        let exchange_dump = dumper
            .dump_request(&Method::Post, "/v1/responses", &[], b"{}")
            .expect("dump request");

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));

        let mut response_body = String::new();
        exchange_dump
            .tee_response_body(200, &headers, Cursor::new(b"data: hello\n\n".to_vec()))
            .read_to_string(&mut response_body)
            .expect("read response body");

        let response_dump = fs::read_to_string(dump_file_with_suffix(&dump_dir, "-response.json"))
            .expect("read response dump");

        assert_eq!(response_body, "data: hello\n\n");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response_dump).expect("parse response dump"),
            json!({
                "status": 200,
                "headers": [
                    {
                        "name": "content-type",
                        "value": "text/event-stream"
                    },
                    {
                        "name": "authorization",
                        "value": "[REDACTED]"
                    }
                ],
                "body": "data: hello\n\n"
            })
        );

        fs::remove_dir_all(dump_dir).expect("remove test dump dir");
    }

    fn test_dump_dir() -> std::path::PathBuf {
        let test_id = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let dump_dir = std::env::temp_dir().join(format!(
            "codex-responses-api-proxy-dump-test-{}-{test_id}",
            std::process::id()
        ));
        fs::create_dir_all(&dump_dir).expect("create test dump dir");
        dump_dir
    }

    fn dump_file_with_suffix(dump_dir: &std::path::Path, suffix: &str) -> std::path::PathBuf {
        let mut matches = fs::read_dir(dump_dir)
            .expect("read dump dir")
            .map(|entry| entry.expect("read dump entry").path())
            .filter(|path| path.to_string_lossy().ends_with(suffix))
            .collect::<Vec<_>>();
        matches.sort();

        assert_eq!(matches.len(), 1);
        matches.pop().expect("single dump file")
    }
}
