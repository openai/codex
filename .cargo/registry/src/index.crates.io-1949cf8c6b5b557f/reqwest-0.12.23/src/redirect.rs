//! Redirect Handling
//!
//! By default, a `Client` will automatically handle HTTP redirects, having a
//! maximum redirect chain of 10 hops. To customize this behavior, a
//! `redirect::Policy` can be used with a `ClientBuilder`.

use std::fmt;
use std::{error::Error as StdError, sync::Arc};

use crate::header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION, REFERER, WWW_AUTHENTICATE};
use http::{HeaderMap, HeaderValue};
use hyper::StatusCode;

use crate::{async_impl, Url};
use tower_http::follow_redirect::policy::{
    Action as TowerAction, Attempt as TowerAttempt, Policy as TowerPolicy,
};

/// A type that controls the policy on how to handle the following of redirects.
///
/// The default value will catch redirect loops, and has a maximum of 10
/// redirects it will follow in a chain before returning an error.
///
/// - `limited` can be used have the same as the default behavior, but adjust
///   the allowed maximum redirect hops in a chain.
/// - `none` can be used to disable all redirect behavior.
/// - `custom` can be used to create a customized policy.
pub struct Policy {
    inner: PolicyKind,
}

/// A type that holds information on the next request and previous requests
/// in redirect chain.
#[derive(Debug)]
pub struct Attempt<'a> {
    status: StatusCode,
    next: &'a Url,
    previous: &'a [Url],
}

/// An action to perform when a redirect status code is found.
#[derive(Debug)]
pub struct Action {
    inner: ActionKind,
}

impl Policy {
    /// Create a `Policy` with a maximum number of redirects.
    ///
    /// An `Error` will be returned if the max is reached.
    pub fn limited(max: usize) -> Self {
        Self {
            inner: PolicyKind::Limit(max),
        }
    }

    /// Create a `Policy` that does not follow any redirect.
    pub fn none() -> Self {
        Self {
            inner: PolicyKind::None,
        }
    }

    /// Create a custom `Policy` using the passed function.
    ///
    /// # Note
    ///
    /// The default `Policy` handles a maximum loop
    /// chain, but the custom variant does not do that for you automatically.
    /// The custom policy should have some way of handling those.
    ///
    /// Information on the next request and previous requests can be found
    /// on the [`Attempt`] argument passed to the closure.
    ///
    /// Actions can be conveniently created from methods on the
    /// [`Attempt`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use reqwest::{Error, redirect};
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let custom = redirect::Policy::custom(|attempt| {
    ///     if attempt.previous().len() > 5 {
    ///         attempt.error("too many redirects")
    ///     } else if attempt.url().host_str() == Some("example.domain") {
    ///         // prevent redirects to 'example.domain'
    ///         attempt.stop()
    ///     } else {
    ///         attempt.follow()
    ///     }
    /// });
    /// let client = reqwest::Client::builder()
    ///     .redirect(custom)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Attempt`]: struct.Attempt.html
    pub fn custom<T>(policy: T) -> Self
    where
        T: Fn(Attempt) -> Action + Send + Sync + 'static,
    {
        Self {
            inner: PolicyKind::Custom(Box::new(policy)),
        }
    }

    /// Apply this policy to a given [`Attempt`] to produce a [`Action`].
    ///
    /// # Note
    ///
    /// This method can be used together with `Policy::custom()`
    /// to construct one `Policy` that wraps another.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use reqwest::{Error, redirect};
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let custom = redirect::Policy::custom(|attempt| {
    ///     eprintln!("{}, Location: {:?}", attempt.status(), attempt.url());
    ///     redirect::Policy::default().redirect(attempt)
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn redirect(&self, attempt: Attempt) -> Action {
        match self.inner {
            PolicyKind::Custom(ref custom) => custom(attempt),
            PolicyKind::Limit(max) => {
                // The first URL in the previous is the initial URL and not a redirection. It needs to be excluded.
                if attempt.previous.len() > max {
                    attempt.error(TooManyRedirects)
                } else {
                    attempt.follow()
                }
            }
            PolicyKind::None => attempt.stop(),
        }
    }

    pub(crate) fn check(&self, status: StatusCode, next: &Url, previous: &[Url]) -> ActionKind {
        self.redirect(Attempt {
            status,
            next,
            previous,
        })
        .inner
    }

    pub(crate) fn is_default(&self) -> bool {
        matches!(self.inner, PolicyKind::Limit(10))
    }
}

impl Default for Policy {
    fn default() -> Policy {
        // Keep `is_default` in sync
        Policy::limited(10)
    }
}

impl<'a> Attempt<'a> {
    /// Get the type of redirect.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Get the next URL to redirect to.
    pub fn url(&self) -> &Url {
        self.next
    }

    /// Get the list of previous URLs that have already been requested in this chain.
    pub fn previous(&self) -> &[Url] {
        self.previous
    }
    /// Returns an action meaning reqwest should follow the next URL.
    pub fn follow(self) -> Action {
        Action {
            inner: ActionKind::Follow,
        }
    }

    /// Returns an action meaning reqwest should not follow the next URL.
    ///
    /// The 30x response will be returned as the `Ok` result.
    pub fn stop(self) -> Action {
        Action {
            inner: ActionKind::Stop,
        }
    }

    /// Returns an action failing the redirect with an error.
    ///
    /// The `Error` will be returned for the result of the sent request.
    pub fn error<E: Into<Box<dyn StdError + Send + Sync>>>(self, error: E) -> Action {
        Action {
            inner: ActionKind::Error(error.into()),
        }
    }
}

enum PolicyKind {
    Custom(Box<dyn Fn(Attempt) -> Action + Send + Sync + 'static>),
    Limit(usize),
    None,
}

impl fmt::Debug for Policy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Policy").field(&self.inner).finish()
    }
}

impl fmt::Debug for PolicyKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PolicyKind::Custom(..) => f.pad("Custom"),
            PolicyKind::Limit(max) => f.debug_tuple("Limit").field(&max).finish(),
            PolicyKind::None => f.pad("None"),
        }
    }
}

