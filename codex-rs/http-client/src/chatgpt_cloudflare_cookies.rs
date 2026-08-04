use std::sync::Arc;
use std::sync::LazyLock;

use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use reqwest::header::HeaderValue;

use crate::chatgpt_hosts::is_allowed_chatgpt_host;

// WARNING: this HTTP cookie store is process-global and may be shared across auth contexts.
// It must only ever contain Cloudflare infrastructure cookies. Never extend this
// store to persist ChatGPT account, session, auth, or other user-specific cookie
// data.
static SHARED_CHATGPT_CLOUDFLARE_COOKIE_STORE: LazyLock<Arc<ChatGptCloudflareCookieStore>> =
    LazyLock::new(|| Arc::new(ChatGptCloudflareCookieStore::default()));

#[derive(Debug, Default)]
struct ChatGptCloudflareCookieStore {
    jar: Jar,
}

pub(crate) struct ChatGptCookieStore {
    cloudflare: Arc<ChatGptCloudflareCookieStore>,
    cookies: Vec<HeaderValue>,
}

impl ChatGptCookieStore {
    pub(crate) fn new(cookies: Vec<HeaderValue>) -> Self {
        Self {
            cloudflare: Arc::clone(&SHARED_CHATGPT_CLOUDFLARE_COOKIE_STORE),
            cookies,
        }
    }

    pub(crate) fn configured_cookies(&self) -> &[HeaderValue] {
        &self.cookies
    }
}

impl CookieStore for ChatGptCloudflareCookieStore {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &HeaderValue>,
        url: &reqwest::Url,
    ) {
        if !is_chatgpt_cookie_url(url) {
            return;
        }

        let mut cloudflare_cookie_headers =
            cookie_headers.filter(|header| is_allowed_cloudflare_set_cookie_header(header));
        self.jar.set_cookies(&mut cloudflare_cookie_headers, url);
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        if is_chatgpt_cookie_url(url) {
            self.jar.cookies(url).and_then(only_cloudflare_cookies)
        } else {
            None
        }
    }
}

impl CookieStore for ChatGptCookieStore {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &HeaderValue>,
        url: &reqwest::Url,
    ) {
        self.cloudflare.set_cookies(cookie_headers, url);
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        if !is_chatgpt_cookie_url(url) {
            return self.cloudflare.cookies(url);
        }
        let cloudflare = self.cloudflare.cookies(url);
        let mut sensitive = cloudflare.as_ref().is_some_and(HeaderValue::is_sensitive);
        let mut cookies = cloudflare
            .map(|cloudflare| cloudflare.as_bytes().to_vec())
            .unwrap_or_default();
        for cookie in &self.cookies {
            sensitive |= cookie.is_sensitive();
            if !cookies.is_empty() {
                cookies.extend_from_slice(b"; ");
            }
            cookies.extend_from_slice(cookie.as_bytes());
        }
        if cookies.is_empty() {
            None
        } else {
            let mut header = HeaderValue::from_bytes(&cookies).ok()?;
            header.set_sensitive(sensitive);
            Some(header)
        }
    }
}

/// Adds the process-local ChatGPT Cloudflare cookie jar used by Codex HTTP clients.
///
/// WARNING: this jar is global within the process. It is only acceptable because it hardcodes a
/// small allowlist of Cloudflare cookie names and refuses all other ChatGPT cookies. Do not store
/// ChatGPT account, session, auth, or other user-specific cookies here. If a future caller needs
/// those cookies, the store must be scoped to the auth/session owner instead of shared globally.
pub fn with_chatgpt_cloudflare_cookie_store(
    builder: reqwest::ClientBuilder,
) -> reqwest::ClientBuilder {
    builder.cookie_provider(Arc::clone(&SHARED_CHATGPT_CLOUDFLARE_COOKIE_STORE))
}

fn is_chatgpt_cookie_url(url: &reqwest::Url) -> bool {
    match url.scheme() {
        "https" => {}
        _ => return false,
    }

    let Some(host) = url.host_str() else {
        return false;
    };

    is_allowed_chatgpt_host(host)
}

