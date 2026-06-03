/// Returns whether `host` is one of the ChatGPT hosts Codex is allowed to treat
/// as first-party ChatGPT traffic.
pub fn is_allowed_chatgpt_host(host: &str) -> bool {
    const EXACT_HOSTS: &[&str] = &["chatgpt.com", "chat.openai.com", "chatgpt-staging.com"];
    const SUBDOMAIN_SUFFIXES: &[&str] = &[".chatgpt.com", ".chatgpt-staging.com"];

    EXACT_HOSTS.contains(&host)
        || SUBDOMAIN_SUFFIXES
            .iter()
            .any(|suffix| host.ends_with(suffix))
}

/// Returns whether `url` is an HTTPS or secure WebSocket ChatGPT URL that Codex
/// may treat as first-party traffic.
pub fn is_allowed_chatgpt_request_url(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    if !matches!(url.scheme(), "https" | "wss") {
        return false;
    }
    url.host_str().is_some_and(is_allowed_chatgpt_host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_chatgpt_hosts_without_suffix_tricks() {
        for host in [
            "chatgpt.com",
            "foo.chatgpt.com",
            "staging.chatgpt.com",
            "chat.openai.com",
            "chatgpt-staging.com",
            "api.chatgpt-staging.com",
        ] {
            assert!(is_allowed_chatgpt_host(host));
        }

        for host in [
            "evilchatgpt.com",
            "chatgpt.com.evil.example",
            "api.openai.com",
            "foo.chat.openai.com",
        ] {
            assert!(!is_allowed_chatgpt_host(host));
        }
    }

    #[test]
    fn recognizes_secure_chatgpt_request_urls() {
        for url in [
            "https://chatgpt.com/backend-api/codex/responses",
            "https://preview.chatgpt.com/backend-api/codex/models",
            "wss://chatgpt-staging.com/backend-api/codex/responses",
        ] {
            assert!(is_allowed_chatgpt_request_url(url));
        }

        for url in [
            "http://chatgpt.com/backend-api/codex/responses",
            "ws://chatgpt.com/backend-api/codex/responses",
            "https://api.openai.com/v1/responses",
            "https://chatgpt.com.evil.example/backend-api",
            "not a url",
        ] {
            assert!(!is_allowed_chatgpt_request_url(url));
        }
    }
}
