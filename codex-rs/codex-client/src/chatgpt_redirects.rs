use reqwest::redirect::Policy;

use crate::chatgpt_hosts::is_allowed_chatgpt_request_url;

/// Prevent headers that are valid only for first-party ChatGPT traffic from
/// being replayed to an arbitrary redirect target.
pub fn with_chatgpt_redirect_protection(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder.redirect(Policy::custom(|attempt| {
        let leaves_chatgpt_allowlist = attempt
            .previous()
            .last()
            .is_some_and(|previous| redirect_leaves_chatgpt_allowlist(previous, attempt.url()));

        if leaves_chatgpt_allowlist {
            attempt.stop()
        } else {
            Policy::default().redirect(attempt)
        }
    }))
}

fn redirect_leaves_chatgpt_allowlist(previous: &reqwest::Url, next: &reqwest::Url) -> bool {
    is_allowed_chatgpt_request_url(previous.as_str())
        && !is_allowed_chatgpt_request_url(next.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_secure_chatgpt_redirects_to_non_chatgpt_urls() {
        let previous =
            reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let external = reqwest::Url::parse("https://example.com/collect").unwrap();
        let downgraded =
            reqwest::Url::parse("http://chatgpt.com/backend-api/codex/responses").unwrap();

        assert!(redirect_leaves_chatgpt_allowlist(&previous, &external));
        assert!(redirect_leaves_chatgpt_allowlist(&previous, &downgraded));
    }

    #[test]
    fn permits_redirects_that_do_not_cross_the_chatgpt_boundary() {
        let chatgpt =
            reqwest::Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
        let preview =
            reqwest::Url::parse("https://preview.chatgpt.com/backend-api/codex/responses").unwrap();
        let external = reqwest::Url::parse("https://example.com/start").unwrap();

        assert!(!redirect_leaves_chatgpt_allowlist(&chatgpt, &preview));
        assert!(!redirect_leaves_chatgpt_allowlist(&external, &chatgpt));
    }
}
