//! Loads evaluated values into the SDK and keeps reads scoped to the initial auth owner.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_backend_client::Client;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use codex_login::AuthOwner;
use codex_login::CodexAuth;
use codex_model_provider::ProviderAuthContext;
use serde::Deserialize;
use serde_json::Map;
use serde_json::Value;
use statsig_client::FreshValuePolicy;
use statsig_client::InitializeOpts;
use statsig_client::ShutdownOpts;
use statsig_client::StatsigClient;
use statsig_client::StatsigOptions;
use statsig_client::StatsigUser;
use statsig_client::UpdateStrategy;
use statsig_client::UpdateUserOpts;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::StatsigBootstrapContext;
use crate::StatsigHttpClient;

const EVALUATION_TIMEOUT: Duration = Duration::from_secs(10);
const INITIALIZE_RETRY_INTERVAL: Duration = Duration::from_secs(60);
const REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);

static PROCESS_RUNTIME: StdMutex<Weak<StatsigRuntime>> = StdMutex::new(Weak::new());

/// A named Statsig gate that can be declared alongside its consumers.
#[derive(Clone, Copy)]
pub struct Gate(&'static str);

impl Gate {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Reads the latest cached decision, defaulting to false until evaluations are available.
    pub fn is_enabled(self) -> bool {
        current_evaluations().is_some_and(|values| values.sdk.check_gate(self.0))
    }
}

/// A named Statsig dynamic config that can be declared alongside its consumers.
#[derive(Clone, Copy)]
pub struct DynamicConfig(&'static str);

impl DynamicConfig {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Reads the latest cached JSON object, defaulting to an empty object until available.
    pub fn value(self) -> Map<String, Value> {
        let Some(values) = current_evaluations() else {
            return Map::new();
        };
        values.sdk.get_dynamic_config(self.0).value().clone()
    }
}

fn current_evaluations() -> Option<Arc<StatsigEvaluations>> {
    let runtime = StatsigRuntime::current()?;
    let values = runtime.evaluations.borrow().clone()?;
    values.is_current().then_some(values)
}

/// The identity this process is authorized to use for Statsig.
pub enum StatsigIdentity {
    Account(ProviderAuthContext),
    Installation(HttpClientFactory),
}

/// Owns one background refresh worker for a harness and exposes its local decisions.
pub struct StatsigRuntime {
    evaluations: watch::Receiver<Option<Arc<StatsigEvaluations>>>,
    worker: JoinHandle<()>,
}

impl StatsigRuntime {
    /// Retains the active process worker while the caller owns this handle.
    pub fn current() -> Option<Arc<Self>> {
        PROCESS_RUNTIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade()
    }

    /// Installs one worker for the process, shared by embedded callers.
    pub fn start_process(
        identity: StatsigIdentity,
        chatgpt_base_url: String,
        sdk_key: String,
        context: StatsigBootstrapContext,
        exposure_logging_enabled: bool,
    ) -> Arc<Self> {
        let mut active = PROCESS_RUNTIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(runtime) = active.upgrade() {
            return runtime;
        }
        let (publish, evaluations) = watch::channel::<Option<Arc<StatsigEvaluations>>>(None);
        let mut auth_changes = match &identity {
            StatsigIdentity::Account(auth) => Some(auth.credentials.auth_change_state_receiver()),
            StatsigIdentity::Installation(_) => None,
        };
        let worker = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = publish.closed() => break,
                    () = async {
                        let generation = auth_changes.as_mut().map(|changes| *changes.borrow_and_update());
                        let current = publish.borrow().clone();
                        let update = if let Some(current) = current.filter(|values| values.is_current()) {
                            current.refresh().await
                        } else {
                            if let Some(previous) = publish.send_replace(None) {
                                previous.shutdown().await;
                            }
                            StatsigEvaluations::start(
                                &identity,
                                &chatgpt_base_url,
                                &sdk_key,
                                context.clone(),
                                StatsigOptions::new()
                                    .with_initialize_url("https://ab.chatgpt.com/v1/initialize")
                                    .with_log_event_url("https://ab.chatgpt.com/v1/rgstr")
                                    .with_sdk_exception_url("https://ab.chatgpt.com/v1/sdk_exception")
                                    .with_disable_all_logging(!exposure_logging_enabled),
                            ).await.map(|values| {
                                publish.send_replace(Some(Arc::new(values)));
                            })
                        };
                        if update.is_err() {
                            tracing::debug!("Statsig evaluations update failed");
                        }
                        let interval = if publish.borrow().is_some() {
                            REFRESH_INTERVAL
                        } else {
                            INITIALIZE_RETRY_INTERVAL
                        };
                        if let (Some(changes), Some(generation)) = (&mut auth_changes, generation) {
                            let _ = tokio::time::timeout(
                                interval,
                                changes.wait_for(|state| {
                                    state.owner_generation != generation.owner_generation
                                        || (interval == INITIALIZE_RETRY_INTERVAL && state.generation != generation.generation)
                                }),
                            ).await;
                        } else {
                            tokio::time::sleep(interval).await;
                        }
                    } => {}
                }
            }
            if let Some(current) = publish.send_replace(None) {
                current.shutdown().await;
            }
        });
        let runtime = Arc::new(Self {
            evaluations,
            worker,
        });
        *active = Arc::downgrade(&runtime);
        runtime
    }

    /// Flushes on normal exit after the last process host releases its worker.
    pub async fn finish(runtime: Option<Arc<Self>>) {
        if let Some(runtime) = runtime.and_then(Arc::into_inner) {
            // Closing the watch channel stops the worker and triggers the SDK flush.
            drop(runtime.evaluations);
            let _ = runtime.worker.await;
        }
    }
}