// pub(crate)

#[derive(Debug)]
pub(crate) enum ActionKind {
    Follow,
    Stop,
    Error(Box<dyn StdError + Send + Sync>),
}

pub(crate) fn remove_sensitive_headers(headers: &mut HeaderMap, next: &Url, previous: &[Url]) {
    if let Some(previous) = previous.last() {
        let cross_host = next.host_str() != previous.host_str()
            || next.port_or_known_default() != previous.port_or_known_default();
        if cross_host {
            headers.remove(AUTHORIZATION);
            headers.remove(COOKIE);
            headers.remove("cookie2");
            headers.remove(PROXY_AUTHORIZATION);
            headers.remove(WWW_AUTHENTICATE);
        }
    }
}

#[derive(Debug)]
struct TooManyRedirects;

impl fmt::Display for TooManyRedirects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("too many redirects")
    }
}

impl StdError for TooManyRedirects {}

#[derive(Clone)]
pub(crate) struct TowerRedirectPolicy {
    policy: Arc<Policy>,
    referer: bool,
    urls: Vec<Url>,
    https_only: bool,
}

impl TowerRedirectPolicy {
    pub(crate) fn new(policy: Policy) -> Self {
        Self {
            policy: Arc::new(policy),
            referer: false,
            urls: Vec::new(),
            https_only: false,
        }
    }

    pub(crate) fn with_referer(&mut self, referer: bool) -> &mut Self {
        self.referer = referer;
        self
    }

    pub(crate) fn with_https_only(&mut self, https_only: bool) -> &mut Self {
        self.https_only = https_only;
        self
    }
}

fn make_referer(next: &Url, previous: &Url) -> Option<HeaderValue> {
    if next.scheme() == "http" && previous.scheme() == "https" {
        return None;
    }

    let mut referer = previous.clone();
    let _ = referer.set_username("");
    let _ = referer.set_password(None);
    referer.set_fragment(None);
    referer.as_str().parse().ok()
}

