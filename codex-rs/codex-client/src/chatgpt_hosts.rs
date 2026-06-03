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
}
