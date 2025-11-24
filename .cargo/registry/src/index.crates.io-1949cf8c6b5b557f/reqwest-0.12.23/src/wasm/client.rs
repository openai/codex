use http::header::USER_AGENT;
use http::{HeaderMap, HeaderValue, Method};
use js_sys::{Promise, JSON};
use std::convert::TryInto;
use std::{fmt, future::Future, sync::Arc};
use url::Url;
use wasm_bindgen::prelude::{wasm_bindgen, UnwrapThrowExt as _};

use super::{AbortGuard, Request, RequestBuilder, Response};
use crate::IntoUrl;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = fetch)]
    fn fetch_with_request(input: &web_sys::Request) -> Promise;
}

fn js_fetch(req: &web_sys::Request) -> Promise {
    use wasm_bindgen::{JsCast, JsValue};
    let global = js_sys::global();

    if let Ok(true) = js_sys::Reflect::has(&global, &JsValue::from_str("ServiceWorkerGlobalScope"))
    {
        global
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_request(req)
    } else {
        // browser
        fetch_with_request(req)
    }
}

/// dox
#[derive(Clone)]
pub struct Client {
    config: Arc<Config>,
}

/// dox
pub struct ClientBuilder {
    config: Config,
}

impl Client {
    /// dox
    pub fn new() -> Self {
        Client::builder().build().unwrap_throw()
    }

    /// dox
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Convenience method to make a `GET` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    /// Convenience method to make a `POST` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    /// Convenience method to make a `PUT` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PUT, url)
    }

    /// Convenience method to make a `PATCH` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }

    /// Convenience method to make a `DELETE` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    /// Convenience method to make a `HEAD` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::HEAD, url)
    }

    /// Start building a `Request` with the `Method` and `Url`.
    ///
    /// Returns a `RequestBuilder`, which will allow setting headers and
    /// request body before sending.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        let req = url.into_url().map(move |url| Request::new(method, url));
        RequestBuilder::new(self.clone(), req)
    }

    /// Executes a `Request`.
    ///
    /// A `Request` can be built manually with `Request::new()` or obtained
    /// from a RequestBuilder with `RequestBuilder::build()`.
    ///
    /// You should prefer to use the `RequestBuilder` and
    /// `RequestBuilder::send()`.
    ///
    /// # Errors
    ///
    /// This method fails if there was an error while sending request,
    /// redirect loop was detected or redirect limit was exhausted.
    pub fn execute(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Response, crate::Error>> {
        self.execute_request(request)
    }

    // merge request headers with Client default_headers, prior to external http fetch
    fn merge_headers(&self, req: &mut Request) {
        use http::header::Entry;
        let headers: &mut HeaderMap = req.headers_mut();
        // insert default headers in the request headers
        // without overwriting already appended headers.
        for (key, value) in self.config.headers.iter() {
            if let Entry::Vacant(entry) = headers.entry(key) {
                entry.insert(value.clone());
            }
        }
    }

    pub(super) fn execute_request(
        &self,
        mut req: Request,
    ) -> impl Future<Output = crate::Result<Response>> {
        self.merge_headers(&mut req);
        fetch(req)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("Client");
        self.config.fmt_fields(&mut builder);
        builder.finish()
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("ClientBuilder");
        self.config.fmt_fields(&mut builder);
        builder.finish()
    }
}

// Can use new methods in web-sys when requiring v0.2.93.
// > `init.method(m)` to `init.set_method(m)`
// For now, ignore their deprecation.
#[allow(deprecated)]
async fn fetch(req: Request) -> crate::Result<Response> {
    // Build the js Request
    let mut init = web_sys::RequestInit::new();
    init.method(req.method().as_str());

    // convert HeaderMap to Headers
    let js_headers = web_sys::Headers::new()
        .map_err(crate::error::wasm)
        .map_err(crate::error::builder)?;

    for (name, value) in req.headers() {
        js_headers
            .append(
                name.as_str(),
                value.to_str().map_err(crate::error::builder)?,
            )
            .map_err(crate::error::wasm)
            .map_err(crate::error::builder)?;
    }
    init.headers(&js_headers.into());

    // When req.cors is true, do nothing because the default mode is 'cors'
    if !req.cors {
        init.mode(web_sys::RequestMode::NoCors);
    }

    if let Some(creds) = req.credentials {
        init.credentials(creds);
    }

    if let Some(cache) = req.cache {
        init.set_cache(cache);
    }

    if let Some(body) = req.body() {
        if !body.is_empty() {
            init.body(Some(body.to_js_value()?.as_ref()));
        }
    }

    let mut abort = AbortGuard::new()?;
    if let Some(timeout) = req.timeout() {
        abort.timeout(*timeout);
    }
    init.signal(Some(&abort.signal()));

    let js_req = web_sys::Request::new_with_str_and_init(req.url().as_str(), &init)
        .map_err(crate::error::wasm)
        .map_err(crate::error::builder)?;

    // Await the fetch() promise
    let p = js_fetch(&js_req);
    let js_resp = super::promise::<web_sys::Response>(p)
        .await
        .map_err(|error| {
            if error.to_string() == "JsValue(\"reqwest::errors::TimedOut\")" {
                crate::error::TimedOut.into()
            } else {
                error
            }
        })
        .map_err(crate::error::request)?;

    // Convert from the js Response
    let mut resp = http::Response::builder().status(js_resp.status());

    let url = Url::parse(&js_resp.url()).expect_throw("url parse");

    let js_headers = js_resp.headers();
    let js_iter = js_sys::try_iter(&js_headers)
        .expect_throw("headers try_iter")
        .expect_throw("headers have an iterator");

    for item in js_iter {
        let item = item.expect_throw("headers iterator doesn't throw");
        let serialized_headers: String = JSON::stringify(&item)
            .expect_throw("serialized headers")
            .into();
        let [name, value]: [String; 2] = serde_json::from_str(&serialized_headers)
            .expect_throw("deserializable serialized headers");
        resp = resp.header(&name, &value);
    }

    resp.body(js_resp)
        .map(|resp| Response::new(resp, url, abort))
        .map_err(crate::error::request)
}

