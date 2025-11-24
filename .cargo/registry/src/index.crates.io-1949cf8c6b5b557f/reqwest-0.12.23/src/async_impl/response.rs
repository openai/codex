use std::fmt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{HeaderMap, StatusCode, Version};
use hyper_util::client::legacy::connect::HttpInfo;
#[cfg(feature = "json")]
use serde::de::DeserializeOwned;
#[cfg(feature = "json")]
use serde_json;
use tokio::time::Sleep;
use url::Url;

use super::body::Body;
use super::decoder::{Accepts, Decoder};
use crate::async_impl::body::ResponseBody;
#[cfg(feature = "cookies")]
use crate::cookie;

#[cfg(feature = "charset")]
use encoding_rs::{Encoding, UTF_8};
#[cfg(feature = "charset")]
use mime::Mime;

/// A Response to a submitted `Request`.
pub struct Response {
    pub(super) res: hyper::Response<Decoder>,
    // Boxed to save space (11 words to 1 word), and it's not accessed
    // frequently internally.
    url: Box<Url>,
}

impl Response {
    pub(super) fn new(
        res: hyper::Response<ResponseBody>,
        url: Url,
        accepts: Accepts,
        total_timeout: Option<Pin<Box<Sleep>>>,
        read_timeout: Option<Duration>,
    ) -> Response {
        let (mut parts, body) = res.into_parts();
        let decoder = Decoder::detect(
            &mut parts.headers,
            super::body::response(body, total_timeout, read_timeout),
            accepts,
        );
        let res = hyper::Response::from_parts(parts, decoder);

        Response {
            res,
            url: Box::new(url),
        }
    }

