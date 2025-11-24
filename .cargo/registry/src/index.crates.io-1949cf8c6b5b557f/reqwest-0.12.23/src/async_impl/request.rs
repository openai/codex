use std::convert::TryFrom;
use std::fmt;
use std::future::Future;
use std::time::Duration;

use serde::Serialize;
#[cfg(feature = "json")]
use serde_json;

use super::body::Body;
use super::client::{Client, Pending};
#[cfg(feature = "multipart")]
use super::multipart;
use super::response::Response;
use crate::config::{RequestConfig, RequestTimeout};
#[cfg(feature = "multipart")]
use crate::header::CONTENT_LENGTH;
use crate::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use crate::{Method, Url};
use http::{request::Parts, Extensions, Request as HttpRequest, Version};

/// A request which can be executed with `Client::execute()`.
pub struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Body>,
    version: Version,
    extensions: Extensions,
}

/// A builder to construct the properties of a `Request`.
///
/// To construct a `RequestBuilder`, refer to the `Client` documentation.
#[must_use = "RequestBuilder does nothing until you 'send' it"]
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
            version: Version::default(),
            extensions: Extensions::new(),
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

    /// Get the extensions.
    #[inline]
    pub(crate) fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Get a mutable reference to the extensions.
    #[inline]
    pub(crate) fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Get the timeout.
    #[inline]
    pub fn timeout(&self) -> Option<&Duration> {
        RequestConfig::<RequestTimeout>::get(&self.extensions)
    }

    /// Get a mutable reference to the timeout.
    #[inline]
    pub fn timeout_mut(&mut self) -> &mut Option<Duration> {
        RequestConfig::<RequestTimeout>::get_mut(&mut self.extensions)
    }

    /// Get the http version.
    #[inline]
    pub fn version(&self) -> Version {
        self.version
    }

    /// Get a mutable reference to the http version.
    #[inline]
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
    }

    /// Attempt to clone the request.
    ///
    /// `None` is returned if the request can not be cloned, i.e. if the body is a stream.
    pub fn try_clone(&self) -> Option<Request> {
        let body = match self.body.as_ref() {
            Some(body) => Some(body.try_clone()?),
            None => None,
        };
        let mut req = Request::new(self.method().clone(), self.url().clone());
        *req.timeout_mut() = self.timeout().copied();
        *req.headers_mut() = self.headers().clone();
        *req.version_mut() = self.version();
        *req.extensions_mut() = self.extensions().clone();
        req.body = body;
        Some(req)
    }

    pub(super) fn pieces(self) -> (Method, Url, HeaderMap, Option<Body>, Version, Extensions) {
        (
            self.method,
            self.url,
            self.headers,
            self.body,
            self.version,
            self.extensions,
        )
    }
}

impl RequestBuilder {
    pub(super) fn new(client: Client, request: crate::Result<Request>) -> RequestBuilder {
        let mut builder = RequestBuilder { client, request };

        let auth = builder
            .request
            .as_mut()
            .ok()
            .and_then(|req| extract_authority(&mut req.url));

        if let Some((username, password)) = auth {
            builder.basic_auth(username, password)
        } else {
            builder
        }
    }

    /// Assemble a builder starting from an existing `Client` and a `Request`.
    pub fn from_parts(client: Client, request: Request) -> RequestBuilder {
        RequestBuilder {
            client,
            request: crate::Result::Ok(request),
        }
    }