// ===== impl ClientBuilder =====

impl ClientBuilder {
    /// dox
    pub fn new() -> Self {
        ClientBuilder {
            config: Config::default(),
        }
    }

    /// Returns a 'Client' that uses this ClientBuilder configuration
    pub fn build(mut self) -> Result<Client, crate::Error> {
        if let Some(err) = self.config.error {
            return Err(err);
        }

        let config = std::mem::take(&mut self.config);
        Ok(Client {
            config: Arc::new(config),
        })
    }

    /// Sets the `User-Agent` header to be used by this client.
    pub fn user_agent<V>(mut self, value: V) -> ClientBuilder
    where
        V: TryInto<HeaderValue>,
        V::Error: Into<http::Error>,
    {
        match value.try_into() {
            Ok(value) => {
                self.config.headers.insert(USER_AGENT, value);
            }
            Err(e) => {
                self.config.error = Some(crate::error::builder(e.into()));
            }
        }
        self
    }

    /// Sets the default headers for every request
    pub fn default_headers(mut self, headers: HeaderMap) -> ClientBuilder {
        for (key, value) in headers.iter() {
            self.config.headers.insert(key, value.clone());
        }
        self
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct Config {
    headers: HeaderMap,
    error: Option<crate::Error>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            headers: HeaderMap::new(),
            error: None,
        }
    }
}

impl Config {
    fn fmt_fields(&self, f: &mut fmt::DebugStruct<'_, '_>) {
        f.field("default_headers", &self.headers);
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn default_headers() {
        use crate::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("x-custom", HeaderValue::from_static("flibbertigibbet"));
        let client = crate::Client::builder()
            .default_headers(headers)
            .build()
            .expect("client");
        let mut req = client
            .get("https://www.example.com")
            .build()
            .expect("request");
        // merge headers as if client were about to issue fetch
        client.merge_headers(&mut req);

        let test_headers = req.headers();
        assert!(test_headers.get(CONTENT_TYPE).is_some(), "content-type");
        assert!(test_headers.get("x-custom").is_some(), "custom header");
        assert!(test_headers.get("accept").is_none(), "no accept header");
    }

    #[wasm_bindgen_test]
    async fn default_headers_clone() {
        use crate::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("x-custom", HeaderValue::from_static("flibbertigibbet"));
        let client = crate::Client::builder()
            .default_headers(headers)
            .build()
            .expect("client");

        let mut req = client
            .get("https://www.example.com")
            .header(CONTENT_TYPE, "text/plain")
            .build()
            .expect("request");
        client.merge_headers(&mut req);
        let headers1 = req.headers();

        // confirm that request headers override defaults
        assert_eq!(
            headers1.get(CONTENT_TYPE).unwrap(),
            "text/plain",
            "request headers override defaults"
        );

        // confirm that request headers don't change client defaults
        let mut req2 = client
            .get("https://www.example.com/x")
            .build()
            .expect("req 2");
        client.merge_headers(&mut req2);
        let headers2 = req2.headers();
        assert_eq!(
            headers2.get(CONTENT_TYPE).unwrap(),
            "application/json",
            "request headers don't change client defaults"
        );
    }

    #[wasm_bindgen_test]
    fn user_agent_header() {
        use crate::header::USER_AGENT;

        let client = crate::Client::builder()
            .user_agent("FooBar/1.2.3")
            .build()
            .expect("client");

        let mut req = client
            .get("https://www.example.com")
            .build()
            .expect("request");

        // Merge the client headers with the request's one.
        client.merge_headers(&mut req);
        let headers1 = req.headers();

        // Confirm that we have the `User-Agent` header set
        assert_eq!(
            headers1.get(USER_AGENT).unwrap(),
            "FooBar/1.2.3",
            "The user-agent header was not set: {req:#?}"
        );

        // Now we try to overwrite the `User-Agent` value

        let mut req2 = client
            .get("https://www.example.com")
            .header(USER_AGENT, "Another-User-Agent/42")
            .build()
            .expect("request 2");

        client.merge_headers(&mut req2);
        let headers2 = req2.headers();

        assert_eq!(
            headers2.get(USER_AGENT).expect("headers2 user agent"),
            "Another-User-Agent/42",
            "Was not able to overwrite the User-Agent value on the request-builder"
        );
    }
}
