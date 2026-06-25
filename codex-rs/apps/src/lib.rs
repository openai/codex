//! Connector-scoped loopback HTTP MCP servers backed by one Codex Apps connection.
//!
//! [`CodexApps::connect_with_environment`] restores an identity-, upstream-, and SKU-scoped tool
//! snapshot when available, then refreshes it from a shared inventory connection.
//! Each generation serves every known connector as a distinct MCP endpoint on one authenticated
//! loopback HTTP listener. Each downstream MCP session lazily opens its own upstream connection so
//! an elicitation in one session cannot block unrelated connectors. [`CodexApps::refresh`]
//! publishes a complete replacement generation atomically;
//! existing [`CodexAppsSnapshot`] handles remain pinned to their internally consistent generation.
//!
//! Tools without complete connector identity are omitted.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use arc_swap::ArcSwap;
use codex_api::SharedAuthProvider;
use codex_exec_server::EnvironmentManager;
use codex_rmcp_client::RmcpClient;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::Tool;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use self::cache::ScopedCodexAppsCacheContext;
use self::elicitation_bridge::AppsElicitationBridge;
use self::file_upload::AppsFileSupport;
use self::generation::CodexAppsGeneration;
use self::generation::CodexAppsGenerationInput;
use self::generation::CodexAppsSnapshotOwner;
use self::generation::InventoryProvenance;

const CODEX_APPS_LOAD_TIMEOUT: Duration = Duration::from_secs(30);
// Hosted Apps currently returns a small inventory. These limits leave room for thousands of tools
// and sparse pagination while bounding memory and requests if an upstream cursor never terminates.
const MAX_CODEX_APPS_TOOLS: usize = 4_096;
const MAX_CODEX_APPS_TOOL_PAGES: usize = 128;
const MAX_CODEX_APPS_TOOL_INVENTORY_BYTES: usize = 8 * 1024 * 1024;
const MAX_CODEX_APPS_UPSTREAM_POST_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MCP_TOOLS_LIST_DURATION_METRIC: &str = "codex.mcp.tools.list.duration_ms";
const MCP_TOOLS_CACHE_WRITE_DURATION_METRIC: &str = "codex.mcp.tools.cache_write.duration_ms";
const MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC: &str = "codex.mcp.tools.fetch_uncached.duration_ms";

mod approval_presentation;
mod auth_elicitation;
mod cache;
mod connector_server;
mod elicitation_bridge;
mod file_upload;
mod generation;
mod http;
mod names;
mod resource_server;
mod upstream;

pub use cache::CodexAppsCacheContext;
pub use cache::CodexAppsCacheIdentity;
pub use generation::CodexApp;
pub use generation::CodexAppToolMetadata;
pub use generation::CodexAppsSnapshot;
pub use upstream::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;
pub use upstream::CodexAppsConnectConfig;

/// Process-local validity check for the credentials backing an Apps connection.
///
/// The check runs at the loopback HTTP boundary and again before proxying a request upstream. This
/// lets a host revoke already-published MCP registrations synchronously when its auth generation
/// changes, without exposing auth concepts to the generic MCP manager. Authorization is linearized
/// when a request is forwarded upstream: an already-forwarded request may finish, while every later
/// request is rejected at the loopback boundary.
#[derive(Clone)]
pub struct CodexAppsAccessGuard {
    is_current: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl CodexAppsAccessGuard {
    pub fn new(is_current: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            is_current: Arc::new(is_current),
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        (self.is_current)()
    }
}

impl Default for CodexAppsAccessGuard {
    fn default() -> Self {
        Self::new(|| true)
    }
}

/// Owns the current Apps inventory and its connector-scoped HTTP MCP servers.
pub struct CodexApps {
    upstream: Arc<AppsUpstream>,
    generation: Arc<ArcSwap<CodexAppsGeneration>>,
    generation_registry: Arc<AppsGenerationRegistry>,
    refresh_coordinator: Arc<AppsRefreshCoordinator>,
    background_refresh: tokio::sync::Mutex<Option<AbortOnDropHandle<()>>>,
    shutdown: CancellationToken,
}

struct AppsUpstream {
    client: tokio::sync::OnceCell<Arc<RmcpClient>>,
    connection_factory: AppsUpstreamConnectionFactory,
    elicitation_bridge: Arc<AppsElicitationBridge>,
    telemetry_url: String,
}

#[derive(Clone)]
struct AppsUpstreamConnectionFactory {
    config: CodexAppsConnectConfig,
    bearer_token: Option<String>,
    auth_provider: SharedAuthProvider,
}

#[derive(Default)]
struct AppsRefreshCoordinator {
    context: std::sync::OnceLock<AppsRefreshContext>,
}

struct AppsRefreshContext {
    upstream: Arc<AppsUpstream>,
    generations: Arc<ArcSwap<CodexAppsGeneration>>,
    generation_registry: Arc<AppsGenerationRegistry>,
    file_support: Option<Arc<AppsFileSupport>>,
    cache_context: Option<ScopedCodexAppsCacheContext>,
    refresh_permit: tokio::sync::Semaphore,
    inventory_changed: Arc<dyn Fn() + Send + Sync>,
    access_guard: CodexAppsAccessGuard,
    shutdown: CancellationToken,
}

#[derive(Default)]
struct AppsGenerationRegistry {
    generations: Mutex<Vec<Weak<CodexAppsGeneration>>>,
}

impl AppsGenerationRegistry {
    fn with_initial(generation: &Arc<CodexAppsGeneration>) -> Self {
        Self {
            generations: Mutex::new(vec![Arc::downgrade(generation)]),
        }
    }

