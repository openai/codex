use std::fmt;
use std::io::{self, Read};
use std::mem;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use http;
use hyper::header::HeaderMap;
#[cfg(feature = "json")]
use serde::de::DeserializeOwned;

use super::client::KeepCoreThreadAlive;
use super::wait;
#[cfg(feature = "cookies")]
use crate::cookie;
use crate::{async_impl, StatusCode, Url, Version};

/// A Response to a submitted `Request`.
pub struct Response {
    inner: async_impl::Response,
    body: Option<Pin<Box<dyn futures_util::io::AsyncRead + Send + Sync>>>,
    timeout: Option<Duration>,
    _thread_handle: KeepCoreThreadAlive,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl Response {
    pub(crate) fn new(
        res: async_impl::Response,
        timeout: Option<Duration>,
        thread: KeepCoreThreadAlive,
    ) -> Response {
        Response {
            inner: res,
            body: None,
            timeout,
            _thread_handle: thread,
        }
    }

    /// Get the `StatusCode` of this `Response`.
    ///
    /// # Examples
    ///
    /// Checking for general status class:
    ///
    /// ```rust
    /// # #[cfg(feature = "json")]
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let resp = reqwest::blocking::get("http://httpbin.org/get")?;
    /// if resp.status().is_success() {
    ///     println!("success!");
    /// } else if resp.status().is_server_error() {
    ///     println!("server error!");
    /// } else {
    ///     println!("Something else happened. Status: {:?}", resp.status());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Checking for specific status codes:
    ///
    /// ```rust
    /// use reqwest::blocking::Client;
    /// use reqwest::StatusCode;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new();
    ///
    /// let resp = client.post("http://httpbin.org/post")
    ///     .body("possibly too large")
    ///     .send()?;
    ///
    /// match resp.status() {
    ///     StatusCode::OK => println!("success!"),
    ///     StatusCode::PAYLOAD_TOO_LARGE => {
    ///         println!("Request payload is too large!");
    ///     }
    ///     s => println!("Received response status: {s:?}"),
    /// };
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Get the `Headers` of this `Response`.
    ///
    /// # Example
    ///
    /// Saving an etag when caching a file:
    ///
    /// ```
    /// use reqwest::blocking::Client;
    /// use reqwest::header::ETAG;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new();
    ///
    /// let mut resp = client.get("http://httpbin.org/cache").send()?;
    /// if resp.status().is_success() {
    ///     if let Some(etag) = resp.headers().get(ETAG) {
    ///         std::fs::write("etag", etag.as_bytes());
    ///     }
    ///     let mut file = std::fs::File::create("file")?;
    ///     resp.copy_to(&mut file)?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Get a mutable reference to the `Headers` of this `Response`.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.inner.headers_mut()
    }

