//! Redirect semantics for requests whose proxy route is selected from the complete URL.
//!
//! Reqwest normally follows redirects inside one `Client::execute` call. That is safe for
//! `ReqwestDefault`, where reqwest owns proxy selection, but not for `RespectSystemProxy`: Codex
//! resolves PAC and operating-system proxy settings before building a client pinned to the
//! resulting direct or concrete-proxy route. An internal reqwest redirect would reuse that client
//! for the new URL without giving [`crate::RouteAwareClientPool`] an opportunity to resolve the
//! redirect destination.
//!
//! The pool therefore disables reqwest redirects for `RespectSystemProxy`, observes each redirect
//! response, and uses these helpers to construct the next request before resolving its route. The
//! helpers preserve the relevant reqwest method/body behavior while applying stricter
//! origin-sensitive credential and `Referer` rules at the boundary between requests.

use std::time::Duration;

use http::HeaderMap;
use http::Method;
use http::StatusCode;
use http::header::AUTHORIZATION;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::COOKIE;
use http::header::LOCATION;
use http::header::PROXY_AUTHORIZATION;
use http::header::REFERER;
use http::header::TRANSFER_ENCODING;
use http::header::WWW_AUTHENTICATE;

pub(super) const MAX_REDIRECTS: usize = 10;

pub(super) fn is_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

pub(super) fn redirect_url(response: &reqwest::Response) -> Option<reqwest::Url> {
    let location = response.headers().get(LOCATION)?.to_str().ok()?;
    response.url().join(location).ok()
}

pub(super) fn redirect_request(
    status: StatusCode,
    mut method: Method,
    mut headers: HeaderMap,
    version: http::Version,
    timeout: Option<Duration>,
    replay: Option<reqwest::Request>,
    next_url: reqwest::Url,
) -> Option<reqwest::Request> {
    let drop_body = match status {
        StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND if method == Method::POST => {
            method = Method::GET;
            true
        }
        StatusCode::SEE_OTHER => {
            if method != Method::HEAD {
                method = Method::GET;
            }
            true
        }
        StatusCode::MOVED_PERMANENTLY
        | StatusCode::FOUND
        | StatusCode::TEMPORARY_REDIRECT
        | StatusCode::PERMANENT_REDIRECT => false,
        _ => return None,
    };

    if drop_body {
        for header in [
            CONTENT_TYPE,
            CONTENT_LENGTH,
            CONTENT_ENCODING,
            TRANSFER_ENCODING,
        ] {
            headers.remove(header);
        }
        let mut request = reqwest::Request::new(method, next_url);
        *request.headers_mut() = headers;
        *request.version_mut() = version;
        *request.timeout_mut() = timeout;
        Some(request)
    } else {
        replay.map(|mut request| {
            *request.url_mut() = next_url;
            request
        })
    }
}

pub(super) fn remove_sensitive_headers(
    headers: &mut HeaderMap,
    previous: &reqwest::Url,
    next: &reqwest::Url,
) {
    if !same_origin(previous, next) {
        for header in [AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION, WWW_AUTHENTICATE] {
            headers.remove(header);
        }
        headers.remove("cookie2");
    }
}

pub(super) fn insert_referer(
    headers: &mut HeaderMap,
    previous: &reqwest::Url,
    next: &reqwest::Url,
) {
    headers.remove(REFERER);
    if next.scheme() == "http" && previous.scheme() == "https" {
        return;
    }

    let mut referer = previous.clone();
    let _ = referer.set_username("");
    let _ = referer.set_password(None);
    referer.set_fragment(None);
    if !same_origin(previous, next) {
        referer.set_path("/");
        referer.set_query(None);
    }
    if let Ok(value) = referer.as_str().parse() {
        headers.insert(REFERER, value);
    }
}

fn same_origin(previous: &reqwest::Url, next: &reqwest::Url) -> bool {
    previous.scheme() == next.scheme()
        && previous.host_str() == next.host_str()
        && previous.port_or_known_default() == next.port_or_known_default()
}

#[cfg(test)]
#[path = "route_aware_redirect_tests.rs"]
mod tests;
