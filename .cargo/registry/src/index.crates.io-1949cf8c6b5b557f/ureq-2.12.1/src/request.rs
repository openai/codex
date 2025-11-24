use std::io::Read;
use std::{fmt, time};

use url::{form_urlencoded, ParseError, Url};

use crate::agent::Agent;
use crate::body::Payload;
use crate::error::{Error, ErrorKind};
use crate::header::{self, Header};
use crate::middleware::MiddlewareNext;
use crate::unit::{self, Unit};
use crate::Response;

pub type Result<T> = std::result::Result<T, Error>;

/// Request instances are builders that creates a request.
///
/// ```
/// # fn main() -> Result<(), ureq::Error> {
/// # ureq::is_test(true);
/// let response = ureq::get("http://example.com/get")
///     .query("foo", "bar baz")  // add ?foo=bar+baz
///     .call()?;                 // run the request
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
#[must_use = "Requests do nothing until consumed by `call()`"]
pub struct Request {
    agent: Agent,
    method: String,
    url: String,
    pub(crate) headers: Vec<Header>,
    timeout: Option<time::Duration>,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Request({} {}, {:?})",
            self.method, self.url, self.headers
        )
    }
}

impl Request {
    pub(crate) fn new(agent: Agent, method: String, url: String) -> Request {
        Request {
            agent,
            method,
            url,
            headers: vec![],
            timeout: None,
        }
    }

    #[inline(always)]
    /// Sets overall timeout for the request, overriding agent's configuration if any.
    pub fn timeout(mut self, timeout: time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sends the request with no body and blocks the caller until done.
    ///
    /// Use this with GET, HEAD, OPTIONS or TRACE. It sends neither
    /// Content-Length nor Transfer-Encoding.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::get("http://example.com/")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn call(self) -> Result<Response> {
        self.do_call(Payload::Empty)
    }

    fn parse_url(&self) -> Result<Url> {
        Ok(self.url.parse().and_then(|url: Url|
            // No hostname is fine for urls in general, but not for website urls.
            if url.host_str().is_none() {
                Err(ParseError::EmptyHost)
            } else {
                Ok(url)
            }
        )?)
    }

    /// Add Accept-Encoding header with supported values, unless user has
    /// already set this header or is requesting a specific byte-range.
    #[cfg(any(feature = "gzip", feature = "brotli"))]
    fn add_accept_encoding(&mut self) {
        let should_add = !self.headers.iter().map(|h| h.name()).any(|name| {
            name.eq_ignore_ascii_case("accept-encoding") || name.eq_ignore_ascii_case("range")
        });
        if should_add {
            const GZ: bool = cfg!(feature = "gzip");
            const BR: bool = cfg!(feature = "brotli");
            const ACCEPT: &str = match (GZ, BR) {
                (true, true) => "gzip, br",
                (true, false) => "gzip",
                (false, true) => "br",
                (false, false) => "identity", // unreachable due to cfg feature on this fn
            };
            self.headers.push(Header::new("accept-encoding", ACCEPT));
        }
    }

    #[cfg_attr(not(any(feature = "gzip", feature = "brotli")), allow(unused_mut))]
    fn do_call(mut self, payload: Payload) -> Result<Response> {
        for h in &self.headers {
            h.validate()?;
        }

        #[cfg(any(feature = "gzip", feature = "brotli"))]
        self.add_accept_encoding();

        let deadline = match self.timeout.or(self.agent.config.timeout) {
            None => None,
            Some(timeout) => {
                let now = time::Instant::now();
                match now.checked_add(timeout) {
                    Some(dl) => Some(dl),
                    None => {
                        return Err(Error::new(
                            ErrorKind::Io,
                            Some("Request deadline overflowed".to_string()),
                        ))
                    }
                }
            }
        };

        let request_fn = |req: Request| {
            let reader = payload.into_read();
            let url = req.parse_url()?;
            let unit = Unit::new(
                &req.agent,
                &req.method,
                &url,
                req.headers,
                &reader,
                deadline,
            );

            unit::connect(unit, true, reader).map_err(|e| e.url(url))
        };

        let response = if !self.agent.state.middleware.is_empty() {
            // Clone agent to get a local copy with same lifetime as Payload
            let agent = self.agent.clone();
            let chain = &mut agent.state.middleware.iter().map(|mw| mw.as_ref());

            let request_fn = Box::new(request_fn);

            let next = MiddlewareNext { chain, request_fn };

            // // Run middleware chain
            next.handle(self)?
        } else {
            // Run the request_fn without any further indirection.
            request_fn(self)?
        };

        if response.status() >= 400 {
            Err(Error::Status(response.status(), response))
        } else {
            Ok(response)
        }
    }