#[derive(Deserialize)]
struct BootstrapUser {
    user: StatsigUser,
}

/// Server decisions for a single Codex identity. Gate and config reads are local.
struct StatsigEvaluations {
    sdk: StatsigClient,
    owner: Option<AuthOwner>,
}

impl StatsigEvaluations {
    /// Uses Codex backend bootstrap for ChatGPT sign-in and SDK initialize for API-key sign-in.
    async fn start(
        identity: &StatsigIdentity,
        chatgpt_base_url: &str,
        sdk_key: &str,
        context: StatsigBootstrapContext,
        options: StatsigOptions,
    ) -> Result<Self> {
        anyhow::ensure!(
            sdk_key.starts_with("client-") && sdk_key.len() > "client-".len(),
            "Statsig requires a client SDK key"
        );
        let (owner, factory, bootstrap) = match identity {
            StatsigIdentity::Account(context_auth) => {
                let auth_manager = &context_auth.credentials;
                let (auth, factory) = auth_manager
                    .auth_with_http_client_factory()
                    .await
                    .ok_or_else(|| anyhow!("Sign in to Codex before starting Statsig"))?;
                let owner = auth_manager
                    .auth_owner_for(&auth)
                    .ok_or_else(|| anyhow!("Codex authentication changed during Statsig setup"))?;
                let bootstrap = match &auth {
                    CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
                        let backend = Client::from_auth_with_runtime_without_redirects(
                            chatgpt_base_url,
                            &auth,
                            factory.clone(),
                            context_auth.runtime_auth.as_ref(),
                            &Default::default(),
                        )
                        .await?;
                        Some(backend.statsig_bootstrap(&context).await?)
                    }
                    CodexAuth::ApiKey(_) => None,
                    _ => anyhow::bail!("This Codex sign-in method is unsupported by Statsig"),
                };
                (Some(owner), factory, bootstrap)
            }
            StatsigIdentity::Installation(factory) => (None, factory.clone(), None),
        };
        let user = match &bootstrap {
            Some(values) => serde_json::from_str::<BootstrapUser>(values)?.user,
            None if owner.is_some() => anonymous_user(context, "apikey"),
            None => anonymous_user(context, "anonymous"),
        };
        let pool = RouteAwareClientPool::new_without_redirects_or_request_logging(
            factory,
            ClientRouteClass::Api,
        );
        let options = options
            .with_runtime_handle(Handle::current())
            .with_http_client(StatsigHttpClient::new(pool))
            .with_disable_all_caching(/* disabled */ true);
        let sdk = StatsigClient::with_options(sdk_key, user, options)?;
        if let Some(values) = bootstrap {
            sdk.initialize_local(UpdateStrategy::Bootstrap(values.as_bytes()))?;
        } else {
            sdk.initialize_with_opts(InitializeOpts {
                timeout: Some(EVALUATION_TIMEOUT),
                fresh_value_policy: FreshValuePolicy::RequireFresh,
            })
            .await?;
        }
        let values = Self { sdk, owner };
        anyhow::ensure!(
            values.is_current(),
            "Codex authentication changed during Statsig setup"
        );
        Ok(values)
    }

    /// Refreshes the SDK's full set of evaluated values for the current user.
    async fn refresh(&self) -> Result<()> {
        anyhow::ensure!(self.is_current(), "Codex authentication changed");
        let user = self.sdk.current_state().user().clone();
        let opts = UpdateUserOpts {
            timeout: Some(EVALUATION_TIMEOUT),
            fresh_value_policy: FreshValuePolicy::RequireFresh,
        };
        self.sdk.update_user_with_opts(user, opts).await?;
        Ok(())
    }

    fn is_current(&self) -> bool {
        self.owner.as_ref().is_none_or(AuthOwner::is_current)
    }

    async fn shutdown(&self) {
        let opts = ShutdownOpts {
            timeout: Some(Duration::from_secs(2)),
        };
        let _ = self.sdk.shutdown_with_opts(opts).await;
    }
}

fn anonymous_user(context: StatsigBootstrapContext, auth_method: &str) -> StatsigUser {
    let mut user = StatsigUser::new(format!("ua-{}", context.stable_id))
        .with_custom_id("stableID", &context.stable_id)
        .with_custom_id("source_surface_stable_id", context.stable_id)
        .with_app_version(context.app_version)
        .with_custom("auth_method", auth_method)
        .with_custom("auth_status", "logged_out")
        .with_custom("brand_name", context.brand_name)
        .with_custom("codex_window_type", context.window_type)
        .with_custom("systemName", context.system_name)
        .with_custom("systemVersion", context.system_version);
    user.locale = context.locale;
    user
}

#[cfg(test)]
#[path = "evaluations_tests.rs"]
mod tests;