    fn publish(
        &self,
        published: &ArcSwap<CodexAppsGeneration>,
        generation: Arc<CodexAppsGeneration>,
        shutdown: &CancellationToken,
    ) -> Result<()> {
        let mut generations = self
            .generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if shutdown.is_cancelled() {
            bail!("Codex Apps is shutting down");
        }
        generations.retain(|generation| generation.strong_count() > 0);
        generations.push(Arc::downgrade(&generation));
        published.store(generation);
        Ok(())
    }

    fn drain_live(&self) -> Vec<Arc<CodexAppsGeneration>> {
        let mut generations = self
            .generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *generations)
            .into_iter()
            .filter_map(|generation| generation.upgrade())
            .collect()
    }
}

struct AppsInitialization {
    upstream: Arc<AppsUpstream>,
    generation: CodexAppsGeneration,
    file_support: Option<Arc<AppsFileSupport>>,
    refresh_coordinator: Arc<AppsRefreshCoordinator>,
    cache_context: Option<ScopedCodexAppsCacheContext>,
    inventory_changed: Arc<dyn Fn() + Send + Sync>,
    access_guard: CodexAppsAccessGuard,
    shutdown: CancellationToken,
}

impl AppsRefreshCoordinator {
    fn initialize(&self, context: AppsRefreshContext) {
        let initialized = self.context.set(context).is_ok();
        debug_assert!(
            initialized,
            "Codex Apps refresh coordinator initialized twice"
        );
    }

    async fn refresh(self: &Arc<Self>) -> Result<Arc<CodexAppsGeneration>> {
        let context = self
            .context
            .get()
            .context("Codex Apps refresh coordinator is not initialized")?;
        let _refresh_permit = context
            .refresh_permit
            .acquire()
            .await
            .context("Codex Apps refresh coordinator is closed")?;
        self.refresh_after_acquiring_permit(context).await
    }

    async fn refresh_if_current(
        self: &Arc<Self>,
        observed: Arc<CodexAppsGeneration>,
    ) -> Result<Arc<CodexAppsGeneration>> {
        let context = self
            .context
            .get()
            .context("Codex Apps refresh coordinator is not initialized")?;
        let _refresh_permit = context
            .refresh_permit
            .acquire()
            .await
            .context("Codex Apps refresh coordinator is closed")?;
        let current = context.generations.load_full();
        if !Arc::ptr_eq(&current, &observed) {
            return Ok(current);
        }
        self.refresh_after_acquiring_permit(context).await
    }