    /// Send data a json value.
    ///
    /// The `Content-Length` header is implicitly set to the length of the serialized value.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::post("http://httpbin.org/post")
    ///     .send_json(ureq::json!({
    ///       "name": "martin",
    ///       "rust": true,
    ///     }))?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "json")]
    pub fn send_json(mut self, data: impl serde::Serialize) -> Result<Response> {
        if self.header("Content-Type").is_none() {
            self = self.set("Content-Type", "application/json");
        }

        let json_bytes = serde_json::to_vec(&data)
            .expect("Failed to serialize data passed to send_json into JSON");

        self.do_call(Payload::Bytes(&json_bytes))
    }

    /// Send data as bytes.
    ///
    /// The `Content-Length` header is implicitly set to the length of the serialized value.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::put("http://httpbin.org/put")
    ///     .send_bytes(&[0; 1000])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_bytes(self, data: &[u8]) -> Result<Response> {
        self.do_call(Payload::Bytes(data))
    }

    /// Send data as a string.
    ///
    /// The `Content-Length` header is implicitly set to the length of the serialized value.
    /// Defaults to `utf-8`
    ///
    /// ## Charset support
    ///
    /// Requires feature `ureq = { version = "*", features = ["charset"] }`
    ///
    /// If a `Content-Type` header is present and it contains a charset specification, we
    /// attempt to encode the string using that character set. If it fails, we fall back
    /// on utf-8.
    ///
    /// ```
    /// // this example requires features = ["charset"]
    ///
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::post("http://httpbin.org/post")
    ///     .set("Content-Type", "text/plain; charset=iso-8859-1")
    ///     .send_string("Hällo Wörld!")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_string(self, data: &str) -> Result<Response> {
        let charset =
            crate::response::charset_from_content_type(self.header("content-type")).to_string();
        self.do_call(Payload::Text(data, charset))
    }

    /// Send a sequence of (key, value) pairs as form-urlencoded data.
    ///
    /// The `Content-Type` header is implicitly set to application/x-www-form-urlencoded.
    /// The `Content-Length` header is implicitly set to the length of the serialized value.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::post("http://httpbin.org/post")
    ///     .send_form(&[
    ///       ("foo", "bar"),
    ///       ("foo2", "bar2"),
    ///     ])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_form(mut self, data: &[(&str, &str)]) -> Result<Response> {
        if self.header("Content-Type").is_none() {
            self = self.set("Content-Type", "application/x-www-form-urlencoded");
        }
        let encoded = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(data)
            .finish();
        self.do_call(Payload::Bytes(&encoded.into_bytes()))
    }

    /// Send data from a reader.
    ///
    /// If no Content-Length and Transfer-Encoding header has been set, it uses the [chunked transfer encoding](https://tools.ietf.org/html/rfc7230#section-4.1).
    ///
    /// The caller may set the Content-Length header to the expected byte size of the reader if is
    /// known.
    ///
    /// The input from the reader is buffered into chunks of size 16,384, the max size of a TLS fragment.
    ///
    /// ```
    /// use std::io::Cursor;
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let read = Cursor::new(vec![0x20; 100]);
    /// let resp = ureq::post("http://httpbin.org/post")
    ///     .send(read)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send(self, reader: impl Read) -> Result<Response> {
        self.do_call(Payload::Reader(Box::new(reader)))
    }

    /// Set a header field.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::get("http://httpbin.org/bytes/1000")
    ///     .set("Accept", "text/plain")
    ///     .set("Range", "bytes=500-999")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set(mut self, header: &str, value: &str) -> Self {
        header::add_header(&mut self.headers, Header::new(header, value));
        self
    }

    /// Returns the value for a set header.
    ///
    /// ```
    /// let req = ureq::get("/my_page")
    ///     .set("X-API-Key", "foobar");
    /// assert_eq!("foobar", req.header("x-api-Key").unwrap());
    /// ```
    pub fn header(&self, name: &str) -> Option<&str> {
        header::get_header(&self.headers, name)
    }

    /// A list of the set header names in this request. Lowercased to be uniform.
    ///
    /// ```
    /// let req = ureq::get("/my_page")
    ///     .set("X-API-Key", "foobar")
    ///     .set("Content-Type", "application/json");
    /// assert_eq!(req.header_names(), vec!["x-api-key", "content-type"]);
    /// ```
    pub fn header_names(&self) -> Vec<String> {
        self.headers
            .iter()
            .map(|h| h.name().to_ascii_lowercase())
            .collect()
    }

    /// Tells if the header has been set.
    ///
    /// ```
    /// let req = ureq::get("/my_page")
    ///     .set("X-API-Key", "foobar");
    /// assert_eq!(true, req.has("x-api-Key"));
    /// ```
    pub fn has(&self, name: &str) -> bool {
        header::has_header(&self.headers, name)
    }

    /// All headers corresponding values for the give name, or empty vector.
    ///
    /// ```
    /// let req = ureq::get("/my_page")
    ///     .set("X-Forwarded-For", "1.2.3.4")
    ///     .set("X-Forwarded-For", "2.3.4.5");
    ///
    /// assert_eq!(req.all("x-forwarded-for"), vec![
    ///     "1.2.3.4",
    ///     "2.3.4.5",
    /// ]);
    /// ```
    pub fn all(&self, name: &str) -> Vec<&str> {
        header::get_all_headers(&self.headers, name)
    }

    /// Set a query parameter.
    ///
    /// For example, to set `?format=json&dest=/login`
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let resp = ureq::get("http://httpbin.org/get")
    ///     .query("format", "json")
    ///     .query("dest", "/login")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn query(mut self, param: &str, value: &str) -> Self {
        if let Ok(mut url) = self.parse_url() {
            url.query_pairs_mut().append_pair(param, value);

            // replace url
            self.url = url.to_string();
        }
        self
    }

    /// Set multi query parameters.
    ///
    /// For example, to set `?format=json&dest=/login`
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    ///
    /// let query = vec![
    ///     ("format", "json"),
    ///     ("dest", "/login"),
    /// ];
    ///
    /// let resp = ureq::get("http://httpbin.org/get")
    ///     .query_pairs(query)
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn query_pairs<'a, P>(mut self, pairs: P) -> Self
    where
        P: IntoIterator<Item = (&'a str, &'a str)>,
    {
        if let Ok(mut url) = self.parse_url() {
            {
                let mut query_pairs = url.query_pairs_mut();
                for (param, value) in pairs {
                    query_pairs.append_pair(param, value);
                }
            }

            // replace url
            self.url = url.to_string();
        }
        self
    }

    /// Returns the value of the request method. Something like `GET`, `POST`, `PUT` etc.
    ///
    /// ```
    /// let req = ureq::put("http://httpbin.org/put");
    ///
    /// assert_eq!(req.method(), "PUT");
    /// ```
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Get the url str that will be used for this request.
    ///
    /// The url might differ from that originally provided when constructing the
    /// request if additional query parameters have been added using [`Request::query()`].
    ///
    /// In case the original url provided to build the request is not possible to
    /// parse to a Url, this function returns the original, and it will error once the
    /// Request object is used.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let req = ureq::get("http://httpbin.org/get")
    ///     .query("foo", "bar");
    ///
    /// assert_eq!(req.url(), "http://httpbin.org/get?foo=bar");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let req = ureq::get("SO WRONG")
    ///     .query("foo", "bar"); // does nothing
    ///
    /// assert_eq!(req.url(), "SO WRONG");
    /// # Ok(())
    /// # }
    /// ```
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the parsed url that will be used for this request. The parsed url
    /// has functions to inspect the parts of the url further.
    ///
    /// The url might differ from that originally provided when constructing the
    /// request if additional query parameters have been added using [`Request::query()`].
    ///
    /// Returns a `Result` since a common use case is to construct
    /// the [`Request`] using a `&str` in which case the url needs to be parsed
    /// to inspect the parts. If the Request url is not possible to parse, this
    /// function produces the same error that would otherwise happen when
    /// `call` or `send_*` is called.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let req = ureq::get("http://httpbin.org/get")
    ///     .query("foo", "bar");
    ///
    /// assert_eq!(req.request_url()?.host(), "httpbin.org");
    /// # Ok(())
    /// # }
    /// ```
    pub fn request_url(&self) -> Result<RequestUrl> {
        Ok(RequestUrl::new(self.parse_url()?))
    }
}