    /// Get the `StatusCode` of this `Response`.
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.res.status()
    }

    /// Get the HTTP `Version` of this `Response`.
    #[inline]
    pub fn version(&self) -> Version {
        self.res.version()
    }

    /// Get the `Headers` of this `Response`.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        self.res.headers()
    }

    /// Get a mutable reference to the `Headers` of this `Response`.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.res.headers_mut()
    }

    /// Get the content length of the response, if it is known.
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
        use hyper::body::Body;

        Body::size_hint(self.res.body()).exact()
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
        cookie::extract_response_cookies(self.res.headers()).filter_map(Result::ok)
    }

    /// Get the final `Url` of this `Response`.
    #[inline]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Get the remote address used to get this `Response`.
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.res
            .extensions()
            .get::<HttpInfo>()
            .map(|info| info.remote_addr())
    }

    /// Returns a reference to the associated extensions.
    pub fn extensions(&self) -> &http::Extensions {
        self.res.extensions()
    }

    /// Returns a mutable reference to the associated extensions.
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        self.res.extensions_mut()
    }

    // body methods

    /// Get the full response text.
    ///
    /// This method decodes the response body with BOM sniffing
    /// and with malformed sequences replaced with the
    /// [`char::REPLACEMENT_CHARACTER`].
    /// Encoding is determined from the `charset` parameter of `Content-Type` header,
    /// and defaults to `utf-8` if not presented.
    ///
    /// Note that the BOM is stripped from the returned String.
    ///
    /// # Note
    ///
    /// If the `charset` feature is disabled the method will only attempt to decode the
    /// response as UTF-8, regardless of the given `Content-Type`
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let content = reqwest::get("http://httpbin.org/range/26")
    ///     .await?
    ///     .text()
    ///     .await?;
    ///
    /// println!("text: {content:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn text(self) -> crate::Result<String> {
        #[cfg(feature = "charset")]
        {
            self.text_with_charset("utf-8").await
        }

        #[cfg(not(feature = "charset"))]
        {
            let full = self.bytes().await?;
            let text = String::from_utf8_lossy(&full);
            Ok(text.into_owned())
        }
    }

    /// Get the full response text given a specific encoding.
    ///
    /// This method decodes the response body with BOM sniffing
    /// and with malformed sequences replaced with the [`char::REPLACEMENT_CHARACTER`].
    /// You can provide a default encoding for decoding the raw message, while the
    /// `charset` parameter of `Content-Type` header is still prioritized. For more information
    /// about the possible encoding name, please go to [`encoding_rs`] docs.
    ///
    /// Note that the BOM is stripped from the returned String.
    ///
    /// [`encoding_rs`]: https://docs.rs/encoding_rs/0.8/encoding_rs/#relationship-with-windows-code-pages
    ///
    /// # Optional
    ///
    /// This requires the optional `encoding_rs` feature enabled.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let content = reqwest::get("http://httpbin.org/range/26")
    ///     .await?
    ///     .text_with_charset("utf-8")
    ///     .await?;
    ///
    /// println!("text: {content:?}");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "charset")]
    #[cfg_attr(docsrs, doc(cfg(feature = "charset")))]
    pub async fn text_with_charset(self, default_encoding: &str) -> crate::Result<String> {
        let content_type = self
            .headers()
            .get(crate::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        let encoding_name = content_type
            .as_ref()
            .and_then(|mime| mime.get_param("charset").map(|charset| charset.as_str()))
            .unwrap_or(default_encoding);
        let encoding = Encoding::for_label(encoding_name.as_bytes()).unwrap_or(UTF_8);

        let full = self.bytes().await?;

        let (text, _, _) = encoding.decode(&full);
        Ok(text.into_owned())
    }

    /// Try to deserialize the response body as JSON.
    ///
    /// # Optional
    ///
    /// This requires the optional `json` feature enabled.
    ///
    /// # Examples
    ///
    /// ```
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
    /// # async fn run() -> Result<(), Error> {
    /// let ip = reqwest::get("http://httpbin.org/ip")
    ///     .await?
    ///     .json::<Ip>()
    ///     .await?;
    ///
    /// println!("ip: {}", ip.origin);
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
    pub async fn json<T: DeserializeOwned>(self) -> crate::Result<T> {
        let full = self.bytes().await?;

        serde_json::from_slice(&full).map_err(crate::error::decode)
    }

    /// Get the full response body as `Bytes`.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let bytes = reqwest::get("http://httpbin.org/ip")
    ///     .await?
    ///     .bytes()
    ///     .await?;
    ///
    /// println!("bytes: {bytes:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bytes(self) -> crate::Result<Bytes> {
        use http_body_util::BodyExt;

        BodyExt::collect(self.res.into_body())
            .await
            .map(|buf| buf.to_bytes())
    }

    /// Stream a chunk of the response body.
    ///
    /// When the response body has been exhausted, this will return `None`.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut res = reqwest::get("https://hyper.rs").await?;
    ///
    /// while let Some(chunk) = res.chunk().await? {
    ///     println!("Chunk: {chunk:?}");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn chunk(&mut self) -> crate::Result<Option<Bytes>> {
        use http_body_util::BodyExt;

        // loop to ignore unrecognized frames
        loop {
            if let Some(res) = self.res.body_mut().frame().await {
                let frame = res?;
                if let Ok(buf) = frame.into_data() {
                    return Ok(Some(buf));
                }
                // else continue
            } else {
                return Ok(None);
            }
        }
    }

    /// Convert the response into a `Stream` of `Bytes` from the body.
    ///
    /// # Example
    ///
    /// ```
    /// use futures_util::StreamExt;
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut stream = reqwest::get("http://httpbin.org/ip")
    ///     .await?
    ///     .bytes_stream();
    ///
    /// while let Some(item) = stream.next().await {
    ///     println!("Chunk: {:?}", item?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the optional `stream` feature to be enabled.
    #[cfg(feature = "stream")]
    #[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
    pub fn bytes_stream(self) -> impl futures_core::Stream<Item = crate::Result<Bytes>> {
        super::body::DataStream(self.res.into_body())
    }

    // util methods

    /// Turn a response into an error if the server returned an error.
    ///
    /// # Example
    ///
    /// ```
    /// # use reqwest::Response;
    /// fn on_response(res: Response) {
    ///     match res.error_for_status() {
    ///         Ok(_res) => (),
    ///         Err(err) => {
    ///             // asserting a 400 as an example
    ///             // it could be any status between 400...599
    ///             assert_eq!(
    ///                 err.status(),
    ///                 Some(reqwest::StatusCode::BAD_REQUEST)
    ///             );
    ///         }
    ///     }
    /// }
    /// # fn main() {}
    /// ```
    pub fn error_for_status(self) -> crate::Result<Self> {
        let status = self.status();
        let reason = self.extensions().get::<hyper::ext::ReasonPhrase>().cloned();
        if status.is_client_error() || status.is_server_error() {
            Err(crate::error::status_code(*self.url, status, reason))
        } else {
            Ok(self)
        }
    }

    /// Turn a reference to a response into an error if the server returned an error.
    ///
    /// # Example
    ///
    /// ```
    /// # use reqwest::Response;
    /// fn on_response(res: &Response) {
    ///     match res.error_for_status_ref() {
    ///         Ok(_res) => (),
    ///         Err(err) => {
    ///             // asserting a 400 as an example
    ///             // it could be any status between 400...599
    ///             assert_eq!(
    ///                 err.status(),
    ///                 Some(reqwest::StatusCode::BAD_REQUEST)
    ///             );
    ///         }
    ///     }
    /// }
    /// # fn main() {}
    /// ```
    pub fn error_for_status_ref(&self) -> crate::Result<&Self> {
        let status = self.status();
        let reason = self.extensions().get::<hyper::ext::ReasonPhrase>().cloned();
        if status.is_client_error() || status.is_server_error() {
            Err(crate::error::status_code(*self.url.clone(), status, reason))
        } else {
            Ok(self)
        }
    }

    // private

    // The Response's body is an implementation detail.
    // You no longer need to get a reference to it, there are async methods
    // on the `Response` itself.
    //
    // This method is just used by the blocking API.
    #[cfg(feature = "blocking")]
    pub(crate) fn body_mut(&mut self) -> &mut Decoder {
        self.res.body_mut()
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Response")
            .field("url", &self.url().as_str())
            .field("status", &self.status())
            .field("headers", self.headers())
            .finish()
    }
}