    async fn refresh_after_acquiring_permit(
        self: &Arc<Self>,
        context: &AppsRefreshContext,
    ) -> Result<Arc<CodexAppsGeneration>> {
        if context.shutdown.is_cancelled() {
            bail!("Codex Apps is shutting down");
        }
        if !context.access_guard.is_current() {
            bail!("Codex Apps credentials are no longer current");
        }
        let upstream = tokio::select! {
            result = context.upstream.client() => result?,
            _ = context.shutdown.cancelled() => bail!("Codex Apps is shutting down"),
        };
        let list_start = Instant::now();
        let raw_tools = tokio::select! {
            result = list_all_upstream_tools(&upstream) => result?,
            _ = context.shutdown.cancelled() => bail!("Codex Apps is shutting down"),
        };
        let generation = Arc::new(
            CodexAppsGeneration::from_tools(CodexAppsGenerationInput {
                upstream: Arc::clone(&context.upstream),
                raw_tools: raw_tools.clone(),
                inventory_provenance: InventoryProvenance::Live,
                file_support: context.file_support.clone(),
                refresh_coordinator: Arc::downgrade(self),
                access_guard: context.access_guard.clone(),
                shutdown: context.shutdown.child_token(),
            })
            .await?,
        );
        if let Some(cache_context) = context.cache_context.clone()
            && let Err(error) = write_cached_tools(cache_context, raw_tools).await
        {
            tracing::warn!(%error, "failed to persist refreshed Codex Apps tool cache");
        }
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
        context.generation_registry.publish(
            &context.generations,
            Arc::clone(&generation),
            &context.shutdown,
        )?;
        (context.inventory_changed)();
        Ok(generation)
    }
}

impl AppsUpstream {
    fn connecting(
        config: CodexAppsConnectConfig,
        bearer_token: Option<String>,
        auth_provider: SharedAuthProvider,
        elicitation_bridge: Arc<AppsElicitationBridge>,
    ) -> Arc<Self> {
        let telemetry_url = config.upstream_url();
        Arc::new(Self {
            client: tokio::sync::OnceCell::new(),
            connection_factory: AppsUpstreamConnectionFactory {
                config,
                bearer_token,
                auth_provider,
            },
            elicitation_bridge,
            telemetry_url,
        })
    }

    /// Returns an upstream connection scoped to one downstream HTTP MCP session.
    fn fork(self: &Arc<Self>) -> Arc<Self> {
        Self::connecting(
            self.connection_factory.config.clone(),
            self.connection_factory.bearer_token.clone(),
            Arc::clone(&self.connection_factory.auth_provider),
            AppsElicitationBridge::new(),
        )
    }

    fn telemetry_url(&self) -> &str {
        &self.telemetry_url
    }

    async fn client(&self) -> Result<Arc<RmcpClient>> {
        if let Some(client) = self.client.get() {
            return Ok(Arc::clone(client));
        }
        let client = self
            .client
            .get_or_try_init(|| async {
                upstream::connect_upstream(
                    &self.connection_factory.config,
                    self.connection_factory.bearer_token.clone(),
                    Arc::clone(&self.connection_factory.auth_provider),
                    Arc::clone(&self.elicitation_bridge),
                )
                .await
            })
            .await?;
        Ok(Arc::clone(client))
    }

    async fn shutdown(&self) {
        if let Some(client) = self.client.get() {
            client.shutdown().await;
        }
    }
}

impl CodexApps {
    #[cfg(test)]
    async fn connect(
        config: &CodexAppsConnectConfig,
        auth_provider: SharedAuthProvider,
    ) -> Result<Self> {
        Self::connect_inner(
            config,
            auth_provider,
            /*file_support*/ None,
            Arc::new(|| {}),
            CodexAppsAccessGuard::default(),
        )
        .await
    }

    /// Connects with host environment access and synchronously reports published inventory
    /// changes. The notifier is installed before a cached inventory can begin refreshing.
    pub async fn connect_with_environment(
        config: &CodexAppsConnectConfig,
        auth_provider: SharedAuthProvider,
        environment_manager: Arc<EnvironmentManager>,
        inventory_changed: Arc<dyn Fn() + Send + Sync>,
        access_guard: CodexAppsAccessGuard,
    ) -> Result<Self> {
        let file_support = Arc::new(AppsFileSupport {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            auth_provider: Arc::clone(&auth_provider),
            environment_manager,
        });
        Self::connect_inner(
            config,
            auth_provider,
            Some(file_support),
            inventory_changed,
            access_guard,
        )
        .await
    }

    async fn connect_inner(
        config: &CodexAppsConnectConfig,
        auth_provider: SharedAuthProvider,
        file_support: Option<Arc<AppsFileSupport>>,
        inventory_changed: Arc<dyn Fn() + Send + Sync>,
        access_guard: CodexAppsAccessGuard,
    ) -> Result<Self> {
        let bearer_token = upstream::connectors_bearer_token()?;
        Self::connect_inner_with_bearer_token(
            config,
            bearer_token,
            auth_provider,
            file_support,
            inventory_changed,
            access_guard,
        )
        .await
    }