impl TowerPolicy<async_impl::body::Body, crate::Error> for TowerRedirectPolicy {
    fn redirect(&mut self, attempt: &TowerAttempt<'_>) -> Result<TowerAction, crate::Error> {
        let previous_url =
            Url::parse(&attempt.previous().to_string()).expect("Previous URL must be valid");

        let next_url = match Url::parse(&attempt.location().to_string()) {
            Ok(url) => url,
            Err(e) => return Err(crate::error::builder(e)),
        };

        self.urls.push(previous_url.clone());

        match self.policy.check(attempt.status(), &next_url, &self.urls) {
            ActionKind::Follow => {
                if next_url.scheme() != "http" && next_url.scheme() != "https" {
                    return Err(crate::error::url_bad_scheme(next_url));
                }

                if self.https_only && next_url.scheme() != "https" {
                    return Err(crate::error::redirect(
                        crate::error::url_bad_scheme(next_url.clone()),
                        next_url,
                    ));
                }
                Ok(TowerAction::Follow)
            }
            ActionKind::Stop => Ok(TowerAction::Stop),
            ActionKind::Error(e) => Err(crate::error::redirect(e, previous_url)),
        }
    }

    fn on_request(&mut self, req: &mut http::Request<async_impl::body::Body>) {
        if let Ok(next_url) = Url::parse(&req.uri().to_string()) {
            remove_sensitive_headers(req.headers_mut(), &next_url, &self.urls);
            if self.referer {
                if let Some(previous_url) = self.urls.last() {
                    if let Some(v) = make_referer(&next_url, previous_url) {
                        req.headers_mut().insert(REFERER, v);
                    }
                }
            }
        };
    }

    // This is must implemented to make 307 and 308 redirects work
    fn clone_body(&self, body: &async_impl::body::Body) -> Option<async_impl::body::Body> {
        body.try_clone()
    }
}

#[test]
fn test_redirect_policy_limit() {
    let policy = Policy::default();
    let next = Url::parse("http://x.y/z").unwrap();
    let mut previous = (0..=9)
        .map(|i| Url::parse(&format!("http://a.b/c/{i}")).unwrap())
        .collect::<Vec<_>>();

    match policy.check(StatusCode::FOUND, &next, &previous) {
        ActionKind::Follow => (),
        other => panic!("unexpected {other:?}"),
    }

    previous.push(Url::parse("http://a.b.d/e/33").unwrap());

    match policy.check(StatusCode::FOUND, &next, &previous) {
        ActionKind::Error(err) if err.is::<TooManyRedirects>() => (),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn test_redirect_policy_limit_to_0() {
    let policy = Policy::limited(0);
    let next = Url::parse("http://x.y/z").unwrap();
    let previous = vec![Url::parse("http://a.b/c").unwrap()];

    match policy.check(StatusCode::FOUND, &next, &previous) {
        ActionKind::Error(err) if err.is::<TooManyRedirects>() => (),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn test_redirect_policy_custom() {
    let policy = Policy::custom(|attempt| {
        if attempt.url().host_str() == Some("foo") {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });

    let next = Url::parse("http://bar/baz").unwrap();
    match policy.check(StatusCode::FOUND, &next, &[]) {
        ActionKind::Follow => (),
        other => panic!("unexpected {other:?}"),
    }

    let next = Url::parse("http://foo/baz").unwrap();
    match policy.check(StatusCode::FOUND, &next, &[]) {
        ActionKind::Stop => (),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn test_remove_sensitive_headers() {
    use hyper::header::{HeaderValue, ACCEPT, AUTHORIZATION, COOKIE};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(AUTHORIZATION, HeaderValue::from_static("let me in"));
    headers.insert(COOKIE, HeaderValue::from_static("foo=bar"));

    let next = Url::parse("http://initial-domain.com/path").unwrap();
    let mut prev = vec![Url::parse("http://initial-domain.com/new_path").unwrap()];
    let mut filtered_headers = headers.clone();

    remove_sensitive_headers(&mut headers, &next, &prev);
    assert_eq!(headers, filtered_headers);

    prev.push(Url::parse("http://new-domain.com/path").unwrap());
    filtered_headers.remove(AUTHORIZATION);
    filtered_headers.remove(COOKIE);

    remove_sensitive_headers(&mut headers, &next, &prev);
    assert_eq!(headers, filtered_headers);
}
