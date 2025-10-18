use crate::types::CodeTaskDetailsResponse;
use crate::types::PaginatedListTaskListItem;
use crate::types::TurnAttemptsSiblingTurnsResponse;
use anyhow::Result;
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

    pub async fn list_tasks(
        &self,
        limit: Option<i32>,
        task_filter: Option<&str>,
        environment_id: Option<&str>,
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
}
