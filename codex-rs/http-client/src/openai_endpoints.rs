//! Classify OpenAI API destinations without treating an unsupported origin as another provider.

use crate::is_allowed_chatgpt_host;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenAiDestination {
    Supported,
    Unsupported,
    Other,
}

pub fn classify_openai_api_url(destination: &str) -> OpenAiDestination {
    let Ok(url) = reqwest::Url::parse(destination) else {
        return OpenAiDestination::Other;
    };
    let Some(host) = url.host_str() else {
        return OpenAiDestination::Other;
    };
    let host = host.strip_suffix('.').unwrap_or(host);
    let openai_owned = [
        "openai.com",
        "openai.org",
        "openai-staging.com",
        "chatgpt.com",
        "chatgpt-staging.com",
    ]
    .iter()
    .any(|domain| {
        let host = host.trim_end_matches('.');
        host == *domain
            || host
                .strip_suffix(*domain)
                .is_some_and(|prefix| prefix.ends_with('.'))
    });
    if !openai_owned {
        return OpenAiDestination::Other;
    }
    let supported_host = matches!(
        host,
        "api.openai.com"
            | "gov.api.openai.com"
            | "us.api.openai.com"
            | "eu.api.openai.com"
            | "au.api.openai.com"
            | "ca.api.openai.com"
            | "jp.api.openai.com"
            | "in.api.openai.com"
            | "sg.api.openai.com"
            | "kr.api.openai.com"
            | "gb.api.openai.com"
            | "ae.api.openai.com"
            | "mtls.api.openai.com"
            | "mtls-us.api.openai.com"
            | "mtls-eu.api.openai.com"
            | "api.openai.org"
            | "api.openai-staging.com"
    ) || is_allowed_chatgpt_host(host);
    if supported_host
        && url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
    {
        OpenAiDestination::Supported
    } else {
        OpenAiDestination::Unsupported
    }
}

pub fn is_openai_api_url(destination: &str) -> bool {
    classify_openai_api_url(destination) == OpenAiDestination::Supported
}

#[cfg(test)]
mod tests {
    use super::OpenAiDestination;
    use super::classify_openai_api_url;
    use super::is_openai_api_url;

    #[test]
    fn unsupported_openai_origins_are_distinct_from_other_providers() {
        for destination in [
            "https://unknown.api.openai.com/v1",
            "https://api.openai.com../v1",
            "https://api.openai.com:8443/v1",
            "http://api.openai.com/v1",
            "https://user@api.openai.com/v1",
            "https://other.openai.org/v1",
        ] {
            assert_eq!(
                classify_openai_api_url(destination),
                OpenAiDestination::Unsupported,
                "{destination}"
            );
        }
        for destination in [
            "https://api.openai.com.example.test/v1",
            "https://example.test/v1",
            "http://127.0.0.1:8080/v1",
        ] {
            assert_eq!(
                classify_openai_api_url(destination),
                OpenAiDestination::Other,
                "{destination}"
            );
        }
    }

    #[test]
    fn runtime_credentials_require_a_trusted_https_origin() {
        for destination in [
            "https://api.openai.com/v1",
            "https://api.openai.com.:443/v1",
            "https://gov.api.openai.com/v1",
            "https://eu.api.openai.com/v1",
            "https://us.api.openai.com/v1",
            "https://au.api.openai.com/v1",
            "https://ca.api.openai.com/v1",
            "https://jp.api.openai.com/v1",
            "https://in.api.openai.com/v1",
            "https://sg.api.openai.com/v1",
            "https://kr.api.openai.com/v1",
            "https://gb.api.openai.com/v1",
            "https://ae.api.openai.com/v1",
            "https://mtls.api.openai.com/v1",
            "https://mtls-us.api.openai.com/v1",
            "https://mtls-eu.api.openai.com/v1",
            "https://api.openai.org/v1",
            "https://api.openai-staging.com/v1",
            "https://chatgpt.com/backend-api",
            "https://chat.openai.com/backend-api",
            "https://us.chatgpt.com/backend-api",
            "https://chatgpt-staging.com/backend-api",
            "https://us.chatgpt-staging.com/backend-api",
        ] {
            assert!(is_openai_api_url(destination), "{destination}");
        }
        for destination in [
            "http://api.openai.com/v1",
            "https://api.openai.com:8443/v1",
            "https://user@api.openai.com/v1",
            "https://user:password@chatgpt.com/backend-api",
            "https://api.openai.com.example.test/v1",
            "https://eu.api.openai.com.example.test/v1",
            "https://unknown.api.openai.com/v1",
            "https://evilchatgpt.com/backend-api",
            "https://chatgpt.com.example.test/backend-api",
            "https://foo.chat.openai.com/backend-api",
            "http://127.0.0.1:8080/v1",
            "https://example.test/v1",
        ] {
            assert!(!is_openai_api_url(destination), "{destination}");
        }
    }
}