    async fn connect_inner_with_bearer_token(
        config: &CodexAppsConnectConfig,
        bearer_token: Option<String>,
        auth_provider: SharedAuthProvider,
        file_support: Option<Arc<AppsFileSupport>>,
        inventory_changed: Arc<dyn Fn() + Send + Sync>,
        access_guard: CodexAppsAccessGuard,
    ) -> Result<Self> {
        let elicitation_bridge = AppsElicitationBridge::new();
        let refresh_coordinator = Arc::new(AppsRefreshCoordinator::default());
        let cache_context = if bearer_token.is_some() {
            None
        } else {
            config.scoped_cache_context()
        };
        let cached_tools = match cache_context.clone() {
            Some(cache_context) => match load_cached_tools(cache_context).await {
                Ok(tools) => tools,
                Err(error) => {
                    tracing::warn!(%error, "ignoring unusable Codex Apps tool cache");
                    None
                }
            },
            None => None,
        };

        if let Some(cached_tools) = cached_tools {
            let upstream = AppsUpstream::connecting(
                config.clone(),
                bearer_token.clone(),
                Arc::clone(&auth_provider),
                Arc::clone(&elicitation_bridge),
            );
            let shutdown = CancellationToken::new();
            let generation = CodexAppsGeneration::from_tools(CodexAppsGenerationInput {
                upstream: Arc::clone(&upstream),
                raw_tools: cached_tools,
                inventory_provenance: InventoryProvenance::Cached,
                file_support: file_support.clone(),
                refresh_coordinator: Arc::downgrade(&refresh_coordinator),
                access_guard: access_guard.clone(),
                shutdown: shutdown.child_token(),
            })
            .await;
            match generation {
                Ok(generation) => {
                    let apps = Self::new(AppsInitialization {
                        upstream,
                        generation,
                        file_support,
                        refresh_coordinator,
                        cache_context,
                        inventory_changed,
                        access_guard,
                        shutdown,
                    });
                    apps.start_background_refresh().await;
                    return Ok(apps);
                }
                Err(error) => {
                    shutdown.cancel();
                    tracing::warn!(
                        %error,
                        "ignoring Codex Apps tool cache that cannot form a valid generation"
                    );
                }
            }
        }

        let upstream = AppsUpstream::connecting(
            config.clone(),
            bearer_token,
            auth_provider,
            Arc::clone(&elicitation_bridge),
        );
        let client = upstream.client().await?;
        let shutdown = CancellationToken::new();
        let list_start = Instant::now();
        let raw_tools = match list_all_upstream_tools(&client).await {
            Ok(tools) => tools,
            Err(error) => {
                client.shutdown().await;
                return Err(error);
            }
        };
        let generation = match CodexAppsGeneration::from_tools(CodexAppsGenerationInput {
            upstream: Arc::clone(&upstream),
            raw_tools: raw_tools.clone(),
            inventory_provenance: InventoryProvenance::Live,
            file_support: file_support.clone(),
            refresh_coordinator: Arc::downgrade(&refresh_coordinator),
            access_guard: access_guard.clone(),
            shutdown: shutdown.child_token(),
        })
        .await
        {
            Ok(generation) => generation,
            Err(error) => {
                client.shutdown().await;
                return Err(error);
            }
        };
        if let Some(cache_context) = cache_context.clone()
            && let Err(error) = write_cached_tools(cache_context, raw_tools).await
        {
            tracing::warn!(%error, "failed to persist Codex Apps tool cache");
        }
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
        Ok(Self::new(AppsInitialization {
            upstream,
            generation,
            file_support,
            refresh_coordinator,
            cache_context,
            inventory_changed,
            access_guard,
            shutdown,
        }))
    }

