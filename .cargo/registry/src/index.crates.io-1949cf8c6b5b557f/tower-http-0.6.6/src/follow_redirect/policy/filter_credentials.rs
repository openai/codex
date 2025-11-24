use super::{eq_origin, Action, Attempt, Policy};
use http::{
    header::{self, HeaderName},
    Request,
};

/// A redirection [`Policy`] that removes credentials from requests in redirections.
#[derive(Clone, Debug)]
pub struct FilterCredentials {
    block_cross_origin: bool,
    block_any: bool,
    remove_blocklisted: bool,
    remove_all: bool,
    blocked: bool,
}

const BLOCKLIST: &[HeaderName] = &[
    header::AUTHORIZATION,
    header::COOKIE,
    header::PROXY_AUTHORIZATION,
];

impl FilterCredentials {
    /// Create a new [`FilterCredentials`] that removes blocklisted request headers in cross-origin
    /// redirections.
    pub fn new() -> Self {
        FilterCredentials {
            block_cross_origin: true,
            block_any: false,
            remove_blocklisted: true,
            remove_all: false,
            blocked: false,
        }
    }

    /// Configure `self` to mark cross-origin redirections as "blocked".
    pub fn block_cross_origin(mut self, enable: bool) -> Self {
        self.block_cross_origin = enable;
        self
    }

    /// Configure `self` to mark every redirection as "blocked".
    pub fn block_any(mut self) -> Self {
        self.block_any = true;
        self
    }

    /// Configure `self` to mark no redirections as "blocked".
    pub fn block_none(mut self) -> Self {
        self.block_any = false;
        self.block_cross_origin(false)
    }

    /// Configure `self` to remove blocklisted headers in "blocked" redirections.
    ///
    /// The blocklist includes the following headers:
    ///
    /// - `Authorization`
    /// - `Cookie`
    /// - `Proxy-Authorization`
    pub fn remove_blocklisted(mut self, enable: bool) -> Self {
        self.remove_blocklisted = enable;
        self
    }

    /// Configure `self` to remove all headers in "blocked" redirections.
    pub fn remove_all(mut self) -> Self {
        self.remove_all = true;
        self
    }

    /// Configure `self` to remove no headers in "blocked" redirections.
    pub fn remove_none(mut self) -> Self {
        self.remove_all = false;
        self.remove_blocklisted(false)
    }
}

impl Default for FilterCredentials {
    fn default() -> Self {
        Self::new()
    }
}

impl<B, E> Policy<B, E> for FilterCredentials {
    fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
        self.blocked = self.block_any
            || (self.block_cross_origin && !eq_origin(attempt.previous(), attempt.location()));
        Ok(Action::Follow)
    }

    fn on_request(&mut self, request: &mut Request<B>) {
        if self.blocked {
            let headers = request.headers_mut();
            if self.remove_all {
                headers.clear();
            } else if self.remove_blocklisted {
                for key in BLOCKLIST {
                    headers.remove(key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Uri;

    #[test]
    fn works() {
        let mut policy = FilterCredentials::default();

        let initial = Uri::from_static("http://example.com/old");
        let same_origin = Uri::from_static("http://example.com/new");
        let cross_origin = Uri::from_static("https://example.com/new");

        let mut request = Request::builder()
            .uri(initial)
            .header(header::COOKIE, "42")
            .body(())
            .unwrap();
        Policy::<(), ()>::on_request(&mut policy, &mut request);
        assert!(request.headers().contains_key(header::COOKIE));

        let attempt = Attempt {
            status: Default::default(),
            location: &same_origin,
            previous: request.uri(),
        };
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_follow());

        let mut request = Request::builder()
            .uri(same_origin)
            .header(header::COOKIE, "42")
            .body(())
            .unwrap();
        Policy::<(), ()>::on_request(&mut policy, &mut request);
        assert!(request.headers().contains_key(header::COOKIE));

        let attempt = Attempt {
            status: Default::default(),
            location: &cross_origin,
            previous: request.uri(),
        };
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_follow());

        let mut request = Request::builder()
            .uri(cross_origin)
            .header(header::COOKIE, "42")
            .body(())
            .unwrap();
        Policy::<(), ()>::on_request(&mut policy, &mut request);
        assert!(!request.headers().contains_key(header::COOKIE));
    }
}
