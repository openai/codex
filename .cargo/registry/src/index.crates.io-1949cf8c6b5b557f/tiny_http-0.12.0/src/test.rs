use crate::{request::new_request, HTTPVersion, Header, HeaderField, Method, Request};
use ascii::AsciiString;
use std::net::SocketAddr;
use std::str::FromStr;

/// A simpler version of [`Request`] that is useful for testing. No data actually goes anywhere.
///
/// By default, `TestRequest` pretends to be an insecure GET request for the server root (`/`)
/// with no headers. To create a `TestRequest` with different parameters, use the builder pattern:
///
/// ```
/// # use tiny_http::{Method, TestRequest};
/// let request = TestRequest::new()
///     .with_method(Method::Post)
///     .with_path("/api/widgets")
///     .with_body("42");
/// ```
///
/// Then, convert the `TestRequest` into a real `Request` and pass it to the server under test:
///
/// ```
/// # use tiny_http::{Method, Request, Response, Server, StatusCode, TestRequest};
/// # use std::io::Cursor;
/// # let request = TestRequest::new()
/// #     .with_method(Method::Post)
/// #     .with_path("/api/widgets")
/// #     .with_body("42");
/// # struct TestServer {
/// #     listener: Server,
/// # }
/// # let server = TestServer {
/// #     listener: Server::http("0.0.0.0:0").unwrap(),
/// # };
/// # impl TestServer {
/// #     fn handle_request(&self, request: Request) -> Response<Cursor<Vec<u8>>> {
/// #         Response::from_string("test")
/// #     }
/// # }
/// let response = server.handle_request(request.into());
/// assert_eq!(response.status_code(), StatusCode(200));
/// ```
pub struct TestRequest {
    body: &'static str,
    remote_addr: SocketAddr,
    // true if HTTPS, false if HTTP
    secure: bool,
    method: Method,
    path: String,
    http_version: HTTPVersion,
    headers: Vec<Header>,
}

impl From<TestRequest> for Request {
    fn from(mut mock: TestRequest) -> Request {
        // if the user didn't set the Content-Length header, then set it for them
        // otherwise, leave it alone (it may be under test)
        if !mock
            .headers
            .iter_mut()
            .any(|h| h.field.equiv("Content-Length"))
        {
            mock.headers.push(Header {
                field: HeaderField::from_str("Content-Length").unwrap(),
                value: AsciiString::from_ascii(mock.body.len().to_string()).unwrap(),
            });
        }
        new_request(
            mock.secure,
            mock.method,
            mock.path,
            mock.http_version,
            mock.headers,
            Some(mock.remote_addr),
            mock.body.as_bytes(),
            std::io::sink(),
        )
        .unwrap()
    }
}

impl Default for TestRequest {
    fn default() -> Self {
        TestRequest {
            body: "",
            remote_addr: "127.0.0.1:23456".parse().unwrap(),
            secure: false,
            method: Method::Get,
            path: "/".to_string(),
            http_version: HTTPVersion::from((1, 1)),
            headers: Vec::new(),
        }
    }
}

impl TestRequest {
    pub fn new() -> Self {
        TestRequest::default()
    }
    pub fn with_body(mut self, body: &'static str) -> Self {
        self.body = body;
        self
    }
    pub fn with_remote_addr(mut self, remote_addr: SocketAddr) -> Self {
        self.remote_addr = remote_addr;
        self
    }
    pub fn with_https(mut self) -> Self {
        self.secure = true;
        self
    }
    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }
    pub fn with_path(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }
    pub fn with_http_version(mut self, version: HTTPVersion) -> Self {
        self.http_version = version;
        self
    }
    pub fn with_header(mut self, header: Header) -> Self {
        self.headers.push(header);
        self
    }
}
