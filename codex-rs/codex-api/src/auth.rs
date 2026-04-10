use codex_client::Request;
use http::HeaderMap;
use http::HeaderValue;

/// Provides bearer and account identity information for API requests.
///
/// Implementations should be cheap and non-blocking; any asynchronous
/// refresh or I/O should be handled by higher layers before requests
/// reach this interface.
pub trait AuthProvider: Send + Sync {
    fn bearer_token(&self) -> Option<String>;
    fn account_id(&self) -> Option<String> {
        None
    }
    fn is_fedramp_account(&self) -> bool {
        false
    }
}

pub(crate) fn add_auth_headers_to_header_map<A: AuthProvider>(auth: &A, headers: &mut HeaderMap) {
    if let Some(token) = auth.bearer_token()
        && let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        let _ = headers.insert(http::header::AUTHORIZATION, header);
    }
    if let Some(account_id) = auth.account_id()
        && let Ok(header) = HeaderValue::from_str(&account_id)
    {
        let _ = headers.insert("ChatGPT-Account-ID", header);
    }
    if auth.is_fedramp_account() {
        add_fedramp_routing_cookie(headers);
    }
}

fn add_fedramp_routing_cookie(headers: &mut HeaderMap) {
    const FEDRAMP_ROUTING_COOKIE: &str = "_account_is_fedramp=true";
    let Some(value) = headers.get(http::header::COOKIE) else {
        headers.insert(
            http::header::COOKIE,
            HeaderValue::from_static(FEDRAMP_ROUTING_COOKIE),
        );
        return;
    };

    let Ok(existing) = value.to_str() else {
        return;
    };
    if existing
        .split(';')
        .any(|cookie| cookie.trim() == FEDRAMP_ROUTING_COOKIE)
    {
        return;
    }

    if let Ok(value) = HeaderValue::from_str(&format!("{existing}; {FEDRAMP_ROUTING_COOKIE}")) {
        headers.insert(http::header::COOKIE, value);
    }
}

pub(crate) fn add_auth_headers<A: AuthProvider>(auth: &A, mut req: Request) -> Request {
    add_auth_headers_to_header_map(auth, &mut req.headers);
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAuth {
        is_fedramp_account: bool,
    }

    impl AuthProvider for TestAuth {
        fn bearer_token(&self) -> Option<String> {
            None
        }

        fn is_fedramp_account(&self) -> bool {
            self.is_fedramp_account
        }
    }

    #[test]
    fn auth_headers_add_fedramp_routing_cookie() {
        let auth = TestAuth {
            is_fedramp_account: true,
        };
        let mut headers = HeaderMap::new();

        add_auth_headers_to_header_map(&auth, &mut headers);

        assert_eq!(
            headers
                .get(http::header::COOKIE)
                .and_then(|v| v.to_str().ok()),
            Some("_account_is_fedramp=true")
        );
    }

    #[test]
    fn auth_headers_do_not_add_fedramp_cookie_by_default() {
        let auth = TestAuth {
            is_fedramp_account: false,
        };
        let mut headers = HeaderMap::new();

        add_auth_headers_to_header_map(&auth, &mut headers);

        assert!(headers.get(http::header::COOKIE).is_none());
    }

    #[test]
    fn auth_headers_merge_fedramp_routing_cookie_with_existing_cookie() {
        let auth = TestAuth {
            is_fedramp_account: true,
        };
        let mut headers = HeaderMap::new();
        headers.insert(http::header::COOKIE, HeaderValue::from_static("foo=bar"));

        add_auth_headers_to_header_map(&auth, &mut headers);

        assert_eq!(
            headers
                .get(http::header::COOKIE)
                .and_then(|v| v.to_str().ok()),
            Some("foo=bar; _account_is_fedramp=true")
        );
    }
}
