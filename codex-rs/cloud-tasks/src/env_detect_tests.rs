use std::collections::HashMap;
use std::sync::Mutex;

use http::HeaderMap;
use http::StatusCode;
use pretty_assertions::assert_eq;

use super::*;

const BASE_URL: &str = "https://chatgpt.com/backend-api";
const BY_REPO_URL: &str =
    "https://chatgpt.com/backend-api/wham/environments/by-repo/github/openai/codex";
const GLOBAL_URL: &str = "https://chatgpt.com/backend-api/wham/environments";

#[tokio::test]
async fn autodetect_requests_exact_repository_endpoint_and_decodes_selection() {
    let http = FakeHttp::new(HashMap::from([(
        BY_REPO_URL.to_string(),
        json_response(r#"[{"id":"env-repo","label":"Repository","is_pinned":true}]"#),
    )]));

    let selection = autodetect_environment_id_with_origins(
        &http,
        BASE_URL,
        &HeaderMap::new(),
        Some("Repository".to_string()),
        &["git@github.com:openai/codex.git".to_string()],
    )
    .await
    .expect("repository environment should be selected");

    assert_eq!(
        selection,
        AutodetectSelection {
            id: "env-repo".to_string(),
            label: Some("Repository".to_string()),
        }
    );
    assert_eq!(http.requested_urls(), vec![BY_REPO_URL.to_string()]);
}

#[tokio::test]
async fn list_requests_exact_repository_and_global_endpoints_and_merges_results() {
    let http = FakeHttp::new(HashMap::from([
        (
            BY_REPO_URL.to_string(),
            json_response(r#"[{"id":"env-repo","label":"Repository"}]"#),
        ),
        (
            GLOBAL_URL.to_string(),
            json_response(
                r#"[{"id":"env-repo","is_pinned":true},{"id":"env-global","label":"Global"}]"#,
            ),
        ),
    ]));

    let rows = list_environments_with_origins(
        &http,
        BASE_URL,
        &HeaderMap::new(),
        &["https://github.com/openai/codex.git".to_string()],
    )
    .await
    .expect("environment list should decode");

    assert_eq!(
        rows.into_iter()
            .map(|row| (row.id, row.label, row.is_pinned, row.repo_hints))
            .collect::<Vec<_>>(),
        vec![
            (
                "env-repo".to_string(),
                Some("Repository".to_string()),
                true,
                Some("openai/codex".to_string()),
            ),
            (
                "env-global".to_string(),
                Some("Global".to_string()),
                false,
                None,
            ),
        ]
    );
    assert_eq!(
        http.requested_urls(),
        vec![BY_REPO_URL.to_string(), GLOBAL_URL.to_string()]
    );
}

struct FakeHttp {
    responses: HashMap<String, EnvironmentResponse>,
    requested_urls: Mutex<Vec<String>>,
}

impl FakeHttp {
    fn new(responses: HashMap<String, EnvironmentResponse>) -> Self {
        Self {
            responses,
            requested_urls: Mutex::new(Vec::new()),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requested_urls
            .lock()
            .expect("requested URL lock")
            .clone()
    }
}

impl EnvironmentHttp for FakeHttp {
    async fn get(&self, url: &str, _headers: &HeaderMap) -> anyhow::Result<EnvironmentResponse> {
        self.requested_urls
            .lock()
            .expect("requested URL lock")
            .push(url.to_string());
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected URL: {url}"))
    }
}

fn json_response(body: &str) -> EnvironmentResponse {
    EnvironmentResponse {
        status: StatusCode::OK,
        content_type: "application/json".to_string(),
        body: body.to_string(),
    }
}
