//! Statsig integration helpers for Codex.

use async_trait::async_trait;
use codex_login::CodexAuth;
use statsig_rust::DynamicValue;
use statsig_rust::EventLoggingAdapter;
use statsig_rust::StatsigErr;
use statsig_rust::StatsigRuntime;
use statsig_rust::StatsigUserData;
use statsig_rust::compression::compression_helper::compress_data;
use statsig_rust::compression::compression_helper::get_compression_format;
use statsig_rust::dyn_value;
use statsig_rust::log_event_payload::LogEventRequest;
use statsig_rust::networking::NetworkClient;
use statsig_rust::networking::NetworkError;
use statsig_rust::networking::RequestArgs;
use statsig_rust::networking::ResponseData;
use statsig_rust::statsig_metadata::StatsigMetadata;
use std::collections::HashMap;
use std::sync::Arc;

pub use statsig_rust::Statsig;
pub use statsig_rust::StatsigOptions;
pub use statsig_rust::StatsigUser;

pub const DEFAULT_CES_STATSIG_LOG_EVENT_URL: &str = "https://chatgpt.com/ces/v1/rgstr";
pub const AUTHORIZATION_HEADER_NAME: &str = "Authorization";
pub const CHATGPT_ACCOUNT_ID_HEADER_NAME: &str = "ChatGPT-Account-Id";

pub mod custom_keys {
    pub const AUTH_STATUS: &str = "auth_status";
    pub const PLAN_TYPE: &str = "plan_type";
    pub const WORKSPACE_ID: &str = "workspace_id";
    pub const ACCOUNT_ID: &str = "account_id";
    pub const USER_AGENT: &str = "user_agent";
    pub const EMAIL_DOMAIN_TYPE: &str = "email_domain_type";
}