fn is_allowed_cloudflare_set_cookie_header(header: &HeaderValue) -> bool {
    header
        .to_str()
        .ok()
        .and_then(set_cookie_name)
        .is_some_and(is_allowed_cloudflare_cookie_name)
}

fn set_cookie_name(header: &str) -> Option<&str> {
    let (name, _) = header.split_once('=')?;
    let name = name.trim();
    (!name.is_empty()).then_some(name)
}

fn only_cloudflare_cookies(header: HeaderValue) -> Option<HeaderValue> {
    let header = header.to_str().ok()?;
    let cookies = header
        .split(';')
        .filter_map(|cookie| {
            let cookie = cookie.trim();
            let name = cookie.split_once('=')?.0.trim();
            is_allowed_cloudflare_cookie_name(name).then_some(cookie)
        })
        .collect::<Vec<_>>()
        .join("; ");

    if cookies.is_empty() {
        None
    } else {
        HeaderValue::from_str(&cookies).ok()
    }
}

fn is_allowed_cloudflare_cookie_name(name: &str) -> bool {
    // Keep this allowlist aligned with Cloudflare's documented service cookies:
    // https://developers.cloudflare.com/fundamentals/reference/policies-compliances/cloudflare-cookies/
    matches!(
        name,
        "__cf_bm"
            | "__cflb"
            | "__cfruid"
            | "__cfseq"
            | "__cfwaitingroom"
            | "_cfuvid"
            | "cf_clearance"
            | "cf_ob_info"
            | "cf_use_ob"
    ) || name.starts_with("cf_chl_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use reqwest::cookie::CookieStore;

    #[test]
    fn additional_cookies_use_current_path_scoped_cloudflare_cookies() {
        let cloudflare = Arc::new(ChatGptCloudflareCookieStore::default());
        let mut sensitive_cookie = HeaderValue::from_static("additional=true");
        sensitive_cookie.set_sensitive(true);
        let store = ChatGptCookieStore {
            cloudflare: Arc::clone(&cloudflare),
            cookies: vec![
                sensitive_cookie,
                HeaderValue::from_static("another=enabled"),
            ],
        };
        let first_url = reqwest::Url::parse("https://cookies.chatgpt.com/a").unwrap();
        let second_url = reqwest::Url::parse("https://cookies.chatgpt.com/b").unwrap();
        let affinity = HeaderValue::from_static("__cflb=current; Path=/a; Secure");
        cloudflare.set_cookies(&mut std::iter::once(&affinity), &first_url);

        assert_eq!(
            store.cookies(&first_url),
            Some(HeaderValue::from_static(
                "__cflb=current; additional=true; another=enabled"
            ))
        );
        assert!(store.cookies(&first_url).unwrap().is_sensitive());
        assert_eq!(
            store.cookies(&second_url),
            Some(HeaderValue::from_static("additional=true; another=enabled"))
        );

        let expired = HeaderValue::from_static("__cflb=; Max-Age=0; Path=/a; Secure");
        store.set_cookies(&mut std::iter::once(&expired), &first_url);
        assert_eq!(
            store.cookies(&first_url),
            Some(HeaderValue::from_static("additional=true; another=enabled"))
        );
    }

    #[test]
    fn additional_cookies_are_restricted_to_https_chatgpt_hosts() {
        let store = ChatGptCookieStore {
            cloudflare: Arc::new(ChatGptCloudflareCookieStore::default()),
            cookies: vec![HeaderValue::from_static("additional=true")],
        };

        for url in ["http://chatgpt.com/a", "https://api.openai.com/a"] {
            let url = reqwest::Url::parse(url).unwrap();
            assert_eq!(store.cookies(&url), None);
        }
        let url = reqwest::Url::parse("https://chatgpt.com/a").unwrap();
        assert_eq!(
            store.cookies(&url),
            Some(HeaderValue::from_static("additional=true"))
        );
        assert!(!store.cookies(&url).unwrap().is_sensitive());
    }

    #[test]
    fn stores_and_returns_cloudflare_cookies_for_chatgpt_hosts() {
        let store = ChatGptCloudflareCookieStore::default();
        let url = reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let load_balancer = HeaderValue::from_static("__cflb=west; Path=/; Secure; HttpOnly");
        let cfuvid = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");
        let clearance =
            HeaderValue::from_static("cf_clearance=clearance; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut [&load_balancer, &cfuvid, &clearance].into_iter(), &url);

        let mut cookies = store
            .cookies(&url)
            .and_then(|value| value.to_str().ok().map(str::to_string))
            .map(|header| {
                header
                    .split("; ")
                    .map(str::to_string)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        cookies.sort();
        assert_eq!(
            cookies,
            vec![
                "__cflb=west".to_string(),
                "_cfuvid=visitor".to_string(),
                "cf_clearance=clearance".to_string()
            ]
        );
    }

    #[test]
    fn ignores_non_chatgpt_cookies() {
        let store = ChatGptCloudflareCookieStore::default();
        let url = reqwest::Url::parse("https://api.openai.com/v1/responses").unwrap();
        let set_cookie = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut std::iter::once(&set_cookie), &url);

        assert_eq!(store.cookies(&url), None);
    }

    #[test]
    fn ignores_non_cloudflare_cookies_for_chatgpt_hosts() {
        let store = ChatGptCloudflareCookieStore::default();
        let url = reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let set_cookie = HeaderValue::from_static(
            "__Secure-next-auth.session-token=secret; Path=/; Secure; HttpOnly",
        );

        store.set_cookies(&mut std::iter::once(&set_cookie), &url);

        assert_eq!(store.cookies(&url), None);
    }

    #[test]
    fn ignores_mixed_non_cloudflare_cookies_for_chatgpt_hosts() {
        let store = ChatGptCloudflareCookieStore::default();
        let url = reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let cfuvid = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");
        let account_cookie =
            HeaderValue::from_static("chatgpt_session=secret; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut [&cfuvid, &account_cookie].into_iter(), &url);

        assert_eq!(
            store
                .cookies(&url)
                .and_then(|value| value.to_str().ok().map(str::to_string)),
            Some("_cfuvid=visitor".to_string())
        );
    }

    #[test]
    fn does_not_return_chatgpt_cloudflare_cookies_for_other_hosts() {
        let store = ChatGptCloudflareCookieStore::default();
        let chatgpt_url =
            reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let api_url = reqwest::Url::parse("https://api.openai.com/v1/responses").unwrap();
        let set_cookie = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut std::iter::once(&set_cookie), &chatgpt_url);

        assert_eq!(store.cookies(&api_url), None);
    }

    #[test]
    fn rejects_plain_http_chatgpt_cookie_urls() {
        let store = ChatGptCloudflareCookieStore::default();
        let http_url = reqwest::Url::parse("http://chatgpt.com/backend-api/codex/responses")
            .expect("URL should parse");
        let https_url = reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses")
            .expect("URL should parse");
        let set_cookie = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut std::iter::once(&set_cookie), &http_url);

        assert_eq!(store.cookies(&http_url), None);
        assert_eq!(store.cookies(&https_url), None);
    }

    #[test]
    fn only_allows_https_urls() {
        let url = reqwest::Url::parse("http://chatgpt.com/backend-api/codex/responses").unwrap();

        assert!(!is_chatgpt_cookie_url(&url));

        let url = reqwest::Url::parse("wss://chatgpt.com/backend-api/codex/responses").unwrap();

        assert!(!is_chatgpt_cookie_url(&url));
    }

    #[test]
    fn allows_only_known_cloudflare_cookie_names() {
        for name in [
            "__cf_bm",
            "__cflb",
            "__cfruid",
            "__cfseq",
            "__cfwaitingroom",
            "_cfuvid",
            "cf_clearance",
            "cf_ob_info",
            "cf_use_ob",
            "cf_chl_rc_i",
        ] {
            assert!(is_allowed_cloudflare_cookie_name(name));
        }

        for name in [
            "__Secure-next-auth.session-token",
            "chatgpt_session",
            "oai-auth-token",
            "not_cf_clearance",
        ] {
            assert!(!is_allowed_cloudflare_cookie_name(name));
        }
    }
}
