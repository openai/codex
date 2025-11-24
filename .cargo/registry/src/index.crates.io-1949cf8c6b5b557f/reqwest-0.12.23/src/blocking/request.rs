use std::convert::TryFrom;
use std::fmt;
use std::time::Duration;

use http::{request::Parts, Request as HttpRequest, Version};
use serde::Serialize;
#[cfg(feature = "json")]
use serde_json;
use serde_urlencoded;

use super::body::{self, Body};
#[cfg(feature = "multipart")]
use super::multipart;
use super::Client;
use crate::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use crate::{async_impl, Method, Url};

/// A request which can be executed with `Client::execute()`.
pub struct Request {
    body: Option<Body>,
    inner: async_impl::Request,
}

/// A builder to construct the properties of a `Request`.
///
/// To construct a `RequestBuilder`, refer to the `Client` documentation.
#[derive(Debug)]
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
            body: None,
            inner: async_impl::Request::new(method, url),
        }
    }

    /// Get the method.
    #[inline]
    pub fn method(&self) -> &Method {
        self.inner.method()
    }

    /// Get a mutable reference to the method.
    #[inline]
    pub fn method_mut(&mut self) -> &mut Method {
        self.inner.method_mut()
    }

    /// Get the url.
    #[inline]
    pub fn url(&self) -> &Url {
        self.inner.url()
    }

    /// Get a mutable reference to the url.
    #[inline]
    pub fn url_mut(&mut self) -> &mut Url {
        self.inner.url_mut()
    }

    /// Get the headers.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Get a mutable reference to the headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.inner.headers_mut()
    }

    /// Get the http version.
    #[inline]
    pub fn version(&self) -> Version {
        self.inner.version()
    }

    /// Get a mutable reference to the http version.
    #[inline]
    pub fn version_mut(&mut self) -> &mut Version {
        self.inner.version_mut()
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
        self.inner.timeout()
    }

    /// Get a mutable reference to the timeout.
    #[inline]
    pub fn timeout_mut(&mut self) -> &mut Option<Duration> {
        self.inner.timeout_mut()
    }

    /// Attempts to clone the `Request`.
    ///
    /// None is returned if a body is which can not be cloned. This can be because the body is a
    /// stream.
    pub fn try_clone(&self) -> Option<Request> {
        let body = if let Some(ref body) = self.body.as_ref() {
            if let Some(body) = body.try_clone() {
                Some(body)
            } else {
                return None;
            }
        } else {
            None
        };
        let mut req = Request::new(self.method().clone(), self.url().clone());
        *req.timeout_mut() = self.timeout().copied();
        *req.headers_mut() = self.headers().clone();
        *req.version_mut() = self.version().clone();
        req.body = body;
        Some(req)
    }

    pub(crate) fn into_async(self) -> (async_impl::Request, Option<body::Sender>) {
        use crate::header::CONTENT_LENGTH;

        let mut req_async = self.inner;
        let body = self.body.and_then(|body| {
            let (tx, body, len) = body.into_async();
            if let Some(len) = len {
                req_async.headers_mut().insert(CONTENT_LENGTH, len.into());
            }
            *req_async.body_mut() = Some(body);
            tx
        });
        (req_async, body)
    }
}

impl RequestBuilder {
    pub(crate) fn new(client: Client, request: crate::Result<Request>) -> RequestBuilder {
        let mut builder = RequestBuilder { client, request };

        let auth = builder
            .request
            .as_mut()
            .ok()
            .and_then(|req| async_impl::request::extract_authority(req.url_mut()));

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
    ///
    /// ```rust
    /// use reqwest::header::USER_AGENT;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.get("https://www.rust-lang.org")
    ///     .header(USER_AGENT, "foo")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn header<K, V>(self, key: K, value: V) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        HeaderValue: TryFrom<V>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.header_sensitive(key, value, false)
    }

