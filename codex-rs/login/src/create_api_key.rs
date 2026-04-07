//! Browser-based OAuth flow for creating OpenAI project API keys.

use std::time::Duration;

use codex_client::build_reqwest_client_with_custom_ca;
use reqwest::Client;
use reqwest::Method;
use serde::Deserialize;
use url::Url;

use crate::oauth_callback_server::AuthorizationCodeServer;
use crate::oauth_callback_server::PortConflictStrategy;
use crate::oauth_callback_server::ShutdownHandle;
use crate::oauth_callback_server::start_authorization_code_server;
use crate::pkce::PkceCodes;

const AUTH_ISSUER: &str = "https://auth.openai.com";
const PLATFORM_HYDRA_CLIENT_ID: &str = "app_2SKx67EdpoN0G6j64rFvigXD";
const PLATFORM_AUDIENCE: &str = "https://api.openai.com/v1";
const API_BASE: &str = "https://api.openai.com";
// This client is registered with Hydra for http://localhost:5000/auth/callback,
// so the browser redirect must stay on port 5000.
const CALLBACK_PORT: u16 = 5000;
const CALLBACK_PATH: &str = "/auth/callback";
const SCOPE: &str = "openid email profile offline_access";
const APP: &str = "api";
const USER_AGENT: &str = "Codex-Create-API-Key/1.0";
const PROJECT_API_KEY_NAME: &str = "Codex CLI";
const PROJECT_POLL_INTERVAL_SECONDS: u64 = 10;
const PROJECT_POLL_TIMEOUT_SECONDS: u64 = 60;
const HTTP_TIMEOUT_SECONDS: u64 = 30;
pub const CREATE_API_KEY_OAUTH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
struct CreateApiKeyOptions {
    issuer: String,
    client_id: String,
    audience: String,
    api_base: String,
    app: String,
    callback_port: u16,
    scope: String,
    api_key_name: String,
    project_poll_interval_seconds: u64,
    project_poll_timeout_seconds: u64,
}

pub struct PendingCreateApiKey {
    client: Client,
    options: CreateApiKeyOptions,
    redirect_uri: String,
    code_verifier: String,
    callback_server: AuthorizationCodeServer,
}

impl PendingCreateApiKey {
    pub fn auth_url(&self) -> &str {
        &self.callback_server.auth_url
    }

    pub fn callback_port(&self) -> u16 {
        self.callback_server.actual_port
    }

    pub fn open_browser(&self) -> bool {
        self.callback_server.open_browser()
    }

    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.callback_server.shutdown_handle.clone()
    }

    pub async fn cancel(self) {
        self.callback_server.shutdown_and_wait().await;
    }

    pub async fn finish(self) -> Result<CreatedApiKey, CreateApiKeyError> {
        let code = self
            .callback_server
            .wait_for_code(CREATE_API_KEY_OAUTH_TIMEOUT)
            .await
            .map_err(|err| CreateApiKeyError::message(err.to_string()))?;
        create_api_key_from_authorization_code(
            &self.client,
            &self.options,
            &self.redirect_uri,
            &self.code_verifier,
            &code,
        )
        .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedApiKey {
    pub organization_id: String,
    pub organization_title: Option<String>,
    pub default_project_id: String,
    pub default_project_title: Option<String>,
    pub project_api_key: String,
}

pub fn start_create_api_key() -> Result<PendingCreateApiKey, CreateApiKeyError> {
    let options = CreateApiKeyOptions {
        issuer: AUTH_ISSUER.to_string(),
        client_id: PLATFORM_HYDRA_CLIENT_ID.to_string(),
        audience: PLATFORM_AUDIENCE.to_string(),
        api_base: API_BASE.to_string(),
        app: APP.to_string(),
        callback_port: CALLBACK_PORT,
        scope: SCOPE.to_string(),
        api_key_name: PROJECT_API_KEY_NAME.to_string(),
        project_poll_interval_seconds: PROJECT_POLL_INTERVAL_SECONDS,
        project_poll_timeout_seconds: PROJECT_POLL_TIMEOUT_SECONDS,
    };
    let client = build_http_client()?;
    let callback_server = start_authorization_code_server(
        options.callback_port,
        PortConflictStrategy::Fail,
        CALLBACK_PATH,
        /*force_state*/ None,
        |redirect_uri, pkce, state| {
            build_authorize_url(&options, redirect_uri, pkce, state)
                .map_err(|err| std::io::Error::other(err.to_string()))
        },
    )
    .map_err(|err| CreateApiKeyError::message(err.to_string()))?;
    let redirect_uri = callback_server.redirect_uri.clone();

    Ok(PendingCreateApiKey {
        client,
        options,
        redirect_uri,
        code_verifier: callback_server.code_verifier().to_string(),
        callback_server,
    })
}

fn build_authorize_url(
    options: &CreateApiKeyOptions,
    redirect_uri: &str,
    pkce: &PkceCodes,
    state: &str,
) -> Result<String, CreateApiKeyError> {
    let mut url = Url::parse(&format!(
        "{}/oauth/authorize",
        options.issuer.trim_end_matches('/')
    ))
    .map_err(|err| CreateApiKeyError::message(format!("invalid issuer URL: {err}")))?;
    url.query_pairs_mut()
        .append_pair("audience", &options.audience)
        .append_pair("client_id", &options.client_id)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &pkce.code_challenge)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &options.scope)
        .append_pair("state", state);
    Ok(url.to_string())
}