/// A `Response` can be piped as the `Body` of another request.
impl From<Response> for Body {
    fn from(r: Response) -> Body {
        Body::wrap(r.res.into_body())
    }
}

// I'm not sure this conversion is that useful... People should be encouraged
// to use `http::Response`, not `reqwest::Response`.
impl<T: Into<Body>> From<http::Response<T>> for Response {
    fn from(r: http::Response<T>) -> Response {
        use crate::response::ResponseUrl;

        let (mut parts, body) = r.into_parts();
        let body: crate::async_impl::body::Body = body.into();
        let decoder = Decoder::detect(
            &mut parts.headers,
            ResponseBody::new(body.map_err(Into::into)),
            Accepts::none(),
        );
        let url = parts
            .extensions
            .remove::<ResponseUrl>()
            .unwrap_or_else(|| ResponseUrl(Url::parse("http://no.url.provided.local").unwrap()));
        let url = url.0;
        let res = hyper::Response::from_parts(parts, decoder);
        Response {
            res,
            url: Box::new(url),
        }
    }
}

/// A `Response` can be converted into a `http::Response`.
// It's supposed to be the inverse of the conversion above.
impl From<Response> for http::Response<Body> {
    fn from(r: Response) -> http::Response<Body> {
        let (parts, body) = r.res.into_parts();
        let body = Body::wrap(body);
        http::Response::from_parts(parts, body)
    }
}

#[cfg(test)]
mod tests {
    use super::Response;
    use crate::ResponseBuilderExt;
    use http::response::Builder;
    use url::Url;

    #[test]
    fn test_from_http_response() {
        let url = Url::parse("http://example.com").unwrap();
        let response = Builder::new()
            .status(200)
            .url(url.clone())
            .body("foo")
            .unwrap();
        let response = Response::from(response);

        assert_eq!(response.status(), 200);
        assert_eq!(*response.url(), url);
    }
}