    /// Add a `Header` to this Request with ability to define if header_value is sensitive.
    fn header_sensitive<K, V>(mut self, key: K, value: V, sensitive: bool) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        HeaderValue: TryFrom<V>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match <HeaderName as TryFrom<K>>::try_from(key) {
                Ok(key) => match <HeaderValue as TryFrom<V>>::try_from(value) {
                    Ok(mut value) => {
                        // We want to potentially make an unsensitive header
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
    ///
    /// ```rust
    /// use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, CONTENT_TYPE};
    /// # use std::fs;
    ///
    /// fn construct_headers() -> HeaderMap {
    ///     let mut headers = HeaderMap::new();
    ///     headers.insert(USER_AGENT, HeaderValue::from_static("reqwest"));
    ///     headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/png"));
    ///     headers
    /// }
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let file = fs::File::open("much_beauty.png")?;
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .headers(construct_headers())
    ///     .body(file)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn headers(mut self, headers: crate::header::HeaderMap) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            crate::util::replace_headers(req.headers_mut(), headers);
        }
        self
    }

    /// Enable HTTP basic authentication.
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let resp = client.delete("http://httpbin.org/delete")
    ///     .basic_auth("admin", Some("good password"))
    ///     .send()?;
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
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let resp = client.delete("http://httpbin.org/delete")
    ///     .bearer_auth("token")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn bearer_auth<T>(self, token: T) -> RequestBuilder
    where
        T: fmt::Display,
    {
        let header_value = format!("Bearer {token}");
        self.header_sensitive(crate::header::AUTHORIZATION, &*header_value, true)
    }

    /// Set the request body.
    ///
    /// # Examples
    ///
    /// Using a string:
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .body("from a &str!")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Using a `File`:
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let file = std::fs::File::open("from_a_file.txt")?;
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .body(file)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Using arbitrary bytes:
    ///
    /// ```rust
    /// # use std::fs;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// // from bytes!
    /// let bytes: Vec<u8> = vec![1, 10, 100];
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .body(bytes)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Modify the query string of the URL.
    ///
    /// Modifies the URL of this request, adding the parameters provided.
    /// This method appends and does not overwrite. This means that it can
    /// be called multiple times and that existing query parameters are not
    /// overwritten if the same key is used. The key will simply show up
    /// twice in the query string.
    /// Calling `.query(&[("foo", "a"), ("foo", "b")])` gives `"foo=a&foo=b"`.
    ///
    /// ```rust
    /// # use reqwest::Error;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.get("http://httpbin.org")
    ///     .query(&[("lang", "rust")])
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
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
            *req.version_mut() = version;
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
    /// # fn run() -> Result<(), Error> {
    /// let mut params = HashMap::new();
    /// params.insert("lang", "rust");
    ///
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.post("http://httpbin.org")
    ///     .form(&params)
    ///     .send()?;
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
    /// Sets the body to the JSON serialization of the passed value, and
    /// also sets the `Content-Type: application/json` header.
    ///
    /// # Optional
    ///
    /// This requires the optional `json` feature enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use reqwest::Error;
    /// # use std::collections::HashMap;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let mut map = HashMap::new();
    /// map.insert("lang", "rust");
    ///
    /// let client = reqwest::blocking::Client::new();
    /// let res = client.post("http://httpbin.org")
    ///     .json(&map)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Sends a multipart/form-data body.
    ///
    /// ```
    /// # use reqwest::Error;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let form = reqwest::blocking::multipart::Form::new()
    ///     .text("key3", "value3")
    ///     .file("file", "/path/to/field")?;
    ///
    /// let response = client.post("your url")
    ///     .multipart(form)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// See [`multipart`](multipart/) for more examples.
    #[cfg(feature = "multipart")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
    pub fn multipart(self, mut multipart: multipart::Form) -> RequestBuilder {
        let mut builder = self.header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", multipart.boundary()).as_str(),
        );
        if let Ok(ref mut req) = builder.request {
            *req.body_mut() = Some(match multipart.compute_length() {
                Some(length) => Body::sized(multipart.reader(), length),
                None => Body::new(multipart.reader()),
            })
        }
        builder
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

    /// Constructs the Request and sends it the target URL, returning a Response.
    ///
    /// # Errors
    ///
    /// This method fails if there was an error while sending request,
    /// redirect loop was detected or redirect limit was exhausted.
    pub fn send(self) -> crate::Result<super::Response> {
        self.client.execute(self.request?)
    }

    /// Attempts to clone the `RequestBuilder`.
    ///
    /// None is returned if a body is which can not be cloned. This can be because the body is a
    /// stream.
    ///
    /// # Examples
    ///
    /// With a static body
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let builder = client.post("http://httpbin.org/post")
    ///     .body("from a &str!");
    /// let clone = builder.try_clone();
    /// assert!(clone.is_some());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Without a body
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let builder = client.get("http://httpbin.org/get");
    /// let clone = builder.try_clone();
    /// assert!(clone.is_some());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// With a non-cloneable body
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::blocking::Client::new();
    /// let builder = client.get("http://httpbin.org/get")
    ///     .body(reqwest::blocking::Body::new(std::io::empty()));
    /// let clone = builder.try_clone();
    /// assert!(clone.is_none());
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
        let mut inner = async_impl::Request::new(method, url);
        crate::util::replace_headers(inner.headers_mut(), headers);
        Ok(Request {
            body: Some(body.into()),
            inner,
        })
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_request_fields(&mut f.debug_struct("Request"), self).finish()
    }
}