fn build_http_client() -> Result<Client, CreateApiKeyError> {
    build_reqwest_client_with_custom_ca(
        reqwest::Client::builder().timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS)),
    )
    .map_err(|err| CreateApiKeyError::message(format!("failed to build HTTP client: {err}")))
}

async fn create_api_key_from_authorization_code(
    client: &Client,
    options: &CreateApiKeyOptions,
    redirect_uri: &str,
    code_verifier: &str,
    code: &str,
) -> Result<CreatedApiKey, CreateApiKeyError> {
    let tokens = exchange_authorization_code_for_tokens(
        client,
        &options.issuer,
        &options.client_id,
        redirect_uri,
        code_verifier,
        code,
    )
    .await?;
    let login = onboarding_login(
        client,
        &options.api_base,
        &options.app,
        &tokens.access_token,
    )
    .await?;
    let target = wait_for_default_project(
        client,
        &options.api_base,
        &login.user.session.sensitive_id,
        options.project_poll_interval_seconds,
        options.project_poll_timeout_seconds,
    )
    .await?;
    let api_key = create_project_api_key(
        client,
        &options.api_base,
        &login.user.session.sensitive_id,
        &target,
        &options.api_key_name,
    )
    .await?
    .key
    .sensitive_id;

    Ok(CreatedApiKey {
        organization_id: target.organization_id,
        organization_title: target.organization_title,
        default_project_id: target.project_id,
        default_project_title: target.project_title,
        project_api_key: api_key,
    })
}

