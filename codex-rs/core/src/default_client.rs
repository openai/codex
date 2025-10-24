pub const DEFAULT_ORIGINATOR: &str = "codex_cli_rs";

pub fn get_codex_user_agent(originator: Option<&str>) -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();
    format!(
        "{}/{build_version} ({} {}; {}) {}",
        originator.unwrap_or(DEFAULT_ORIGINATOR),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        crate::terminal::user_agent()
    )
}

/// Create a reqwest client with default `originator` and `User-Agent` headers set.
pub fn create_client(originator: &str) -> reqwest::Client {
    use reqwest::header::HeaderMap;
    use reqwest::header::HeaderValue;

    let mut headers = HeaderMap::new();
    let originator_value = HeaderValue::from_str(originator)
        .unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_ORIGINATOR));
    headers.insert("originator", originator_value);
    let ua = get_codex_user_agent(Some(originator));

    match reqwest::Client::builder()
        // Set UA via dedicated helper to avoid header validation pitfalls
        .user_agent(ua)
        .default_headers(headers)
        .build()
    {
        Ok(client) => client,
        Err(_) => reqwest::Client::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_codex_user_agent() {
        let user_agent = get_codex_user_agent(None);
        assert!(user_agent.starts_with("codex_cli_rs/"));
    }

    #[tokio::test]
    async fn test_create_client_sets_default_headers() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let originator = "test_originator";
        let client = create_client(originator);

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
        assert_eq!(originator_header.to_str().unwrap(), originator);

        // User-Agent matches the computed Codex UA for that originator
        let expected_ua = get_codex_user_agent(Some(originator));
        let ua_header = headers
            .get("user-agent")
            .expect("user-agent header missing");
        assert_eq!(ua_header.to_str().unwrap(), expected_ua);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos() {
        use regex_lite::Regex;
        let user_agent = get_codex_user_agent(None);
        let re = Regex::new(
            r"^codex_cli_rs/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$",
        )
        .unwrap();
        assert!(re.is_match(&user_agent));
    }
}