    fn new(initialization: AppsInitialization) -> Self {
        let AppsInitialization {
            upstream,
            generation,
            file_support,
            refresh_coordinator,
            cache_context,
            inventory_changed,
            access_guard,
            shutdown,
        } = initialization;
        let generation = Arc::new(generation);
        let generations = Arc::new(ArcSwap::from(Arc::clone(&generation)));
        let generation_registry = Arc::new(AppsGenerationRegistry::with_initial(&generation));
        refresh_coordinator.initialize(AppsRefreshContext {
            upstream: Arc::clone(&upstream),
            generations: Arc::clone(&generations),
            generation_registry: Arc::clone(&generation_registry),
            file_support,
            cache_context,
            refresh_permit: tokio::sync::Semaphore::new(1),
            inventory_changed,
            access_guard,
            shutdown: shutdown.clone(),
        });
        Self {
            upstream,
            generation: generations,
            generation_registry,
            refresh_coordinator,
            background_refresh: tokio::sync::Mutex::new(None),
            shutdown,
        }
    }

    async fn start_background_refresh(&self) {
        let refresh_coordinator = Arc::clone(&self.refresh_coordinator);
        let observed = self.generation.load_full();
        let task = tokio::spawn(async move {
            if let Err(error) = refresh_coordinator.refresh_if_current(observed).await {
                tracing::warn!(%error, "failed to refresh cached Codex Apps tools");
            }
        });
        *self.background_refresh.lock().await = Some(AbortOnDropHandle::new(task));
    }

    /// Pins and returns the currently published Apps generation.
    pub fn snapshot(&self) -> CodexAppsSnapshot {
        CodexAppsSnapshot {
            owner: Arc::new(CodexAppsSnapshotOwner {
                generation: self.generation.load_full(),
                _refresh_coordinator: Arc::clone(&self.refresh_coordinator),
            }),
        }
    }

    /// Builds and atomically publishes a fresh Apps generation.
    ///
    /// A failed refresh leaves the previously published generation untouched.
    pub async fn refresh(&self) -> Result<CodexAppsSnapshot> {
        let generation = self.refresh_coordinator.refresh().await?;
        Ok(CodexAppsSnapshot {
            owner: Arc::new(CodexAppsSnapshotOwner {
                generation,
                _refresh_coordinator: Arc::clone(&self.refresh_coordinator),
            }),
        })
    }

    /// Returns a live inventory, joining an already-running cached startup refresh when needed.
    pub async fn ensure_live(&self) -> Result<CodexAppsSnapshot> {
        let observed = self.generation.load_full();
        let generation = if observed.inventory_provenance == InventoryProvenance::Live {
            observed
        } else {
            self.refresh_coordinator
                .refresh_if_current(observed)
                .await?
        };
        Ok(CodexAppsSnapshot {
            owner: Arc::new(CodexAppsSnapshotOwner {
                generation,
                _refresh_coordinator: Arc::clone(&self.refresh_coordinator),
            }),
        })
    }

    /// Stops every connector endpoint created by this owner, including pinned older generations.
    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        let background_refresh = self.background_refresh.lock().await.take();
        if let Some(task) = background_refresh {
            task.abort();
            if let Err(error) = task.await
                && !error.is_cancelled()
            {
                tracing::warn!(%error, "failed to join Codex Apps background refresh");
            }
        }
        for generation in self.generation_registry.drain_live() {
            generation.shutdown().await;
        }
        self.upstream.shutdown().await;
    }
}

async fn load_cached_tools(
    cache_context: ScopedCodexAppsCacheContext,
) -> Result<Option<Vec<Tool>>> {
    let start = Instant::now();
    let tools = tokio::task::spawn_blocking(move || cache_context.load_tools())
        .await
        .context("Codex Apps cache reader task failed")??;
    if tools.is_some() {
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            start.elapsed(),
            &[("cache", "hit")],
        );
    }
    Ok(tools)
}

async fn write_cached_tools(
    cache_context: ScopedCodexAppsCacheContext,
    tools: Vec<Tool>,
) -> Result<()> {
    let start = Instant::now();
    let result = tokio::task::spawn_blocking(move || cache_context.write_tools(&tools))
        .await
        .context("Codex Apps cache writer task failed")
        .and_then(|result| result);
    emit_duration(MCP_TOOLS_CACHE_WRITE_DURATION_METRIC, start.elapsed(), &[]);
    result
}

fn validate_raw_tool_inventory_size(tool_count: usize) -> Result<()> {
    if tool_count > MAX_CODEX_APPS_TOOLS {
        bail!("Codex Apps raw tool inventory exceeded the {MAX_CODEX_APPS_TOOLS}-tool limit");
    }
    Ok(())
}

