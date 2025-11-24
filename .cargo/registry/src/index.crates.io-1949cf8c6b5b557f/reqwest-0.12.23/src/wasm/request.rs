use std::convert::TryFrom;
use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use http::{request::Parts, Method, Request as HttpRequest};
use serde::Serialize;
#[cfg(feature = "json")]
use serde_json;
use url::Url;
use web_sys::{RequestCache, RequestCredentials};

use super::{Body, Client, Response};
use crate::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};

/// A request which can be executed with `Client::execute()`.
pub struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Body>,
    timeout: Option<Duration>,
    pub(super) cors: bool,
    pub(super) credentials: Option<RequestCredentials>,
    pub(super) cache: Option<RequestCache>,
}

/// A builder to construct the properties of a `Request`.
pub struct RequestBuilder {
    client: Client,
    request: crate::Result<Request>,
}

impl Request {
    /// Constructs a new request.
    #[inline]
    pub fn new(method: Method, url: Url) -> Self {
        Request {
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
            timeout: None,
            cors: true,
            credentials: None,
            cache: None,
        }
    }

    /// Get the method.
    #[inline]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Get a mutable reference to the method.
    #[inline]
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.method
    }

    /// Get the url.
    #[inline]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Get a mutable reference to the url.
    #[inline]
    pub fn url_mut(&mut self) -> &mut Url {
        &mut self.url
    }

    /// Get the headers.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Get a mutable reference to the headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Get the body.
    #[inline]
    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    /// Get a mutable reference to the body.
    #[inline]
    pub fn body_mut(&mut self) -> &mut Option<Body> {
        &mut self.body
    }

    /// Get the timeout.
    #[inline]
    pub fn timeout(&self) -> Option<&Duration> {
        self.timeout.as_ref()
    }

    /// Get a mutable reference to the timeout.
    #[inline]
    pub fn timeout_mut(&mut self) -> &mut Option<Duration> {
        &mut self.timeout
    }

    /// Attempts to clone the `Request`.
    ///
    /// None is returned if a body is which can not be cloned.
    pub fn try_clone(&self) -> Option<Request> {
        let body = match self.body.as_ref() {
            Some(body) => Some(body.try_clone()?),
            None => None,
        };

        Some(Self {
            method: self.method.clone(),
            url: self.url.clone(),
            headers: self.headers.clone(),
            body,
            timeout: self.timeout,
            cors: self.cors,
            credentials: self.credentials,
            cache: self.cache,
        })
    }
}

impl RequestBuilder {
    pub(super) fn new(client: Client, request: crate::Result<Request>) -> RequestBuilder {
        RequestBuilder { client, request }
    }

    /// Assemble a builder starting from an existing `Client` and a `Request`.
    pub fn from_parts(client: crate::Client, request: crate::Request) -> crate::RequestBuilder {
        crate::RequestBuilder {
            client,
            request: crate::Result::Ok(request),
        }
    }

