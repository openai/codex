use crate::types::CodeTaskDetailsResponse;
use crate::types::ConfigFileResponse;
use crate::types::PaginatedListTaskListItem;
use crate::types::RateLimitStatusPayload;
use crate::types::TurnAttemptsSiblingTurnsResponse;
use anyhow::Result;
use codex_core::auth::CodexAuth;
use codex_core::default_client::get_codex_user_agent;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::protocol::CreditsSnapshot;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathStyle {
    /// /api/codex/…
    CodexApi,
    /// /wham/…
    ChatGptApi,
}

impl PathStyle {
    pub fn from_base_url(base_url: &str) -> Self {
        if base_url.contains("/backend-api") {
            PathStyle::ChatGptApi
        } else {
            PathStyle::CodexApi
        }
    }
}

#[derive(Clone, Debug)]
pub struct Client {
    base_url: String,
    http: reqwest::Client,
    bearer_token: Option<String>,
    user_agent: Option<HeaderValue>,
    chatgpt_account_id: Option<String>,
    path_style: PathStyle,
}

impl Client {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let mut base_url = base_url.into();
        // Normalize common ChatGPT hostnames to include /backend-api so we hit the WHAM paths.
        // Also trim trailing slashes for consistent URL building.
        while base_url.ends_with('/') {
            base_url.pop();
        }
        if (base_url.starts_with("https://chatgpt.com")
            || base_url.starts_with("https://chat.openai.com"))
            && !base_url.contains("/backend-api")
        {
            base_url = format!("{base_url}/backend-api");
        }
        let http = reqwest::Client::builder().build()?;
        let path_style = PathStyle::from_base_url(&base_url);
        Ok(Self {
            base_url,
            http,
            bearer_token: None,
            user_agent: None,
            chatgpt_account_id: None,
            path_style,
        })
    }

    pub fn from_auth(base_url: impl Into<String>, auth: &CodexAuth) -> Result<Self> {
        let token = auth.get_token().map_err(anyhow::Error::from)?;
        let mut client = Self::new(base_url)?
            .with_user_agent(get_codex_user_agent())
            .with_bearer_token(token);
        if let Some(account_id) = auth.get_account_id() {
            client = client.with_chatgpt_account_id(account_id);
        }
        Ok(client)
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        if let Ok(hv) = HeaderValue::from_str(&ua.into()) {
            self.user_agent = Some(hv);
        }
        self
    }

    pub fn with_chatgpt_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.chatgpt_account_id = Some(account_id.into());
        self
    }

    pub fn with_path_style(mut self, style: PathStyle) -> Self {
        self.path_style = style;
        self
    }

    fn headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(ua) = &self.user_agent {
            h.insert(USER_AGENT, ua.clone());
        } else {
            h.insert(USER_AGENT, HeaderValue::from_static("codex-cli"));
        }
        if let Some(token) = &self.bearer_token {
            let value = format!("Bearer {token}");
            if let Ok(hv) = HeaderValue::from_str(&value) {
                h.insert(AUTHORIZATION, hv);
            }
        }
        if let Some(acc) = &self.chatgpt_account_id
            && let Ok(name) = HeaderName::from_bytes(b"ChatGPT-Account-Id")
            && let Ok(hv) = HeaderValue::from_str(acc)
        {
            h.insert(name, hv);
        }
        h
    }

    async fn exec_request(
        &self,
        req: reqwest::RequestBuilder,
        method: &str,
        url: &str,
    ) -> Result<(String, String)> {
        let res = req.send().await?;
        let status = res.status();
        let ct = res
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("{method} {url} failed: {status}; content-type={ct}; body={body}");
        }
        Ok((body, ct))
    }

    fn decode_json<T: DeserializeOwned>(&self, url: &str, ct: &str, body: &str) -> Result<T> {
        match serde_json::from_str::<T>(body) {
            Ok(v) => Ok(v),
            Err(e) => {
                anyhow::bail!("Decode error for {url}: {e}; content-type={ct}; body={body}");
            }
        }
    }

    pub async fn get_rate_limits(&self) -> Result<RateLimitSnapshot> {
        let snapshots = self.get_rate_limits_many().await?;
        let preferred = snapshots
            .iter()
            .find(|snapshot| snapshot.limit_id.as_deref() == Some("codex"))
            .cloned();
        Ok(preferred.unwrap_or_else(|| snapshots[0].clone()))
    }

    pub async fn get_rate_limits_many(&self) -> Result<Vec<RateLimitSnapshot>> {
        let url = match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/usage", self.base_url),
            PathStyle::ChatGptApi => format!("{}/wham/usage", self.base_url),
        };
        let req = self.http.get(&url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        let payload: RateLimitStatusPayload = self.decode_json(&url, &ct, &body)?;
        Ok(Self::rate_limit_snapshots_from_payload(payload))
    }

    pub async fn list_tasks(
        &self,
        limit: Option<i32>,
        task_filter: Option<&str>,
        environment_id: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<PaginatedListTaskListItem> {
        let url = match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/tasks/list", self.base_url),
            PathStyle::ChatGptApi => format!("{}/wham/tasks/list", self.base_url),
        };
        let req = self.http.get(&url).headers(self.headers());
        let req = if let Some(lim) = limit {
            req.query(&[("limit", lim)])
        } else {
            req
        };
        let req = if let Some(tf) = task_filter {
            req.query(&[("task_filter", tf)])
        } else {
            req
        };
        let req = if let Some(c) = cursor {
            req.query(&[("cursor", c)])
        } else {
            req
        };
        let req = if let Some(id) = environment_id {
            req.query(&[("environment_id", id)])
        } else {
            req
        };
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        self.decode_json::<PaginatedListTaskListItem>(&url, &ct, &body)
    }

    pub async fn get_task_details(&self, task_id: &str) -> Result<CodeTaskDetailsResponse> {
        let (parsed, _body, _ct) = self.get_task_details_with_body(task_id).await?;
        Ok(parsed)
    }

    pub async fn get_task_details_with_body(
        &self,
        task_id: &str,
    ) -> Result<(CodeTaskDetailsResponse, String, String)> {
        let url = match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/tasks/{}", self.base_url, task_id),
            PathStyle::ChatGptApi => format!("{}/wham/tasks/{}", self.base_url, task_id),
        };
        let req = self.http.get(&url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        let parsed: CodeTaskDetailsResponse = self.decode_json(&url, &ct, &body)?;
        Ok((parsed, body, ct))
    }

    pub async fn list_sibling_turns(
        &self,
        task_id: &str,
        turn_id: &str,
    ) -> Result<TurnAttemptsSiblingTurnsResponse> {
        let url = match self.path_style {
            PathStyle::CodexApi => format!(
                "{}/api/codex/tasks/{}/turns/{}/sibling_turns",
                self.base_url, task_id, turn_id
            ),
            PathStyle::ChatGptApi => format!(
                "{}/wham/tasks/{}/turns/{}/sibling_turns",
                self.base_url, task_id, turn_id
            ),
        };
        let req = self.http.get(&url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        self.decode_json::<TurnAttemptsSiblingTurnsResponse>(&url, &ct, &body)
    }

    /// Fetch the managed requirements file from codex-backend.
    ///
    /// `GET /api/codex/config/requirements` (Codex API style) or
    /// `GET /wham/config/requirements` (ChatGPT backend-api style).
    pub async fn get_config_requirements_file(&self) -> Result<ConfigFileResponse> {
        let url = match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/config/requirements", self.base_url),
            PathStyle::ChatGptApi => format!("{}/wham/config/requirements", self.base_url),
        };
        let req = self.http.get(&url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        self.decode_json::<ConfigFileResponse>(&url, &ct, &body)
    }

    /// Create a new task (user turn) by POSTing to the appropriate backend path
    /// based on `path_style`. Returns the created task id.
    pub async fn create_task(&self, request_body: serde_json::Value) -> Result<String> {
        let url = match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/tasks", self.base_url),
            PathStyle::ChatGptApi => format!("{}/wham/tasks", self.base_url),
        };
        let req = self
            .http
            .post(&url)
            .headers(self.headers())
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&request_body);
        let (body, ct) = self.exec_request(req, "POST", &url).await?;
        // Extract id from JSON: prefer `task.id`; fallback to top-level `id` when present.
        match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(v) => {
                if let Some(id) = v
                    .get("task")
                    .and_then(|t| t.get("id"))
                    .and_then(|s| s.as_str())
                {
                    Ok(id.to_string())
                } else if let Some(id) = v.get("id").and_then(|s| s.as_str()) {
                    Ok(id.to_string())
                } else {
                    anyhow::bail!(
                        "POST {url} succeeded but no task id found; content-type={ct}; body={body}"
                    );
                }
            }
            Err(e) => anyhow::bail!("Decode error for {url}: {e}; content-type={ct}; body={body}"),
        }
    }

    // rate limit helpers
    fn rate_limit_snapshots_from_payload(
        payload: RateLimitStatusPayload,
    ) -> Vec<RateLimitSnapshot> {
        let plan_type = Some(Self::map_plan_type(payload.plan_type));
        let mut snapshots = vec![Self::make_rate_limit_snapshot(
            Some("codex".to_string()),
            None,
            payload.rate_limit.flatten().map(|details| *details),
            payload.credits.flatten().map(|details| *details),
            plan_type,
        )];
        if let Some(additional) = payload.additional_rate_limits.flatten() {
            snapshots.extend(additional.into_iter().map(|details| {
                Self::make_rate_limit_snapshot(
                    Some(details.metered_feature),
                    Some(details.limit_name),
                    details.rate_limit.flatten().map(|rate_limit| *rate_limit),
                    None,
                    plan_type,
                )
            }));
        }
        snapshots
    }

    fn make_rate_limit_snapshot(
        limit_id: Option<String>,
        limit_name: Option<String>,
        rate_limit: Option<crate::types::RateLimitStatusDetails>,
        credits: Option<crate::types::CreditStatusDetails>,
        plan_type: Option<AccountPlanType>,
    ) -> RateLimitSnapshot {
        let (primary, secondary) = match rate_limit {
            Some(details) => (
                Self::map_rate_limit_window(details.primary_window),
                Self::map_rate_limit_window(details.secondary_window),
            ),
            None => (None, None),
        };
        RateLimitSnapshot {
            limit_id,
            limit_name,
            primary,
            secondary,
            credits: Self::map_credits(credits),
            plan_type,
        }
    }

    fn map_rate_limit_window(
        window: Option<Option<Box<crate::types::RateLimitWindowSnapshot>>>,
    ) -> Option<RateLimitWindow> {
        let snapshot = window.flatten().map(|details| *details)?;

        let used_percent = f64::from(snapshot.used_percent);
        let window_minutes = Self::window_minutes_from_seconds(snapshot.limit_window_seconds);
        let resets_at = Some(i64::from(snapshot.reset_at));
        Some(RateLimitWindow {
            used_percent,
            window_minutes,
            resets_at,
        })
    }

    fn map_credits(credits: Option<crate::types::CreditStatusDetails>) -> Option<CreditsSnapshot> {
        let details = credits?;

        Some(CreditsSnapshot {
            has_credits: details.has_credits,
            unlimited: details.unlimited,
            balance: details.balance.flatten(),
        })
    }

    fn map_plan_type(plan_type: crate::types::PlanType) -> AccountPlanType {
        match plan_type {
            crate::types::PlanType::Free => AccountPlanType::Free,
            crate::types::PlanType::Go => AccountPlanType::Go,
            crate::types::PlanType::Plus => AccountPlanType::Plus,
            crate::types::PlanType::Pro => AccountPlanType::Pro,
            crate::types::PlanType::Team => AccountPlanType::Team,
            crate::types::PlanType::Business => AccountPlanType::Business,
            crate::types::PlanType::Enterprise => AccountPlanType::Enterprise,
            crate::types::PlanType::Edu | crate::types::PlanType::Education => AccountPlanType::Edu,
            crate::types::PlanType::Guest
            | crate::types::PlanType::FreeWorkspace
            | crate::types::PlanType::Quorum
            | crate::types::PlanType::K12 => AccountPlanType::Unknown,
        }
    }

    fn window_minutes_from_seconds(seconds: i32) -> Option<i64> {
        if seconds <= 0 {
            return None;
        }

        let seconds_i64 = i64::from(seconds);
        Some((seconds_i64 + 59) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn usage_payload_maps_primary_and_additional_rate_limits() {
        let payload = RateLimitStatusPayload {
            plan_type: crate::types::PlanType::Pro,
            rate_limit: Some(Some(Box::new(crate::types::RateLimitStatusDetails {
                primary_window: Some(Some(Box::new(crate::types::RateLimitWindowSnapshot {
                    used_percent: 42,
                    limit_window_seconds: 300,
                    reset_after_seconds: 0,
                    reset_at: 123,
                }))),
                secondary_window: Some(Some(Box::new(crate::types::RateLimitWindowSnapshot {
                    used_percent: 84,
                    limit_window_seconds: 3600,
                    reset_after_seconds: 0,
                    reset_at: 456,
                }))),
                ..Default::default()
            }))),
            additional_rate_limits: Some(Some(vec![crate::types::AdditionalRateLimitDetails {
                limit_name: "codex_other".to_string(),
                metered_feature: "codex_other".to_string(),
                rate_limit: Some(Some(Box::new(crate::types::RateLimitStatusDetails {
                    primary_window: Some(Some(Box::new(crate::types::RateLimitWindowSnapshot {
                        used_percent: 70,
                        limit_window_seconds: 900,
                        reset_after_seconds: 0,
                        reset_at: 789,
                    }))),
                    secondary_window: None,
                    ..Default::default()
                }))),
            }])),
            credits: Some(Some(Box::new(crate::types::CreditStatusDetails {
                has_credits: true,
                unlimited: false,
                balance: Some(Some("9.99".to_string())),
                ..Default::default()
            }))),
        };

        let snapshots = Client::rate_limit_snapshots_from_payload(payload);
        assert_eq!(snapshots.len(), 2);

        assert_eq!(snapshots[0].limit_id.as_deref(), Some("codex"));
        assert_eq!(snapshots[0].limit_name, None);
        assert_eq!(
            snapshots[0].primary.as_ref().map(|w| w.used_percent),
            Some(42.0)
        );
        assert_eq!(
            snapshots[0].secondary.as_ref().map(|w| w.used_percent),
            Some(84.0)
        );
        assert_eq!(
            snapshots[0].credits,
            Some(CreditsSnapshot {
                has_credits: true,
                unlimited: false,
                balance: Some("9.99".to_string()),
            })
        );
        assert_eq!(snapshots[0].plan_type, Some(AccountPlanType::Pro));

        assert_eq!(snapshots[1].limit_id.as_deref(), Some("codex_other"));
        assert_eq!(snapshots[1].limit_name.as_deref(), Some("codex_other"));
        assert_eq!(
            snapshots[1].primary.as_ref().map(|w| w.used_percent),
            Some(70.0)
        );
        assert_eq!(snapshots[1].credits, None);
        assert_eq!(snapshots[1].plan_type, Some(AccountPlanType::Pro));
    }

    #[test]
    fn usage_payload_maps_zero_rate_limit_when_primary_absent() {
        let payload = RateLimitStatusPayload {
            plan_type: crate::types::PlanType::Plus,
            rate_limit: None,
            additional_rate_limits: Some(Some(vec![crate::types::AdditionalRateLimitDetails {
                limit_name: "codex_other".to_string(),
                metered_feature: "codex_other".to_string(),
                rate_limit: None,
            }])),
            credits: None,
        };

        let snapshots = Client::rate_limit_snapshots_from_payload(payload);
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].limit_id.as_deref(), Some("codex"));
        assert_eq!(snapshots[0].limit_name, None);
        assert_eq!(snapshots[0].primary, None);
        assert_eq!(snapshots[1].limit_id.as_deref(), Some("codex_other"));
        assert_eq!(snapshots[1].limit_name.as_deref(), Some("codex_other"));
    }

    #[test]
    fn preferred_snapshot_selection_matches_get_rate_limits_behavior() {
        let snapshots = [
            RateLimitSnapshot {
                limit_id: Some("codex_other".to_string()),
                limit_name: Some("codex_other".to_string()),
                primary: Some(RateLimitWindow {
                    used_percent: 90.0,
                    window_minutes: Some(60),
                    resets_at: Some(1),
                }),
                secondary: None,
                credits: None,
                plan_type: Some(AccountPlanType::Pro),
            },
            RateLimitSnapshot {
                limit_id: Some("codex".to_string()),
                limit_name: Some("codex".to_string()),
                primary: Some(RateLimitWindow {
                    used_percent: 10.0,
                    window_minutes: Some(60),
                    resets_at: Some(2),
                }),
                secondary: None,
                credits: None,
                plan_type: Some(AccountPlanType::Pro),
            },
        ];

        let preferred = snapshots
            .iter()
            .find(|snapshot| snapshot.limit_id.as_deref() == Some("codex"))
            .cloned()
            .unwrap_or_else(|| snapshots[0].clone());
        assert_eq!(preferred.limit_id.as_deref(), Some("codex"));
    }
}