pub mod custom_id_keys {
    pub const WORKSPACE_ID: &str = "workspace_id";
    pub const ACCOUNT_ID: &str = "account_id";
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatsigUserAuthStatus {
    #[default]
    LoggedOut,
    LoggedIn,
}

impl StatsigUserAuthStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::LoggedOut => "logged_out",
            Self::LoggedIn => "logged_in",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatsigUserMetadata {
    pub auth_status: StatsigUserAuthStatus,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub plan_type: Option<String>,
    pub is_workspace_account: bool,
    pub app_version: Option<String>,
    pub user_agent: Option<String>,
    pub statsig_environment: Option<HashMap<String, String>>,
}

impl StatsigUserMetadata {
    pub fn from_auth(auth: Option<&CodexAuth>) -> Self {
        let token_data = auth.and_then(|auth| auth.get_token_data().ok());
        let user_id = token_data
            .as_ref()
            .and_then(|token_data| token_data.id_token.chatgpt_user_id.clone());

        Self {
            auth_status: if user_id.is_some() {
                StatsigUserAuthStatus::LoggedIn
            } else {
                StatsigUserAuthStatus::LoggedOut
            },
            user_id,
            email: token_data
                .as_ref()
                .and_then(|token_data| token_data.id_token.email.clone()),
            account_id: token_data
                .as_ref()
                .and_then(|token_data| token_data.account_id.clone()),
            plan_type: token_data
                .as_ref()
                .and_then(|token_data| token_data.id_token.get_chatgpt_plan_type_raw()),
            is_workspace_account: token_data
                .as_ref()
                .is_some_and(|token_data| token_data.id_token.is_workspace_account()),
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalyticsMode {
    Enabled,
    Disabled,
}

impl AnalyticsMode {
    fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

pub fn make_statsig_options(
    sdk_key: &str,
    auth: Option<&CodexAuth>,
    analytics_mode: AnalyticsMode,
) -> StatsigOptions {
    make_statsig_options_with_log_event_url(
        sdk_key,
        auth,
        analytics_mode,
        DEFAULT_CES_STATSIG_LOG_EVENT_URL,
    )
}

pub fn make_statsig_options_with_log_event_url(
    sdk_key: &str,
    auth: Option<&CodexAuth>,
    analytics_mode: AnalyticsMode,
    log_event_url: impl Into<String>,
) -> StatsigOptions {
    let log_event_url = log_event_url.into();
    let analytics_disabled = !analytics_mode.is_enabled();
    let mut options = StatsigOptions::new();
    options.log_event_url = Some(log_event_url.clone());
    options.disable_network = Some(analytics_disabled);
    options.disable_all_logging = Some(analytics_disabled);
    options.event_logging_adapter = Some(Arc::new(CodexCesEventLoggingAdapter::new(
        sdk_key,
        &options,
        auth,
        log_event_url,
    )));
    options
}

pub fn make_statsig_user(metadata: StatsigUserMetadata) -> StatsigUser {
    let mut custom = HashMap::from([
        (
            custom_keys::AUTH_STATUS.to_string(),
            dyn_value!(metadata.auth_status.as_str()),
        ),
        (
            custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
            dyn_value!(get_email_domain_type(
                metadata.email.as_deref().unwrap_or_default()
            )),
        ),
    ]);

    if let Some(plan_type) = metadata.plan_type.as_ref() {
        custom.insert(custom_keys::PLAN_TYPE.to_string(), dyn_value!(plan_type));
    }

    if let Some(account_id) = metadata.account_id.as_ref() {
        custom.insert(custom_keys::ACCOUNT_ID.to_string(), dyn_value!(account_id));
        if metadata.is_workspace_account {
            custom.insert(
                custom_keys::WORKSPACE_ID.to_string(),
                dyn_value!(account_id),
            );
        }
    }

    if let Some(user_agent) = metadata.user_agent.as_ref() {
        custom.insert(custom_keys::USER_AGENT.to_string(), dyn_value!(user_agent));
    }

    let custom_ids = metadata.account_id.as_ref().map(|account_id| {
        HashMap::from([
            (
                custom_id_keys::ACCOUNT_ID.to_string(),
                dyn_value!(account_id),
            ),
            (
                custom_id_keys::WORKSPACE_ID.to_string(),
                dyn_value!(account_id),
            ),
        ])
    });

    let statsig_environment = metadata
        .statsig_environment
        .map(dynamic_value_map_from_string_map);

    StatsigUser::new(StatsigUserData {
        user_id: metadata.user_id.map(|user_id| dyn_value!(user_id)),
        custom_ids,
        user_agent: metadata.user_agent.map(|user_agent| dyn_value!(user_agent)),
        app_version: metadata
            .app_version
            .map(|app_version| dyn_value!(app_version)),
        statsig_environment,
        custom: Some(custom),
        ..StatsigUserData::default()
    })
}

pub fn get_email_domain_type(email: &str) -> &'static str {
    let email = email.trim();
    if email.is_empty() {
        return "missing";
    }

    let Some((_, domain)) = email.rsplit_once('@') else {
        return "unknown";
    };
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() || !domain.contains('.') {
        return "unknown";
    }

    if is_social_email_domain(&domain) {
        return "social";
    }
    if is_government_email_domain(&domain) {
        return "government";
    }
    if is_education_email_domain(&domain) {
        return "edu";
    }
    "professional"
}

fn dynamic_value_map_from_string_map(
    map: HashMap<String, String>,
) -> HashMap<String, DynamicValue> {
    map.into_iter()
        .map(|(key, value)| (key, dyn_value!(value)))
        .collect()
}

fn is_social_email_domain(domain: &str) -> bool {
    const SOCIAL_DOMAINS: &[&str] = &[
        "gmail.com",
        "googlemail.com",
        "icloud.com",
        "me.com",
        "mac.com",
        "msn.com",
        "pm.me",
        "proton.me",
        "protonmail.com",
        "fastmail.com",
        "hey.com",
        "zoho.com",
        "zohomail.com",
        "yandex.com",
        "qq.com",
        "163.com",
        "naver.com",
        "daum.net",
    ];
    if SOCIAL_DOMAINS.contains(&domain) {
        return true;
    }

    let Some((provider, suffix)) = domain.split_once('.') else {
        return false;
    };
    matches!(
        provider,
        "yahoo"
            | "hotmail"
            | "outlook"
            | "live"
            | "email"
            | "online"
            | "mail"
            | "posteo"
            | "gmx"
            | "aol"
    ) && suffix.split('.').all(|part| (2..=3).contains(&part.len()))
}

fn is_government_email_domain(domain: &str) -> bool {
    domain.split('.').any(|part| part == "gov")
}

fn is_education_email_domain(domain: &str) -> bool {
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.contains(&"edu") {
        return true;
    }
    if parts
        .windows(2)
        .any(|window| window[0] == "ac" && window[1].len() == 2)
    {
        return true;
    }
    if domain.ends_with(".k12.us")
        || domain.ends_with(".k12.tr")
        || domain.ends_with(".sch.id")
        || domain.ends_with(".sch.uk")
        || domain.ends_with(".ac.jp")
        || domain.ends_with(".ed.jp")
    {
        return true;
    }

    let Some(first_label) = parts.first() else {
        return false;
    };
    matches!(
        *first_label,
        "student" | "students" | "stu" | "edu" | "educa" | "education" | "educacion" | "educar"
    )
}

fn statsig_ces_headers(
    sdk_key: &str,
    options: &StatsigOptions,
    auth: Option<&CodexAuth>,
) -> HashMap<String, String> {
    let mut headers =
        StatsigMetadata::get_constant_request_headers(sdk_key, options.service_name.as_deref());

    let Some(auth) = auth.filter(|auth| auth.is_chatgpt_auth()) else {
        return headers;
    };
    let Ok(token) = auth.get_token() else {
        return headers;
    };

    headers.insert(
        AUTHORIZATION_HEADER_NAME.to_string(),
        format!("Bearer {token}"),
    );
    if let Some(account_id) = auth.get_account_id() {
        headers.insert(CHATGPT_ACCOUNT_ID_HEADER_NAME.to_string(), account_id);
    }
    headers
}

struct CodexCesEventLoggingAdapter {
    log_event_url: String,
    network: NetworkClient,
}

impl CodexCesEventLoggingAdapter {
    fn new(
        sdk_key: &str,
        options: &StatsigOptions,
        auth: Option<&CodexAuth>,
        log_event_url: String,
    ) -> Self {
        Self {
            log_event_url,
            network: NetworkClient::new(
                sdk_key,
                Some(statsig_ces_headers(sdk_key, options, auth)),
                Some(options),
            ),
        }
    }

    async fn send_events_over_http(&self, request: &LogEventRequest) -> Result<(), StatsigErr> {
        let compression_format = get_compression_format();
        let headers = HashMap::from([
            (
                "statsig-event-count".to_string(),
                request.event_count.to_string(),
            ),
            (
                "statsig-retry-count".to_string(),
                request.retries.to_string(),
            ),
            ("Content-Encoding".to_owned(), compression_format),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ]);

        let bytes = serde_json::to_vec(&request.payload)
            .map_err(|e| StatsigErr::SerializationError(e.to_string()))?;
        let compressed = compress_data(&bytes)?;
        let response = self
            .network
            .post(
                RequestArgs {
                    url: self.log_event_url.clone(),
                    headers: Some(headers),
                    accept_gzip_response: true,
                    ..RequestArgs::new()
                },
                Some(compressed),
            )
            .await
            .map_err(StatsigErr::NetworkError)?;

        let Some(res_data) = response.data else {
            return Err(StatsigErr::NetworkError(NetworkError::RequestFailed(
                self.log_event_url.clone(),
                response.status_code,
                "Empty response from network".to_string(),
            )));
        };
        ensure_log_event_response_success(res_data)?;
        Ok(())
    }
}

fn ensure_log_event_response_success(mut res_data: ResponseData) -> Result<(), StatsigErr> {
    let bytes = res_data.read_to_bytes()?;
    if bytes.is_empty() {
        return Ok(());
    }

    let result = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| StatsigErr::JsonParseError("LogEventResult".to_string(), e.to_string()))?;
    if result.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err(StatsigErr::LogEventError(
            "Unsuccessful response from network".into(),
        ));
    }
    Ok(())
}

#[async_trait]
impl EventLoggingAdapter for CodexCesEventLoggingAdapter {
    async fn start(&self, _statsig_runtime: &Arc<StatsigRuntime>) -> Result<(), StatsigErr> {
        Ok(())
    }

    async fn log_events(&self, request: LogEventRequest) -> Result<bool, StatsigErr> {
        self.send_events_over_http(&request).await.map(|()| true)
    }

    async fn shutdown(&self) -> Result<(), StatsigErr> {
        self.network.shutdown();
        Ok(())
    }

    fn should_schedule_background_flush(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests;