    /// Modify the query string of the URL.
    ///
    /// Modifies the URL of this request, adding the parameters provided.
    /// This method appends and does not overwrite. This means that it can
    /// be called multiple times and that existing query parameters are not
    /// overwritten if the same key is used. The key will simply show up
    /// twice in the query string.
    /// Calling `.query([("foo", "a"), ("foo", "b")])` gives `"foo=a&foo=b"`.
    ///
    /// # Note
    /// This method does not support serializing a single key-value
    /// pair. Instead of using `.query(("key", "val"))`, use a sequence, such
    /// as `.query(&[("key", "val")])`. It's also possible to serialize structs
    /// and maps into a key-value pair.
    ///
    /// # Errors
    /// This method will fail if the object you provide cannot be serialized
    /// into a query string.
    pub fn query<T: Serialize + ?Sized>(mut self, query: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            let url = req.url_mut();
            let mut pairs = url.query_pairs_mut();
            let serializer = serde_urlencoded::Serializer::new(&mut pairs);

            if let Err(err) = query.serialize(serializer) {
                error = Some(crate::error::builder(err));
            }
        }
        if let Ok(ref mut req) = self.request {
            if let Some("") = req.url().query() {
                req.url_mut().set_query(None);
            }
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Send a form body.
    ///
    /// Sets the body to the url encoded serialization of the passed value,
    /// and also sets the `Content-Type: application/x-www-form-urlencoded`
    /// header.
    ///
    /// # Errors
    ///
    /// This method fails if the passed value cannot be serialized into
    /// url encoded format
    pub fn form<T: Serialize + ?Sized>(mut self, form: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match serde_urlencoded::to_string(form) {
                Ok(body) => {
                    req.headers_mut().insert(
                        CONTENT_TYPE,
                        HeaderValue::from_static("application/x-www-form-urlencoded"),
                    );
                    *req.body_mut() = Some(body.into());
                }
                Err(err) => error = Some(crate::error::builder(err)),
            }
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    /// Set the request json
    pub fn json<T: Serialize + ?Sized>(mut self, json: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match serde_json::to_vec(json) {
                Ok(body) => {
                    req.headers_mut()
                        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    *req.body_mut() = Some(body.into());
                }
                Err(err) => error = Some(crate::error::builder(err)),
            }
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Enable HTTP basic authentication.
    pub fn basic_auth<U, P>(self, username: U, password: Option<P>) -> RequestBuilder
    where
        U: fmt::Display,
        P: fmt::Display,
    {
        let header_value = crate::util::basic_auth(username, password);
        self.header(crate::header::AUTHORIZATION, header_value)
    }

    /// Enable HTTP bearer authentication.
    pub fn bearer_auth<T>(self, token: T) -> RequestBuilder
    where
        T: fmt::Display,
    {
        let header_value = format!("Bearer {token}");
        self.header(crate::header::AUTHORIZATION, header_value)
    }

    /// Set the request body.
    pub fn body<T: Into<Body>>(mut self, body: T) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.body = Some(body.into());
        }
        self
    }

    /// Enables a request timeout.
    pub fn timeout(mut self, timeout: Duration) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.timeout_mut() = Some(timeout);
        }
        self
    }

    /// TODO
    #[cfg(feature = "multipart")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
    pub fn multipart(mut self, multipart: super::multipart::Form) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.body_mut() = Some(Body::from_form(multipart))
        }
        self
    }

    /// Add a `Header` to this Request.
    pub fn header<K, V>(mut self, key: K, value: V) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match <HeaderName as TryFrom<K>>::try_from(key) {
                Ok(key) => match <HeaderValue as TryFrom<V>>::try_from(value) {
                    Ok(value) => {
                        req.headers_mut().append(key, value);
                    }
                    Err(e) => error = Some(crate::error::builder(e.into())),
                },
                Err(e) => error = Some(crate::error::builder(e.into())),
            };
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Add a set of Headers to the existing ones on this Request.
    ///
    /// The headers will be merged in to any already set.
    pub fn headers(mut self, headers: crate::header::HeaderMap) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            crate::util::replace_headers(req.headers_mut(), headers);
        }
        self
    }

    /// Disable CORS on fetching the request.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request mode][mdn] will be set to 'no-cors'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/mode
    pub fn fetch_mode_no_cors(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cors = false;
        }
        self
    }

    /// Set fetch credentials to 'same-origin'
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request credentials][mdn] will be set to 'same-origin'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/credentials
    pub fn fetch_credentials_same_origin(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.credentials = Some(RequestCredentials::SameOrigin);
        }
        self
    }

    /// Set fetch credentials to 'include'
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request credentials][mdn] will be set to 'include'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/credentials
    pub fn fetch_credentials_include(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.credentials = Some(RequestCredentials::Include);
        }
        self
    }

    /// Set fetch credentials to 'omit'
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request credentials][mdn] will be set to 'omit'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/credentials
    pub fn fetch_credentials_omit(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.credentials = Some(RequestCredentials::Omit);
        }
        self
    }

    /// Set fetch cache mode to 'default'.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request cache][mdn] will be set to 'default'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/cache
    pub fn fetch_cache_default(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cache = Some(RequestCache::Default);
        }
        self
    }

    /// Set fetch cache mode to 'no-store'.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request cache][mdn] will be set to 'no-store'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/cache
    pub fn fetch_cache_no_store(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cache = Some(RequestCache::NoStore);
        }
        self
    }

    /// Set fetch cache mode to 'reload'.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request cache][mdn] will be set to 'reload'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/cache
    pub fn fetch_cache_reload(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cache = Some(RequestCache::Reload);
        }
        self
    }

    /// Set fetch cache mode to 'no-cache'.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request cache][mdn] will be set to 'no-cache'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/cache
    pub fn fetch_cache_no_cache(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cache = Some(RequestCache::NoCache);
        }
        self
    }

    /// Set fetch cache mode to 'force-cache'.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request cache][mdn] will be set to 'force-cache'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/cache
    pub fn fetch_cache_force_cache(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cache = Some(RequestCache::ForceCache);
        }
        self
    }

    /// Set fetch cache mode to 'only-if-cached'.
    ///
    /// # WASM
    ///
    /// This option is only effective with WebAssembly target.
    ///
    /// The [request cache][mdn] will be set to 'only-if-cached'.
    ///
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Request/cache
    pub fn fetch_cache_only_if_cached(mut self) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.cache = Some(RequestCache::OnlyIfCached);
        }
        self
    }

    /// Build a `Request`, which can be inspected, modified and executed with
    /// `Client::execute()`.
    pub fn build(self) -> crate::Result<Request> {
        self.request
    }

    /// Build a `Request`, which can be inspected, modified and executed with
    /// `Client::execute()`.
    ///
    /// This is similar to [`RequestBuilder::build()`], but also returns the
    /// embedded `Client`.
    pub fn build_split(self) -> (Client, crate::Result<Request>) {
        (self.client, self.request)
    }

    /// Constructs the Request and sends it to the target URL, returning a
    /// future Response.
    ///
    /// # Errors
    ///
    /// This method fails if there was an error while sending request.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use reqwest::Error;
    /// #
    /// # async fn run() -> Result<(), Error> {
    /// let response = reqwest::Client::new()
    ///     .get("https://hyper.rs")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(self) -> crate::Result<Response> {
        let req = self.request?;
        self.client.execute_request(req).await
    }

    /// Attempt to clone the RequestBuilder.
    ///
    /// `None` is returned if the RequestBuilder can not be cloned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use reqwest::Error;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let client = reqwest::Client::new();
    /// let builder = client.post("http://httpbin.org/post")
    ///     .body("from a &str!");
    /// let clone = builder.try_clone();
    /// assert!(clone.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_clone(&self) -> Option<RequestBuilder> {
        self.request
            .as_ref()
            .ok()
            .and_then(|req| req.try_clone())
            .map(|req| RequestBuilder {
                client: self.client.clone(),
                request: Ok(req),
            })
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_request_fields(&mut f.debug_struct("Request"), self).finish()
    }
}

