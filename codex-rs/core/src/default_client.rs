use reqwest::header::HeaderValue;
use std::sync::LazyLock;

pub const CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";

#[derive(Debug, Clone)]
pub struct Originator {
    pub value: String,
    pub header_value: HeaderValue,
}

pub static ORIGINATOR: LazyLock<Originator> = LazyLock::new(|| {
    let default = "codex_cli_rs";
    let value = std::env::var(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
        .unwrap_or_else(|_| default.to_string());

    match HeaderValue::from_str(&value) {
        Ok(header_value) => Originator {
            value,
            header_value,
        },
        Err(e) => {
            tracing::error!("Unable to turn originator override {value} into header value: {e}");
            Originator {
                value: default.to_string(),
                header_value: HeaderValue::from_static(default),
            }
        }
    }
});

pub fn get_codex_user_agent() -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();
    format!(
        "{}/{build_version} ({} {}; {}) {}",
        ORIGINATOR.value.as_str(),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        crate::terminal::user_agent()
    )
}

/// Create a reqwest client with default `originator` and `User-Agent` headers set.
pub fn create_client() -> reqwest::Client {
    use reqwest::header::HeaderMap;

    let mut headers = HeaderMap::new();
    headers.insert("originator", ORIGINATOR.header_value.clone());
    let ua = get_codex_user_agent();

    reqwest::Client::builder()
        // Set UA via dedicated helper to avoid header validation pitfalls
        .user_agent(ua)
        .default_headers(headers)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_codex_user_agent() {
        let user_agent = get_codex_user_agent();
        assert!(user_agent.starts_with("codex_cli_rs/"));
    }

    #[tokio::test]
    async fn test_create_client_sets_default_headers() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let client = create_client();

        // Spin up a local mock server and capture a request.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let resp = client
            .get(server.uri())
            .send()
            .await
            .expect("failed to send request");
        assert!(resp.status().is_success());

        let requests = server
            .received_requests()
            .await
            .expect("failed to fetch received requests");
        assert!(!requests.is_empty());
        let headers = &requests[0].headers;

        // originator header is set to the provided value
        let originator_header = headers
            .get("originator")
            .expect("originator header missing");
        assert_eq!(originator_header.to_str().unwrap(), "codex_cli_rs");

        // User-Agent matches the computed Codex UA for that originator
        let expected_ua = get_codex_user_agent();
        let ua_header = headers
            .get("user-agent")
            .expect("user-agent header missing");
        assert_eq!(ua_header.to_str().unwrap(), expected_ua);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos() {
        use regex_lite::Regex;
        let user_agent = get_codex_user_agent();
        let re = Regex::new(
            r"^codex_cli_rs/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$",
        )
        .unwrap();
        assert!(re.is_match(&user_agent));
    }
}
