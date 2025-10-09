use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_backend_client::EnvironmentSummary as BackendEnvironmentSummary;
use codex_cloud_tasks::util::extract_chatgpt_account_id;
use codex_cloud_tasks::util::normalize_base_url;
use codex_cloud_tasks::util::set_user_agent_suffix;
use codex_cloud_tasks_client::CloudBackend;
use codex_cloud_tasks_client::HttpClient;
use codex_cloud_tasks_client::MockClient;
use codex_common::CliConfigOverrides;

/// Fresh backend handles for each headless invocation.
#[allow(dead_code)]
pub struct CloudContext {
    backend: Arc<dyn CloudBackend>,
    http_extras: Option<HttpExtras>,
    overrides: CliConfigOverrides,
}

#[allow(dead_code)]
struct HttpExtras {
    backend_client: codex_backend_client::Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentSummary {
    pub id: String,
    pub label: Option<String>,
}

#[allow(dead_code)]
impl CloudContext {
    pub async fn new(overrides: CliConfigOverrides) -> Result<Self> {
        set_user_agent_suffix("codex_cloud_headless");
        let use_mock = matches!(
            std::env::var("CODEX_CLOUD_TASKS_MODE").ok().as_deref(),
            Some("mock") | Some("MOCK")
        );

        if use_mock {
            let backend: Arc<dyn CloudBackend> = Arc::new(MockClient);
            return Ok(Self {
                backend,
                http_extras: None,
                overrides,
            });
        }

        let base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
            .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
        let base_url = normalize_base_url(&base_url);
        let ua = codex_core::default_client::get_codex_user_agent();

        let mut http_client = HttpClient::new(base_url.clone())?.with_user_agent(ua.clone());
        let mut backend_client = codex_backend_client::Client::new(base_url)?.with_user_agent(ua);

        let codex_home = codex_core::config::find_codex_home()
            .context("Not signed in. Run 'codex login' to sign in with ChatGPT.")?;
        let auth_manager = codex_login::AuthManager::new(codex_home, false);
        let auth = auth_manager
            .auth()
            .ok_or_else(|| anyhow!("Not signed in. Run 'codex login' to sign in with ChatGPT."))?;
        let token = auth
            .get_token()
            .await
            .context("Failed to load ChatGPT session token")?;
        if token.is_empty() {
            bail!("Not signed in. Run 'codex login' to sign in with ChatGPT.");
        }

        http_client = http_client.with_bearer_token(token.clone());
        backend_client = backend_client.with_bearer_token(token.clone());

        if let Some(account_id) = auth
            .get_account_id()
            .or_else(|| extract_chatgpt_account_id(&token))
        {
            http_client = http_client.with_chatgpt_account_id(account_id.clone());
            backend_client = backend_client.with_chatgpt_account_id(account_id);
        }

        let backend: Arc<dyn CloudBackend> = Arc::new(http_client);
        let http_extras = Some(HttpExtras { backend_client });

        Ok(Self {
            backend,
            http_extras,
            overrides,
        })
    }

    pub fn backend(&self) -> Arc<dyn CloudBackend> {
        Arc::clone(&self.backend)
    }

    pub fn backend_client(&self) -> Option<&codex_backend_client::Client> {
        self.http_extras
            .as_ref()
            .map(|extras| &extras.backend_client)
    }

    pub async fn list_environments(&self) -> Result<Vec<EnvironmentSummary>> {
        if let Some(extras) = &self.http_extras {
            let envs = extras
                .backend_client
                .list_environments()
                .await
                .context("Failed to list environments from Cloud")?;
            return Ok(envs
                .into_iter()
                .map(|env: BackendEnvironmentSummary| EnvironmentSummary {
                    id: env.id,
                    label: env.label.filter(|label| !label.is_empty()),
                })
                .collect());
        }

        Ok(vec![
            EnvironmentSummary {
                id: "env_abc123".to_string(),
                label: Some("OrgA/prod".to_string()),
            },
            EnvironmentSummary {
                id: "env_def456".to_string(),
                label: Some("OrgA/qa".to_string()),
            },
            EnvironmentSummary {
                id: "env_xyz789".to_string(),
                label: Some("L1nuxOne/ade".to_string()),
            },
            EnvironmentSummary {
                id: "env_prod999".to_string(),
                label: Some("OrgB/prod".to_string()),
            },
        ])
    }

    pub fn overrides(&self) -> &CliConfigOverrides {
        &self.overrides
    }
}