struct SerializedToolInventorySize {
    bytes: usize,
    tool_count: usize,
}

impl SerializedToolInventorySize {
    fn empty() -> Self {
        // The serialized representation of an empty tool array is `[]`.
        Self {
            bytes: 2,
            tool_count: 0,
        }
    }

    fn add_page(&mut self, tools: &[Tool], max_bytes: usize) -> Result<()> {
        for tool in tools {
            let tool_bytes = serde_json::to_vec(tool)
                .context("failed to measure Codex Apps tool inventory")?
                .len();
            let separator_bytes = usize::from(self.tool_count > 0);
            let next_bytes = self
                .bytes
                .checked_add(separator_bytes)
                .and_then(|bytes| bytes.checked_add(tool_bytes))
                .context("Codex Apps serialized tool inventory size overflowed")?;
            if next_bytes > max_bytes {
                bail!(
                    "Codex Apps tools/list exceeded the {max_bytes}-byte serialized inventory limit"
                );
            }
            self.bytes = next_bytes;
            self.tool_count += 1;
        }
        Ok(())
    }
}

async fn list_all_upstream_tools(upstream: &Arc<RmcpClient>) -> Result<Vec<Tool>> {
    let start = Instant::now();
    let tools = list_all_upstream_tools_with_timeout(upstream, CODEX_APPS_LOAD_TIMEOUT).await?;
    emit_duration(
        MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
        start.elapsed(),
        &[],
    );
    Ok(tools)
}

fn emit_duration(metric: &str, duration: Duration, tags: &[(&str, &str)]) {
    if let Some(metrics) = codex_otel::global() {
        let _ = metrics.record_duration(metric, duration, tags);
    }
}

async fn list_all_upstream_tools_with_timeout(
    upstream: &Arc<RmcpClient>,
    load_timeout: Duration,
) -> Result<Vec<Tool>> {
    let upstream = Arc::clone(upstream);
    list_all_upstream_tools_with_lister(load_timeout, move |params, remaining| {
        let upstream = Arc::clone(&upstream);
        Box::pin(async move { upstream.list_tools(params, Some(remaining)).await })
    })
    .await
}

type ListToolsPageFuture = Pin<Box<dyn Future<Output = Result<ListToolsResult>> + Send + 'static>>;

async fn list_all_upstream_tools_with_lister(
    load_timeout: Duration,
    list_tools: impl FnMut(Option<PaginatedRequestParams>, Duration) -> ListToolsPageFuture,
) -> Result<Vec<Tool>> {
    list_all_upstream_tools_with_lister_and_inventory_limit(
        load_timeout,
        MAX_CODEX_APPS_TOOL_INVENTORY_BYTES,
        list_tools,
    )
    .await
}

async fn list_all_upstream_tools_with_lister_and_inventory_limit(
    load_timeout: Duration,
    max_inventory_bytes: usize,
    mut list_tools: impl FnMut(Option<PaginatedRequestParams>, Duration) -> ListToolsPageFuture,
) -> Result<Vec<Tool>> {
    let deadline = tokio::time::Instant::now() + load_timeout;
    let mut tools = Vec::new();
    let mut serialized_size = SerializedToolInventorySize::empty();
    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    let mut page_count = 0;
    loop {
        if page_count == MAX_CODEX_APPS_TOOL_PAGES {
            bail!("Codex Apps tools/list exceeded the {MAX_CODEX_APPS_TOOL_PAGES}-page limit");
        }
        let params = cursor
            .clone()
            .map(|cursor| PaginatedRequestParams::default().with_cursor(Some(cursor)));
        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .context("Codex Apps tools/list exceeded the inventory load timeout")?;
        let listed = list_tools(params, remaining)
            .await
            .context("failed to list Codex Apps tools")?;
        page_count += 1;
        let tool_count = tools
            .len()
            .checked_add(listed.tools.len())
            .context("Codex Apps tools/list tool count overflowed")?;
        validate_raw_tool_inventory_size(tool_count)?;
        serialized_size.add_page(&listed.tools, max_inventory_bytes)?;
        tools.extend(listed.tools);
        let Some(next_cursor) = listed.next_cursor else {
            break;
        };
        if !seen_cursors.insert(next_cursor.clone()) {
            bail!("Codex Apps tools/list repeated cursor `{next_cursor}`");
        }
        cursor = Some(next_cursor);
    }
    Ok(tools)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