fn fmt_request_fields<'a, 'b>(
    f: &'a mut fmt::DebugStruct<'a, 'b>,
    req: &Request,
) -> &'a mut fmt::DebugStruct<'a, 'b> {
    f.field("method", req.method())
        .field("url", req.url())
        .field("headers", req.headers())
}

#[cfg(test)]
mod tests {
    use super::super::{body, Client};
    use super::{HttpRequest, Request, Version};
    use crate::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, HOST};
    use crate::Method;
    use serde::Serialize;
    #[cfg(feature = "json")]
    use serde_json;
    use serde_urlencoded;
    use std::collections::{BTreeMap, HashMap};
    use std::time::Duration;

    #[test]
    fn basic_get_request() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.get(some_url).build().unwrap();

        assert_eq!(r.method(), &Method::GET);
        assert_eq!(r.url().as_str(), some_url);
    }

    #[test]
    fn basic_head_request() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.head(some_url).build().unwrap();

        assert_eq!(r.method(), &Method::HEAD);
        assert_eq!(r.url().as_str(), some_url);
    }

    #[test]
    fn basic_post_request() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url).build().unwrap();

        assert_eq!(r.method(), &Method::POST);
        assert_eq!(r.url().as_str(), some_url);
    }

    #[test]
    fn basic_put_request() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.put(some_url).build().unwrap();

        assert_eq!(r.method(), &Method::PUT);
        assert_eq!(r.url().as_str(), some_url);
    }

    #[test]
    fn basic_patch_request() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.patch(some_url).build().unwrap();

        assert_eq!(r.method(), &Method::PATCH);
        assert_eq!(r.url().as_str(), some_url);
    }

    #[test]
    fn basic_delete_request() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.delete(some_url).build().unwrap();

        assert_eq!(r.method(), &Method::DELETE);
        assert_eq!(r.url().as_str(), some_url);
    }

    #[test]
    fn add_header() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);

        let header = HeaderValue::from_static("google.com");

        // Add a copy of the header to the request builder
        let r = r.header(HOST, header.clone()).build().unwrap();

        // then check it was actually added
        assert_eq!(r.headers().get(HOST), Some(&header));
    }

    #[test]
    fn add_headers() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);

        let header = HeaderValue::from_static("google.com");

        let mut headers = HeaderMap::new();
        headers.insert(HOST, header);

        // Add a copy of the headers to the request builder
        let r = r.headers(headers.clone()).build().unwrap();

        // then make sure they were added correctly
        assert_eq!(r.headers(), &headers);
    }

    #[test]
    fn add_headers_multi() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);

        let header_json = HeaderValue::from_static("application/json");
        let header_xml = HeaderValue::from_static("application/xml");

        let mut headers = HeaderMap::new();
        headers.append(ACCEPT, header_json);
        headers.append(ACCEPT, header_xml);

        // Add a copy of the headers to the request builder
        let r = r.headers(headers.clone()).build().unwrap();

        // then make sure they were added correctly
        assert_eq!(r.headers(), &headers);
        let mut all_values = r.headers().get_all(ACCEPT).iter();
        assert_eq!(all_values.next().unwrap(), &"application/json");
        assert_eq!(all_values.next().unwrap(), &"application/xml");
        assert_eq!(all_values.next(), None);
    }

    #[test]
    fn add_body() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);

        let body = "Some interesting content";

        let mut r = r.body(body).build().unwrap();

        let buf = body::read_to_string(r.body_mut().take().unwrap()).unwrap();

        assert_eq!(buf, body);
    }

    #[test]
    fn add_query_append() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let mut r = client.get(some_url);

        r = r.query(&[("foo", "bar")]);
        r = r.query(&[("qux", 3)]);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=3"));
    }

    #[test]
    fn add_query_append_same() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let mut r = client.get(some_url);

        r = r.query(&[("foo", "a"), ("foo", "b")]);

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
        let mut r = client.get(some_url);

        let params = Params {
            foo: "bar".into(),
            qux: 3,
        };

        r = r.query(&params);

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
        let mut r = client.get(some_url);

        r = r.query(&params);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=three"));
    }

    #[test]
    fn add_form() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);

        let mut form_data = HashMap::new();
        form_data.insert("foo", "bar");

        let mut r = r.form(&form_data).build().unwrap();

        // Make sure the content type was set
        assert_eq!(
            r.headers().get(CONTENT_TYPE).unwrap(),
            &"application/x-www-form-urlencoded"
        );

        let buf = body::read_to_string(r.body_mut().take().unwrap()).unwrap();

        let body_should_be = serde_urlencoded::to_string(&form_data).unwrap();
        assert_eq!(buf, body_should_be);
    }

    #[test]
    #[cfg(feature = "json")]
    fn add_json() {
        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);

        let mut json_data = HashMap::new();
        json_data.insert("foo", "bar");

        let mut r = r.json(&json_data).build().unwrap();

        // Make sure the content type was set
        assert_eq!(r.headers().get(CONTENT_TYPE).unwrap(), &"application/json");

        let buf = body::read_to_string(r.body_mut().take().unwrap()).unwrap();

        let body_should_be = serde_json::to_string(&json_data).unwrap();
        assert_eq!(buf, body_should_be);
    }

    #[test]
    #[cfg(feature = "json")]
    fn add_json_fail() {
        use serde::ser::Error as _;
        use serde::{Serialize, Serializer};
        use std::error::Error as _;
        struct MyStruct;
        impl Serialize for MyStruct {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                Err(S::Error::custom("nope"))
            }
        }

        let client = Client::new();
        let some_url = "https://google.com/";
        let r = client.post(some_url);
        let json_data = MyStruct;
        let err = r.json(&json_data).build().unwrap_err();
        assert!(err.is_builder()); // well, duh ;)
        assert!(err.source().unwrap().is::<serde_json::Error>());
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
    fn convert_from_http_request() {
        let http_request = HttpRequest::builder()
            .method("GET")
            .uri("http://localhost/")
            .header("User-Agent", "my-awesome-agent/1.0")
            .body("test test test")
            .unwrap();
        let req: Request = Request::try_from(http_request).unwrap();
        assert_eq!(req.body().is_none(), false);
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
        assert_eq!(req.body().is_none(), false);
        let test_data = b"test test test";
        assert_eq!(req.body().unwrap().as_bytes(), Some(&test_data[..]));
        let headers = req.headers();
        assert_eq!(headers.get("User-Agent").unwrap(), "my-awesome-agent/1.0");
        assert_eq!(req.method(), Method::GET);
        assert_eq!(req.url().as_str(), "http://localhost/");
        assert_eq!(req.version(), Version::HTTP_11);
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
        assert_eq!(req.headers()["authorization"].is_sensitive(), true);
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
        assert_eq!(req.headers()["authorization"].is_sensitive(), true);
    }

    #[test]
    fn test_request_cloning() {
        let mut request = Request::new(Method::GET, "https://example.com".try_into().unwrap());
        *request.timeout_mut() = Some(Duration::from_secs(42));
        *request.version_mut() = Version::HTTP_11;

        let clone = request.try_clone().unwrap();
        assert_eq!(request.version(), clone.version());
        assert_eq!(request.headers(), clone.headers());
        assert_eq!(request.timeout(), clone.timeout());
    }
}