/// Parsed result of a request url with handy inspection methods.
#[derive(Debug, Clone)]
pub struct RequestUrl {
    url: Url,
    query_pairs: Vec<(String, String)>,
}

impl RequestUrl {
    fn new(url: Url) -> Self {
        // This is needed to avoid url::Url Cow<str>. We want ureq API to work with &str.
        let query_pairs = url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        RequestUrl { url, query_pairs }
    }

    /// Handle the request url as a standard [`url::Url`].
    pub fn as_url(&self) -> &Url {
        &self.url
    }

    /// Get the scheme of the request url, i.e. "https" or "http".
    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    /// Host of the request url.
    pub fn host(&self) -> &str {
        // this unwrap() is ok, because RequestUrl is tested for empty host
        // urls in Request::parse_url().
        self.url.host_str().unwrap()
    }

    /// Port of the request url, if available. Ports are only available if they
    /// are present in the original url. Specifically the scheme default ports,
    /// 443 for `https` and and 80 for `http` are `None` unless explicitly
    /// set in the url, i.e. `https://my-host.com:443/some/path`.
    pub fn port(&self) -> Option<u16> {
        self.url.port()
    }

    /// Path of the request url.
    pub fn path(&self) -> &str {
        self.url.path()
    }

    /// Returns all query parameters as a vector of key-value pairs.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let req = ureq::get("http://httpbin.org/get")
    ///     .query("foo", "42")
    ///     .query("foo", "43");
    ///
    /// assert_eq!(req.request_url()?.query_pairs(), vec![
    ///     ("foo", "42"),
    ///     ("foo", "43")
    /// ]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn query_pairs(&self) -> Vec<(&str, &str)> {
        self.query_pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_implements_send_and_sync() {
        let _request: Box<dyn Send> = Box::new(Request::new(
            Agent::new(),
            "GET".to_string(),
            "https://example.com/".to_string(),
        ));
        let _request: Box<dyn Sync> = Box::new(Request::new(
            Agent::new(),
            "GET".to_string(),
            "https://example.com/".to_string(),
        ));
    }

    #[test]
    fn send_byte_slice() {
        let bytes = vec![1, 2, 3];
        crate::agent()
            .post("http://example.com")
            .send(&bytes[1..2])
            .ok();
    }

    #[test]
    fn disallow_empty_host() {
        let req = crate::agent().get("file:///some/path");

        // Both request_url and call() must surface the same error.
        assert_eq!(
            req.request_url().unwrap_err().kind(),
            crate::ErrorKind::InvalidUrl
        );

        assert_eq!(req.call().unwrap_err().kind(), crate::ErrorKind::InvalidUrl);
    }
}