    /// Add a `Header` to this Request.
    pub fn header<K, V>(self, key: K, value: V) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.header_sensitive(key, value, false)
    }

    /// Add a `Header` to this Request with ability to define if `header_value` is sensitive.
    fn header_sensitive<K, V>(mut self, key: K, value: V, sensitive: bool) -> RequestBuilder
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
                    Ok(mut value) => {
                        // We want to potentially make an non-sensitive header
                        // to be sensitive, not the reverse. So, don't turn off
                        // a previously sensitive header.
                        if sensitive {
                            value.set_sensitive(true);
                        }
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

    /// Enable HTTP basic authentication.
    ///
    /// ```rust
    /// # use reqwest::Error;
    ///
    /// # async fn run() -> Result<(), Error> {
    /// let client = reqwest::Client::new();
    /// let resp = client.delete("http://httpbin.org/delete")
    ///     .basic_auth("admin", Some("good password"))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn basic_auth<U, P>(self, username: U, password: Option<P>) -> RequestBuilder
    where
        U: fmt::Display,
        P: fmt::Display,
    {
        let header_value = crate::util::basic_auth(username, password);
        self.header_sensitive(crate::header::AUTHORIZATION, header_value, true)
    }

    /// Enable HTTP bearer authentication.
    pub fn bearer_auth<T>(self, token: T) -> RequestBuilder
    where
        T: fmt::Display,
    {
        let header_value = format!("Bearer {token}");
        self.header_sensitive(crate::header::AUTHORIZATION, header_value, true)
    }

    /// Set the request body.
    pub fn body<T: Into<Body>>(mut self, body: T) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.body_mut() = Some(body.into());
        }
        self
    }

    /// Enables a request timeout.
    ///
    /// The timeout is applied from when the request starts connecting until the
    /// response body has finished. It affects only this request and overrides
    /// the timeout configured using `ClientBuilder::timeout()`.
    pub fn timeout(mut self, timeout: Duration) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.timeout_mut() = Some(timeout);
        }
        self
    }

    /// Sends a multipart/form-data body.
    ///
    /// ```
    /// # use reqwest::Error;
    ///
    /// # async fn run() -> Result<(), Error> {
    /// let client = reqwest::Client::new();
    /// let form = reqwest::multipart::Form::new()
    ///     .text("key3", "value3")
    ///     .text("key4", "value4");
    ///
    ///
    /// let response = client.post("your url")
    ///     .multipart(form)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// In additional the request's body, the Content-Type and Content-Length fields are
    /// appropriately set.
    #[cfg(feature = "multipart")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
    pub fn multipart(self, mut multipart: multipart::Form) -> RequestBuilder {
        let mut builder = self.header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", multipart.boundary()).as_str(),
        );

        builder = match multipart.compute_length() {
            Some(length) => builder.header(CONTENT_LENGTH, length),
            None => builder,
        };

        if let Ok(ref mut req) = builder.request {
            *req.body_mut() = Some(multipart.stream())
        }
        builder
    }

    /// Modify the query string of the URL.
    ///
    /// Modifies the URL of this request, adding the parameters provided.
    /// This method appends and does not overwrite. This means that it can
    /// be called multiple times and that existing query parameters are not
    /// overwritten if the same key is used. The key will simply show up
    /// twice in the query string.
    /// Calling `.query(&[("foo", "a"), ("foo", "b")])` gives `"foo=a&foo=b"`.
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

    /// Set HTTP version
    pub fn version(mut self, version: Version) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.version = version;
        }
        self
    }

    /// Send a form body.
    ///
    /// Sets the body to the url encoded serialization of the passed value,
    /// and also sets the `Content-Type: application/x-www-form-urlencoded`
    /// header.
    ///
    /// ```rust
    /// # use reqwest::Error;
    /// # use std::collections::HashMap;
    /// #
    /// # async fn run() -> Result<(), Error> {
    /// let mut params = HashMap::new();
    /// params.insert("lang", "rust");
    ///
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org")
    ///     .form(&params)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
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
                    req.headers_mut()
                        .entry(CONTENT_TYPE)
                        .or_insert(HeaderValue::from_static(
                            "application/x-www-form-urlencoded",
                        ));
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

    /// Send a JSON body.
    ///
    /// # Optional
    ///
    /// This requires the optional `json` feature enabled.
    ///
    /// # Errors
    ///
    /// Serialization can fail if `T`'s implementation of `Serialize` decides to
    /// fail, or if `T` contains a map with non-string keys.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn json<T: Serialize + ?Sized>(mut self, json: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match serde_json::to_vec(json) {
                Ok(body) => {
                    if !req.headers().contains_key(CONTENT_TYPE) {
                        req.headers_mut()
                            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    }
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

    // This was a shell only meant to help with rendered documentation.
    // However, docs.rs can now show the docs for the wasm platforms, so this
    // is no longer needed.
    //
    // You should not otherwise depend on this function. It's deprecation
    // is just to nudge people to reduce breakage. It may be removed in a
    // future patch version.
    #[doc(hidden)]
    #[cfg_attr(target_arch = "wasm32", deprecated)]
    pub fn fetch_mode_no_cors(self) -> RequestBuilder {
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
    /// This method fails if there was an error while sending request,
    /// redirect loop was detected or redirect limit was exhausted.
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
    pub fn send(self) -> impl Future<Output = Result<Response, crate::Error>> {
        match self.request {
            Ok(req) => self.client.execute_request(req),
            Err(err) => Pending::new_err(err),
        }
    }

    /// Attempt to clone the RequestBuilder.
    ///
    /// `None` is returned if the RequestBuilder can not be cloned,
    /// i.e. if the request body is a stream.
    ///
    /// # Examples
    ///
    /// ```
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

/// Check the request URL for a "username:password" type authority, and if
/// found, remove it from the URL and return it.
pub(crate) fn extract_authority(url: &mut Url) -> Option<(String, Option<String>)> {
    use percent_encoding::percent_decode;

    if url.has_authority() {
        let username: String = percent_decode(url.username().as_bytes())
            .decode_utf8()
            .ok()?
            .into();
        let password = url.password().and_then(|pass| {
            percent_decode(pass.as_bytes())
                .decode_utf8()
                .ok()
                .map(String::from)
        });
        if !username.is_empty() || password.is_some() {
            url.set_username("")
                .expect("has_authority means set_username shouldn't fail");
            url.set_password(None)
                .expect("has_authority means set_password shouldn't fail");
            return Some((username, password));
        }
    }

    None
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
            version,
            extensions,
            ..
        } = parts;
        let url = Url::parse(&uri.to_string()).map_err(crate::error::builder)?;
        Ok(Request {
            method,
            url,
            headers,
            body: Some(body.into()),
            version,
            extensions,
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
            version,
            extensions,
            ..
        } = req;

        let mut req = HttpRequest::builder()
            .version(version)
            .method(method)
            .uri(url.as_str())
            .body(body.unwrap_or_else(Body::empty))
            .map_err(crate::error::builder)?;

        *req.headers_mut() = headers;
        *req.extensions_mut() = extensions;
        Ok(req)
    }
}

#[cfg(test)]
mod tests {
    #![cfg(not(feature = "rustls-tls-manual-roots-no-provider"))]

    use super::{Client, HttpRequest, Request, RequestBuilder, Version};
    use crate::Method;
    use serde::Serialize;
    use std::collections::BTreeMap;
    use std::convert::TryFrom;

    #[test]
    fn add_query_append() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.get(some_url);

        let r = r.query(&[("foo", "bar")]);
        let r = r.query(&[("qux", 3)]);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=3"));
    }

    #[test]
    fn add_query_append_same() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.get(some_url);

        let r = r.query(&[("foo", "a"), ("foo", "b")]);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=a&foo=b"));
    }

    #[test]
    fn add_query_struct() {
        #[derive(Serialize)]
        struct Params {
            foo: String,
            qux: i32,
        }

        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.get(some_url);

        let params = Params {
            foo: "bar".into(),
            qux: 3,
        };

        let r = r.query(&params);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=3"));
    }

    #[test]
    fn add_query_map() {
        let mut params = BTreeMap::new();
        params.insert("foo", "bar");
        params.insert("qux", "three");

        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.get(some_url);

        let r = r.query(&params);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=three"));
    }

    #[test]
    fn test_replace_headers() {
        use http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert("foo", "bar".parse().unwrap());
        headers.append("foo", "baz".parse().unwrap());

        let client = Client::new();
        let req = client
            .get("https://hyper.rs")
            .header("im-a", "keeper")
            .header("foo", "pop me")
            .headers(headers)
            .build()
            .expect("request build");

        assert_eq!(req.headers()["im-a"], "keeper");

        let foo = req.headers().get_all("foo").iter().collect::<Vec<_>>();
        assert_eq!(foo.len(), 2);
        assert_eq!(foo[0], "bar");
        assert_eq!(foo[1], "baz");
    }

    #[test]
    fn normalize_empty_query() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let empty_query: &[(&str, &str)] = &[];

        let req = client
            .get(some_url)
            .query(empty_query)
            .build()
            .expect("request build");

        assert_eq!(req.url().query(), None);
        assert_eq!(req.url().as_str(), "https://google.com/");
    }

    #[test]
    fn try_clone_reusable() {
        let client = Client::new();
        let builder = client
            .post("http://httpbin.org/post")
            .header("foo", "bar")
            .body("from a &str!");
        let req = builder
            .try_clone()
            .expect("clone successful")
            .build()
            .expect("request is valid");
        assert_eq!(req.url().as_str(), "http://httpbin.org/post");
        assert_eq!(req.method(), Method::POST);
        assert_eq!(req.headers()["foo"], "bar");
    }

    #[test]
    fn try_clone_no_body() {
        let client = Client::new();
        let builder = client.get("http://httpbin.org/get");
        let req = builder
            .try_clone()
            .expect("clone successful")
            .build()
            .expect("request is valid");
        assert_eq!(req.url().as_str(), "http://httpbin.org/get");
        assert_eq!(req.method(), Method::GET);
        assert!(req.body().is_none());
    }

    #[test]
    #[cfg(feature = "stream")]
    fn try_clone_stream() {
        let chunks: Vec<Result<_, ::std::io::Error>> = vec![Ok("hello"), Ok(" "), Ok("world")];
        let stream = futures_util::stream::iter(chunks);
        let client = Client::new();
        let builder = client
            .get("http://httpbin.org/get")
            .body(super::Body::wrap_stream(stream));
        let clone = builder.try_clone();
        assert!(clone.is_none());
    }

    #[test]
    fn convert_url_authority_into_basic_auth() {
        let client = Client::new();
        let some_url = "https://Aladdin:open sesame@localhost/";

        let req = client.get(some_url).build().expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(
            req.headers()["authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
    }

    #[test]
    fn test_basic_auth_sensitive_header() {
        let client = Client::new();
        let some_url = "https://localhost/";

        let req = client
            .get(some_url)
            .basic_auth("Aladdin", Some("open sesame"))
            .build()
            .expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(
            req.headers()["authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        assert!(req.headers()["authorization"].is_sensitive());
    }

    #[test]
    fn test_bearer_auth_sensitive_header() {
        let client = Client::new();
        let some_url = "https://localhost/";

        let req = client
            .get(some_url)
            .bearer_auth("Hold my bear")
            .build()
            .expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(req.headers()["authorization"], "Bearer Hold my bear");
        assert!(req.headers()["authorization"].is_sensitive());
    }

    #[test]
    fn test_explicit_sensitive_header() {
        let client = Client::new();
        let some_url = "https://localhost/";

        let mut header = http::HeaderValue::from_static("in plain sight");
        header.set_sensitive(true);

        let req = client
            .get(some_url)
            .header("hiding", header)
            .build()
            .expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(req.headers()["hiding"], "in plain sight");
        assert!(req.headers()["hiding"].is_sensitive());
    }

    #[test]
    fn convert_from_http_request() {
        let http_request = HttpRequest::builder()
            .method("GET")
            .uri("http://localhost/")
            .header("User-Agent", "my-awesome-agent/1.0")
            .body("test test test")
            .unwrap();
        let req: Request = Request::try_from(http_request).unwrap();
        assert!(req.body().is_some());
        let test_data = b"test test test";
        assert_eq!(req.body().unwrap().as_bytes(), Some(&test_data[..]));
        let headers = req.headers();
        assert_eq!(headers.get("User-Agent").unwrap(), "my-awesome-agent/1.0");
        assert_eq!(req.method(), Method::GET);
        assert_eq!(req.url().as_str(), "http://localhost/");
    }

    #[test]
    fn set_http_request_version() {
        let http_request = HttpRequest::builder()
            .method("GET")
            .uri("http://localhost/")
            .header("User-Agent", "my-awesome-agent/1.0")
            .version(Version::HTTP_11)
            .body("test test test")
            .unwrap();
        let req: Request = Request::try_from(http_request).unwrap();
        assert!(req.body().is_some());
        let test_data = b"test test test";
        assert_eq!(req.body().unwrap().as_bytes(), Some(&test_data[..]));
        let headers = req.headers();
        assert_eq!(headers.get("User-Agent").unwrap(), "my-awesome-agent/1.0");
        assert_eq!(req.method(), Method::GET);
        assert_eq!(req.url().as_str(), "http://localhost/");
        assert_eq!(req.version(), Version::HTTP_11);
    }

    #[test]
    fn builder_split_reassemble() {
        let builder = {
            let client = Client::new();
            client.get("http://example.com")
        };
        let (client, inner) = builder.build_split();
        let request = inner.unwrap();
        let builder = RequestBuilder::from_parts(client, request);
        builder.build().unwrap();
    }

    /*
    use {body, Method};
    use super::Client;
    use header::{Host, Headers, ContentType};
    use std::collections::HashMap;
    use serde_urlencoded;
    use serde_json;

    #[test]
    fn basic_get_request() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let r = client.get(some_url).unwrap().build();

        assert_eq!(r.method, Method::Get);
        assert_eq!(r.url.as_str(), some_url);
    }

    #[test]
    fn basic_head_request() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let r = client.head(some_url).unwrap().build();

        assert_eq!(r.method, Method::Head);
        assert_eq!(r.url.as_str(), some_url);
    }

    #[test]
    fn basic_post_request() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let r = client.post(some_url).unwrap().build();

        assert_eq!(r.method, Method::Post);
        assert_eq!(r.url.as_str(), some_url);
    }

    #[test]
    fn basic_put_request() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let r = client.put(some_url).unwrap().build();

        assert_eq!(r.method, Method::Put);
        assert_eq!(r.url.as_str(), some_url);
    }

    #[test]
    fn basic_patch_request() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let r = client.patch(some_url).unwrap().build();

        assert_eq!(r.method, Method::Patch);
        assert_eq!(r.url.as_str(), some_url);
    }

    #[test]
    fn basic_delete_request() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let r = client.delete(some_url).unwrap().build();

        assert_eq!(r.method, Method::Delete);
        assert_eq!(r.url.as_str(), some_url);
    }

    #[test]
    fn add_header() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();

        let header = Host {
            hostname: "google.com".to_string(),
            port: None,
        };

        // Add a copy of the header to the request builder
        let r = r.header(header.clone()).build();

        // then check it was actually added
        assert_eq!(r.headers.get::<Host>(), Some(&header));
    }

    #[test]
    fn add_headers() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();

        let header = Host {
            hostname: "google.com".to_string(),
            port: None,
        };

        let mut headers = Headers::new();
        headers.set(header);

        // Add a copy of the headers to the request builder
        let r = r.headers(headers.clone()).build();

        // then make sure they were added correctly
        assert_eq!(r.headers, headers);
    }

    #[test]
    fn add_headers_multi() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();

        let header = Host {
            hostname: "google.com".to_string(),
            port: None,
        };

        let mut headers = Headers::new();
        headers.set(header);

        // Add a copy of the headers to the request builder
        let r = r.headers(headers.clone()).build();

        // then make sure they were added correctly
        assert_eq!(r.headers, headers);
    }

    #[test]
    fn add_body() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();

        let body = "Some interesting content";

        let r = r.body(body).build();

        let buf = body::read_to_string(r.body.unwrap()).unwrap();

        assert_eq!(buf, body);
    }

    #[test]
    fn add_form() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();

        let mut form_data = HashMap::new();
        form_data.insert("foo", "bar");

        let r = r.form(&form_data).unwrap().build();

        // Make sure the content type was set
        assert_eq!(r.headers.get::<ContentType>(),
                   Some(&ContentType::form_url_encoded()));

        let buf = body::read_to_string(r.body.unwrap()).unwrap();

        let body_should_be = serde_urlencoded::to_string(&form_data).unwrap();
        assert_eq!(buf, body_should_be);
    }

    #[test]
    fn add_json() {
        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();

        let mut json_data = HashMap::new();
        json_data.insert("foo", "bar");

        let r = r.json(&json_data).unwrap().build();

        // Make sure the content type was set
        assert_eq!(r.headers.get::<ContentType>(), Some(&ContentType::json()));

        let buf = body::read_to_string(r.body.unwrap()).unwrap();

        let body_should_be = serde_json::to_string(&json_data).unwrap();
        assert_eq!(buf, body_should_be);
    }

    #[test]
    fn add_json_fail() {
        use serde::{Serialize, Serializer};
        use serde::ser::Error;
        struct MyStruct;
        impl Serialize for MyStruct {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer
                {
                    Err(S::Error::custom("nope"))
                }
        }

        let client = Client::new().unwrap();
        let some_url = "https://google.com/";
        let mut r = client.post(some_url).unwrap();
        let json_data = MyStruct{};
        assert!(r.json(&json_data).unwrap_err().is_serialization());
    }
    */
}
