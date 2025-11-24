use std::fmt;

use http::{HeaderMap, Method};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use url::Url;

pub const BODY_PRINT_LIMIT: usize = 10_000;

/// Specifies limitations on printing request bodies when logging requests. For some mock servers
/// the bodies may be too large to reasonably print and it may be desirable to limit them.
#[derive(Debug, Copy, Clone)]
pub enum BodyPrintLimit {
    /// Maximum length of a body to print in bytes.
    Limited(usize),
    /// There is no limit to the size of a body that may be printed.
    Unlimited,
}

/// An incoming request to an instance of [`MockServer`].
///
/// Each matcher gets an immutable reference to a `Request` instance in the [`matches`] method
/// defined in the [`Match`] trait.
///
/// [`MockServer`]: crate::MockServer
/// [`matches`]: crate::Match::matches
/// [`Match`]: crate::Match
///
/// ### Implementation notes:
/// We can't use `http_types::Request` directly in our `Match::matches` signature:
/// it requires having mutable access to the request to extract the body (which gets
/// consumed when read!).
/// It would also require `matches` to be async, which is cumbersome due to the lack of async traits.
///
/// We introduce our `Request` type to perform this extraction once when the request
/// arrives in the mock serve, store the result and pass an immutable reference to it
/// to all our matchers.
#[derive(Debug, Clone)]
pub struct Request {
    pub url: Url,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Request {
    pub fn body_json<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    pub(crate) async fn from_hyper(request: hyper::Request<hyper::body::Incoming>) -> Request {
        let (parts, body) = request.into_parts();
        let url = match parts.uri.authority() {
            Some(_) => parts.uri.to_string(),
            None => format!("http://localhost{}", parts.uri),
        }
        .parse()
        .unwrap();

        let body = body
            .collect()
            .await
            .expect("Failed to read request body.")
            .to_bytes();

        Self {
            url,
            method: parts.method,
            headers: parts.headers,
            body: body.to_vec(),
        }
    }

    pub(crate) fn print_with_limit(
        &self,
        mut buffer: impl fmt::Write,
        body_print_limit: BodyPrintLimit,
    ) -> fmt::Result {
        writeln!(buffer, "{} {}", self.method, self.url)?;
        for name in self.headers.keys() {
            let values = self
                .headers
                .get_all(name)
                .iter()
                .map(|value| String::from_utf8_lossy(value.as_bytes()))
                .collect::<Vec<_>>();
            let values = values.join(",");
            writeln!(buffer, "{}: {}", name, values)?;
        }

        match body_print_limit {
            BodyPrintLimit::Limited(limit) if self.body.len() > limit => {
                let mut written = false;
                for end_byte in limit..(limit + 4).max(self.body.len()) {
                    if let Ok(truncated) = std::str::from_utf8(&self.body[..end_byte]) {
                        written = true;
                        writeln!(buffer, "{}", truncated)?;
                        if end_byte < self.body.len() {
                            writeln!(
                                buffer,
                                "We truncated the body because it was too large: {} bytes (limit: {} bytes)",
                                self.body.len(),
                                limit
                            )?;
                            writeln!(
                                buffer,
                                "Increase this limit by setting `WIREMOCK_BODY_PRINT_LIMIT`, or calling `MockServerBuilder::body_print_limit` when building your MockServer instance"
                            )?;
                        }
                        break;
                    }
                }
                if !written {
                    writeln!(
                        buffer,
                        "Body is likely binary (invalid utf-8) size is {} bytes",
                        self.body.len()
                    )
                } else {
                    Ok(())
                }
            }
            _ => {
                if let Ok(body) = std::str::from_utf8(&self.body) {
                    writeln!(buffer, "{}", body)
                } else {
                    writeln!(
                        buffer,
                        "Body is likely binary (invalid utf-8) size is {} bytes",
                        self.body.len()
                    )
                }
            }
        }
    }
}