async fn exchange_authorization_code_for_tokens(
    client: &Client,
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    code_verifier: &str,
    code: &str,
) -> Result<OAuthTokens, CreateApiKeyError> {
    let url = format!("{}/oauth/token", issuer.trim_end_matches('/'));
    execute_json(
        client
            .request(Method::POST, &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .body(format!(
                "client_id={}&code_verifier={}&code={}&grant_type={}&redirect_uri={}",
                urlencoding::encode(client_id),
                urlencoding::encode(code_verifier),
                urlencoding::encode(code),
                urlencoding::encode("authorization_code"),
                urlencoding::encode(redirect_uri)
            )),
        "POST",
        &url,
    )
    .await
}

async fn onboarding_login(
    client: &Client,
    api_base: &str,
    app: &str,
    access_token: &str,
) -> Result<OnboardingLoginResponse, CreateApiKeyError> {
    let url = format!(
        "{}/dashboard/onboarding/login",
        api_base.trim_end_matches('/')
    );
    execute_json(
        client
            .request(Method::POST, &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .bearer_auth(access_token)
            .json(&serde_json::json!({ "app": app })),
        "POST",
        &url,
    )
    .await
}

async fn list_organizations(
    client: &Client,
    api_base: &str,
    session_key: &str,
) -> Result<Vec<Organization>, CreateApiKeyError> {
    let url = format!("{}/v1/organizations", api_base.trim_end_matches('/'));
    let response: DataList<Organization> = execute_json(
        client
            .request(Method::GET, &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .bearer_auth(session_key),
        "GET",
        &url,
    )
    .await?;
    Ok(response.data)
}

async fn list_projects(
    client: &Client,
    api_base: &str,
    session_key: &str,
    organization_id: &str,
) -> Result<Vec<Project>, CreateApiKeyError> {
    let url = format!(
        "{}/dashboard/organizations/{}/projects?detail=basic&limit=100",
        api_base.trim_end_matches('/'),
        urlencoding::encode(organization_id)
    );
    let response: DataList<Project> = execute_json(
        client
            .request(Method::GET, &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header("openai-organization", organization_id)
            .bearer_auth(session_key),
        "GET",
        &url,
    )
    .await?;
    Ok(response.data)
}

async fn wait_for_default_project(
    client: &Client,
    api_base: &str,
    session_key: &str,
    poll_interval_seconds: u64,
    timeout_seconds: u64,
) -> Result<ProjectApiKeyTarget, CreateApiKeyError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        let organizations = list_organizations(client, api_base, session_key).await?;
        let last_state = if organizations.is_empty() {
            "no organization found".to_string()
        } else {
            let ordered_organizations = organizations_by_preference(&organizations);
            let mut project_count = 0;
            for organization in ordered_organizations {
                let projects =
                    list_projects(client, api_base, session_key, &organization.id).await?;
                project_count += projects.len();
                if let Some(project) = find_default_project(&projects) {
                    return Ok(ProjectApiKeyTarget {
                        organization_id: organization.id.clone(),
                        organization_title: organization.title.clone(),
                        project_id: project.id.clone(),
                        project_title: project.title.clone(),
                    });
                }
            }
            format!(
                "checked {} organizations and {} projects, but no default project is ready yet.",
                organizations.len(),
                project_count
            )
        };

        if std::time::Instant::now() >= deadline {
            return Err(CreateApiKeyError::message(format!(
                "Timed out waiting for an organization and default project. Last observed state: {last_state}"
            )));
        }
        let remaining_seconds = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_secs();
        let sleep_seconds = poll_interval_seconds.min(remaining_seconds.max(1));
        tokio::time::sleep(Duration::from_secs(sleep_seconds)).await;
    }
}

fn organizations_by_preference(organizations: &[Organization]) -> Vec<&Organization> {
    let mut ordered_organizations = organizations.iter().enumerate().collect::<Vec<_>>();
    ordered_organizations.sort_by_key(|(index, organization)| {
        let rank = if organization.is_default {
            0
        } else if organization.personal {
            1
        } else {
            2
        };
        (rank, *index)
    });
    ordered_organizations
        .into_iter()
        .map(|(_, organization)| organization)
        .collect()
}

fn find_default_project(projects: &[Project]) -> Option<&Project> {
    projects.iter().find(|project| project.is_initial)
}

async fn create_project_api_key(
    client: &Client,
    api_base: &str,
    session_key: &str,
    target: &ProjectApiKeyTarget,
    key_name: &str,
) -> Result<CreateProjectApiKeyResponse, CreateApiKeyError> {
    let url = format!(
        "{}/dashboard/organizations/{}/projects/{}/api_keys",
        api_base.trim_end_matches('/'),
        urlencoding::encode(&target.organization_id),
        urlencoding::encode(&target.project_id)
    );
    execute_json(
        client
            .request(Method::POST, &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .bearer_auth(session_key)
            .json(&serde_json::json!({
                "action": "create",
                "name": key_name,
            })),
        "POST",
        &url,
    )
    .await
}

async fn execute_json<T>(
    request: reqwest::RequestBuilder,
    method: &str,
    url: &str,
) -> Result<T, CreateApiKeyError>
where
    T: for<'de> Deserialize<'de>,
{
    let response = request
        .send()
        .await
        .map_err(|err| CreateApiKeyError::message(format!("Network error calling {url}: {err}")))?;
    let status = response.status();
    let body = response.bytes().await.map_err(|err| {
        CreateApiKeyError::message(format!("Failed reading response from {url}: {err}"))
    })?;
    if !status.is_success() {
        return Err(CreateApiKeyError::api(
            format!("{method} {url} failed with HTTP {status}"),
            String::from_utf8_lossy(&body).into_owned(),
        ));
    }
    serde_json::from_slice(&body)
        .map_err(|err| CreateApiKeyError::message(format!("{url} returned invalid JSON: {err}")))
}

#[derive(Debug, Deserialize)]
struct OAuthTokens {
    #[serde(rename = "id_token")]
    _id_token: String,
    access_token: String,
    #[serde(rename = "refresh_token")]
    _refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct OnboardingLoginResponse {
    user: OnboardingUser,
}

#[derive(Debug, Deserialize)]
struct OnboardingUser {
    session: OnboardingSession,
}

#[derive(Debug, Deserialize)]
struct OnboardingSession {
    sensitive_id: String,
}

#[derive(Debug, Deserialize)]
struct DataList<T> {
    data: Vec<T>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct Organization {
    id: String,
    title: Option<String>,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    personal: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct Project {
    id: String,
    title: Option<String>,
    #[serde(default)]
    is_initial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectApiKeyTarget {
    organization_id: String,
    organization_title: Option<String>,
    project_id: String,
    project_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateProjectApiKeyResponse {
    key: CreatedProjectApiKey,
}

#[derive(Debug, Deserialize)]
struct CreatedProjectApiKey {
    sensitive_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateApiKeyError {
    message: String,
}

impl CreateApiKeyError {
    fn message(message: String) -> Self {
        Self { message }
    }

    fn api(message: String, body: String) -> Self {
        Self {
            message: format!("{message}: {body}"),
        }
    }
}

impl std::fmt::Display for CreateApiKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CreateApiKeyError {}

#[cfg(test)]
#[path = "create_api_key_tests.rs"]
mod tests;