impl fmt::Debug for RequestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("RequestBuilder");
        match self.request {
            Ok(ref req) => fmt_request_fields(&mut builder, req).finish(),
            Err(ref err) => builder.field("error", err).finish(),
        }
    }
}

fn fmt_request_fields<'a, 'b>(
    f: &'a mut fmt::DebugStruct<'a, 'b>,
    req: &Request,
) -> &'a mut fmt::DebugStruct<'a, 'b> {
    f.field("method", &req.method)
        .field("url", &req.url)
        .field("headers", &req.headers)
}

impl<T> TryFrom<HttpRequest<T>> for Request
where
    T: Into<Body>,
{
    type Error = crate::Error;

    fn try_from(req: HttpRequest<T>) -> crate::Result<Self> {
        let (parts, body) = req.into_parts();
        let Parts {
            method,
            uri,
            headers,
            ..
        } = parts;
        let url = Url::parse(&uri.to_string()).map_err(crate::error::builder)?;
        Ok(Request {
            method,
            url,
            headers,
            body: Some(body.into()),
            timeout: None,
            cors: true,
            credentials: None,
            cache: None,
        })
    }
}

impl TryFrom<Request> for HttpRequest<Body> {
    type Error = crate::Error;

    fn try_from(req: Request) -> crate::Result<Self> {
        let Request {
            method,
            url,
            headers,
            body,
            ..
        } = req;

        let mut req = HttpRequest::builder()
            .method(method)
            .uri(url.as_str())
            .body(body.unwrap_or_else(|| Body::from(Bytes::default())))
            .map_err(crate::error::builder)?;

        *req.headers_mut() = headers;
        Ok(req)
    }
}
