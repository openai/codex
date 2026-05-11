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

/// Returns whether `url` is an HTTPS URL targeting a first-party ChatGPT host.
pub fn is_allowed_chatgpt_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(is_allowed_chatgpt_host)
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
    fn recognizes_chatgpt_urls_without_origin_confusion() {
        for url in [
            "https://chatgpt.com/backend-api",
            "https://foo.chatgpt.com/backend-api",
            "https://chat.openai.com/backend-api",
            "https://api.chatgpt-staging.com/backend-api",
        ] {
            let parsed = reqwest::Url::parse(url).expect("test URL should parse");
            assert!(is_allowed_chatgpt_url(&parsed));
        }

        for url in [
            "http://chatgpt.com/backend-api",
            "https://chatgpt.com.fromspeech.ai/backend-api",
            "https://chat.openai.com.evil.example/backend-api",
            "https://api.openai.com/v1/responses",
        ] {
            let parsed = reqwest::Url::parse(url).expect("test URL should parse");
            assert!(!is_allowed_chatgpt_url(&parsed));
        }
    }
}
