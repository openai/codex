use std::sync::Arc;
use std::sync::LazyLock;

use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use reqwest::header::HeaderValue;

static SHARED_CHATGPT_COOKIE_STORE: LazyLock<Arc<ChatGptCookieStore>> =
    LazyLock::new(|| Arc::new(ChatGptCookieStore::default()));

#[derive(Debug, Default)]
struct ChatGptCookieStore {
    jar: Jar,
}

impl CookieStore for ChatGptCookieStore {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &HeaderValue>,
        url: &reqwest::Url,
    ) {
        if is_chatgpt_cookie_url(url) {
            self.jar.set_cookies(cookie_headers, url);
        }
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        if is_chatgpt_cookie_url(url) {
            self.jar.cookies(url)
        } else {
            None
        }
    }
}

/// Adds the process-local ChatGPT cookie jar used by Codex HTTP clients.
///
/// The jar is intentionally not persisted to disk. It only preserves cookies for ChatGPT backend
/// hosts so Cloudflare visitor cookies can be replayed across freshly built HTTP clients without
/// broadening cookie handling for arbitrary third-party hosts.
pub fn with_chatgpt_cookie_store(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder.cookie_provider(Arc::clone(&SHARED_CHATGPT_COOKIE_STORE))
}

fn is_chatgpt_cookie_url(url: &reqwest::Url) -> bool {
    match url.scheme() {
        "http" | "https" => {}
        _ => return false,
    }

    let Some(host) = url.host_str() else {
        return false;
    };

    host == "chatgpt.com"
        || host.ends_with(".chatgpt.com")
        || host == "chat.openai.com"
        || host == "chatgpt-staging.com"
        || host.ends_with(".chatgpt-staging.com")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use reqwest::cookie::CookieStore;

    #[test]
    fn stores_and_returns_chatgpt_cookies() {
        let store = ChatGptCookieStore::default();
        let url = reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let set_cookie = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut std::iter::once(&set_cookie), &url);

        assert_eq!(
            store
                .cookies(&url)
                .and_then(|value| value.to_str().ok().map(str::to_string)),
            Some("_cfuvid=visitor".to_string())
        );
    }

    #[test]
    fn ignores_non_chatgpt_cookies() {
        let store = ChatGptCookieStore::default();
        let url = reqwest::Url::parse("https://api.openai.com/v1/responses").unwrap();
        let set_cookie = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut std::iter::once(&set_cookie), &url);

        assert_eq!(store.cookies(&url), None);
    }

    #[test]
    fn does_not_return_chatgpt_cookies_for_other_hosts() {
        let store = ChatGptCookieStore::default();
        let chatgpt_url =
            reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let api_url = reqwest::Url::parse("https://api.openai.com/v1/responses").unwrap();
        let set_cookie = HeaderValue::from_static("_cfuvid=visitor; Path=/; Secure; HttpOnly");

        store.set_cookies(&mut std::iter::once(&set_cookie), &chatgpt_url);

        assert_eq!(store.cookies(&api_url), None);
    }

    #[test]
    fn recognizes_chatgpt_hosts_without_suffix_tricks() {
        for url in [
            "https://chatgpt.com/backend-api/codex/responses",
            "https://foo.chatgpt.com/backend-api/codex/responses",
            "https://chat.openai.com/backend-api/codex/responses",
            "https://api.chatgpt-staging.com/backend-api/codex/responses",
        ] {
            let url = reqwest::Url::parse(url).unwrap();
            assert!(is_chatgpt_cookie_url(&url));
        }

        for url in [
            "https://evilchatgpt.com/backend-api/codex/responses",
            "https://chatgpt.com.evil.example/backend-api/codex/responses",
            "https://api.openai.com/v1/responses",
        ] {
            let url = reqwest::Url::parse(url).unwrap();
            assert!(!is_chatgpt_cookie_url(&url));
        }
    }
}