    /// Retrieve the cookies contained in the response.
    ///
    /// Note that invalid 'Set-Cookie' headers will be ignored.
    ///
    /// # Optional
    ///
    /// This requires the optional `cookies` feature to be enabled.
    #[cfg(feature = "cookies")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cookies")))]
    pub fn cookies<'a>(&'a self) -> impl Iterator<Item = cookie::Cookie<'a>> + 'a {
        cookie::extract_response_cookies(self.headers()).filter_map(Result::ok)
    }

    /// Get the HTTP `Version` of this `Response`.
    #[inline]
    pub fn version(&self) -> Version {
        self.inner.version()
    }

    /// Get the final `Url` of this `Response`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let resp = reqwest::blocking::get("http://httpbin.org/redirect/1")?;
    /// assert_eq!(resp.url().as_str(), "http://httpbin.org/get");
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn url(&self) -> &Url {
        self.inner.url()
    }

    /// Get the remote address used to get this `Response`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let resp = reqwest::blocking::get("http://httpbin.org/redirect/1")?;
    /// println!("httpbin.org address: {:?}", resp.remote_addr());
    /// # Ok(())
    /// # }
    /// ```
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.inner.remote_addr()
    }

    /// Returns a reference to the associated extensions.
    pub fn extensions(&self) -> &http::Extensions {
        self.inner.extensions()
    }

    /// Returns a mutable reference to the associated extensions.
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        self.inner.extensions_mut()
    }

    /// Get the content length of the response, if it is known.
    ///
    ///
    /// This value does not directly represents the value of the `Content-Length`
    /// header, but rather the size of the response's body. To read the header's
    /// value, please use the [`Response::headers`] method instead.
    ///
    /// Reasons it may not be known:
    ///
    /// - The response does not include a body (e.g. it responds to a `HEAD`
    ///   request).
    /// - The response is gzipped and automatically decoded (thus changing the
    ///   actual decoded length).
    pub fn content_length(&self) -> Option<u64> {
        self.inner.content_length()
    }

    /// Try and deserialize the response body as JSON using `serde`.
    ///
    /// # Optional
    ///
    /// This requires the optional `json` feature enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate reqwest;
    /// # extern crate serde;
    /// #
    /// # use reqwest::Error;
    /// # use serde::Deserialize;
    /// #
    /// // This `derive` requires the `serde` dependency.
    /// #[derive(Deserialize)]
    /// struct Ip {
    ///     origin: String,
    /// }
    ///
    /// # fn run() -> Result<(), Error> {
    /// let json: Ip = reqwest::blocking::get("http://httpbin.org/ip")?.json()?;
    /// # Ok(())
    /// # }
    /// #
    /// # fn main() { }
    /// ```
    ///
    /// # Errors
    ///
    /// This method fails whenever the response body is not in JSON format,
    /// or it cannot be properly deserialized to target type `T`. For more
    /// details please see [`serde_json::from_reader`].
    ///
    /// [`serde_json::from_reader`]: https://docs.serde.rs/serde_json/fn.from_reader.html
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn json<T: DeserializeOwned>(self) -> crate::Result<T> {
        wait::timeout(self.inner.json(), self.timeout).map_err(|e| match e {
            wait::Waited::TimedOut(e) => crate::error::decode(e),
            wait::Waited::Inner(e) => e,
        })
    }

    /// Get the full response body as `Bytes`.
    ///
    /// # Example
    ///
    /// ```
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let bytes = reqwest::blocking::get("http://httpbin.org/ip")?.bytes()?;
    ///
    /// println!("bytes: {bytes:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub fn bytes(self) -> crate::Result<Bytes> {
        wait::timeout(self.inner.bytes(), self.timeout).map_err(|e| match e {
            wait::Waited::TimedOut(e) => crate::error::decode(e),
            wait::Waited::Inner(e) => e,
        })
    }

    /// Get the response text.
    ///
    /// This method decodes the response body with BOM sniffing
    /// and with malformed sequences replaced with the [`char::REPLACEMENT_CHARACTER`].
    /// Encoding is determined from the `charset` parameter of `Content-Type` header,
    /// and defaults to `utf-8` if not presented.
    ///
    /// # Note
    ///
    /// If the `charset` feature is disabled the method will only attempt to decode the
    /// response as UTF-8, regardless of the given `Content-Type`
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let content = reqwest::blocking::get("http://httpbin.org/range/26")?.text()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn text(self) -> crate::Result<String> {
        wait::timeout(self.inner.text(), self.timeout).map_err(|e| match e {
            wait::Waited::TimedOut(e) => crate::error::decode(e),
            wait::Waited::Inner(e) => e,
        })
    }

    /// Get the response text given a specific encoding.
    ///
    /// This method decodes the response body with BOM sniffing
    /// and with malformed sequences replaced with the [`char::REPLACEMENT_CHARACTER`].
    /// You can provide a default encoding for decoding the raw message, while the
    /// `charset` parameter of `Content-Type` header is still prioritized. For more information
    /// about the possible encoding name, please go to [`encoding_rs`] docs.
    ///
    /// [`encoding_rs`]: https://docs.rs/encoding_rs/0.8/encoding_rs/#relationship-with-windows-code-pages
    ///
    /// # Optional
    ///
    /// This requires the optional `charset` feature enabled.
    ///
    /// # Example
    ///
    /// ```rust
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let content = reqwest::blocking::get("http://httpbin.org/range/26")?
    ///     .text_with_charset("utf-8")?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "charset")]
    #[cfg_attr(docsrs, doc(cfg(feature = "charset")))]
    pub fn text_with_charset(self, default_encoding: &str) -> crate::Result<String> {
        wait::timeout(self.inner.text_with_charset(default_encoding), self.timeout).map_err(|e| {
            match e {
                wait::Waited::TimedOut(e) => crate::error::decode(e),
                wait::Waited::Inner(e) => e,
            }
        })
    }

    /// Copy the response body into a writer.
    ///
    /// This function internally uses [`std::io::copy`] and hence will continuously read data from
    /// the body and then write it into writer in a streaming fashion until EOF is met.
    ///
    /// On success, the total number of bytes that were copied to `writer` is returned.
    ///
    /// [`std::io::copy`]: https://doc.rust-lang.org/std/io/fn.copy.html
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut resp = reqwest::blocking::get("http://httpbin.org/range/5")?;
    /// let mut buf: Vec<u8> = vec![];
    /// resp.copy_to(&mut buf)?;
    /// assert_eq!(b"abcde", buf.as_slice());
    /// # Ok(())
    /// # }
    /// ```
    pub fn copy_to<W: ?Sized>(&mut self, w: &mut W) -> crate::Result<u64>
    where
        W: io::Write,
    {
        io::copy(self, w).map_err(crate::error::decode_io)
    }

    /// Turn a response into an error if the server returned an error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let res = reqwest::blocking::get("http://httpbin.org/status/400")?
    ///     .error_for_status();
    /// if let Err(err) = res {
    ///     assert_eq!(err.status(), Some(reqwest::StatusCode::BAD_REQUEST));
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn error_for_status(self) -> crate::Result<Self> {
        let Response {
            body,
            inner,
            timeout,
            _thread_handle,
        } = self;
        inner.error_for_status().map(move |inner| Response {
            inner,
            body,
            timeout,
            _thread_handle,
        })
    }

    /// Turn a reference to a response into an error if the server returned an error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let res = reqwest::blocking::get("http://httpbin.org/status/400")?;
    /// let res = res.error_for_status_ref();
    /// if let Err(err) = res {
    ///     assert_eq!(err.status(), Some(reqwest::StatusCode::BAD_REQUEST));
    /// }
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn error_for_status_ref(&self) -> crate::Result<&Self> {
        self.inner.error_for_status_ref().and_then(|_| Ok(self))
    }

    // private

    fn body_mut(&mut self) -> Pin<&mut dyn futures_util::io::AsyncRead> {
        use futures_util::TryStreamExt;
        if self.body.is_none() {
            let body = mem::replace(self.inner.body_mut(), async_impl::Decoder::empty());

            let body = body.into_stream().into_async_read();

            self.body = Some(Box::pin(body));
        }
        self.body.as_mut().expect("body was init").as_mut()
    }
}

impl Read for Response {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use futures_util::io::AsyncReadExt;

        let timeout = self.timeout;
        wait::timeout(self.body_mut().read(buf), timeout).map_err(|e| match e {
            wait::Waited::TimedOut(e) => crate::error::decode(e).into_io(),
            wait::Waited::Inner(e) => e,
        })
    }
}

impl<T: Into<async_impl::body::Body>> From<http::Response<T>> for Response {
    fn from(r: http::Response<T>) -> Response {
        let response = async_impl::Response::from(r);
        Response::new(response, None, KeepCoreThreadAlive::empty())
    }
}
