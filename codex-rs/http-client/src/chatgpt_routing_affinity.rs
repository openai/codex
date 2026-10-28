//! Connection-owned ChatGPT routing pins. Keep only the handshake's routing cookies,
//! with their original expiry, and never forward them outside the original endpoint.

use std::sync::Arc;

use http::HeaderMap;
use http::HeaderValue;
use http::Uri;
use http::header::SET_COOKIE;
use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use url::Url;

use crate::chatgpt_cloudflare_cookies::chatgpt_cookie_url;

const ROUTING_COOKIE_NAMES: [&str; 2] = ["__oailb", "__cflb"];

/// A routing pin returned by one connection, independent of the process-wide cookie jar.
/// This is a best-effort hint: the edge still validates the cookie and may fail over.
/// Equality identifies the captured connection, independently of cookie expiry, so a live
/// colocated reviewer socket need not be disconnected when its handshake cookie expires.
#[derive(Clone)]
pub struct ChatGptRoutingAffinity {
    url: Url,
    jar: Arc<Jar>,
}

impl std::fmt::Debug for ChatGptRoutingAffinity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatGptRoutingAffinity")
            .finish_non_exhaustive()
    }
}

impl PartialEq for ChatGptRoutingAffinity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.jar, &other.jar)
    }
}

impl Eq for ChatGptRoutingAffinity {}

impl ChatGptRoutingAffinity {
    /// Captures valid `__oailb` and Cloudflare's native `__cflb` response cookies.
    /// Ambiguous or oversized cookies reject the pin; request cookies alone cannot establish
    /// where failover landed. Cookie scope and expiry remain owned by the private jar.
    pub fn from_response(uri: &Uri, headers: &HeaderMap) -> Option<Self> {
        let url = chatgpt_cookie_url(uri)?;
        let mut cookies = Vec::with_capacity(2);
        for name in ROUTING_COOKIE_NAMES {
            let mut matches = headers.get_all(SET_COOKIE).iter().filter(|header| {
                header
                    .to_str()
                    .ok()
                    .and_then(|value| value.split_once('='))
                    .is_some_and(|(cookie_name, _)| cookie_name.trim() == name)
            });
            if let Some(cookie) = matches.next() {
                if cookie.as_bytes().len() > 8192 || matches.next().is_some() {
                    return None;
                }
                cookies.push(cookie);
            }
        }
        let jar = Jar::default();
        jar.set_cookies(&mut cookies.into_iter(), &url);
        jar.cookies(&url)?;
        Some(Self {
            url,
            jar: Arc::new(jar),
        })
    }

    /// Returns the still-valid pin only for the same secure ChatGPT origin and endpoint path.
    /// The private jar retains Max-Age/Expires and is never refreshed by unrelated traffic.
    pub fn cookie_header(&self, uri: &Uri) -> Option<HeaderValue> {
        let url = chatgpt_cookie_url(uri)?;
        if url.origin() != self.url.origin() || url.path() != self.url.path() {
            return None;
        }
        let mut cookie = self.jar.cookies(&url)?;
        cookie.set_sensitive(true);
        Some(cookie)
    }

    /// Replaces routing cookies with the scoped pin while preserving unrelated cookies.
    /// Returns `None` when the pin is no longer valid for this endpoint.
    pub fn merge_cookie_header(
        &self,
        uri: &Uri,
        existing: Option<&HeaderValue>,
    ) -> Option<HeaderValue> {
        let pin = self.cookie_header(uri)?;
        let mut parts = existing
            .and_then(|value| value.to_str().ok())
            .into_iter()
            .flat_map(|value| value.split(';'))
            .map(str::trim)
            .filter(|part| {
                !part.is_empty()
                    && !part
                        .split_once('=')
                        .is_some_and(|(name, _)| ROUTING_COOKIE_NAMES.contains(&name.trim()))
            })
            .collect::<Vec<_>>();
        parts.push(pin.to_str().ok()?);
        let mut cookies = HeaderValue::from_str(&parts.join("; ")).ok()?;
        cookies.set_sensitive(true);
        Some(cookies)
    }
}

#[cfg(test)]
#[path = "chatgpt_routing_affinity_tests.rs"]
mod tests;
