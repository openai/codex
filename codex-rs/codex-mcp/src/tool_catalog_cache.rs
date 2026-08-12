use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_exec_server::Environment;
use codex_protocol::mcp::ClientMcpExtensions;
use lru::LruCache;
use rmcp::model::ElicitationCapability;
use sha1::Digest;
use sha1::Sha1;
use tokio::time::Instant;

use crate::McpProtocolMode;
use crate::McpRuntimeContext;
use crate::ToolInfo;
use crate::server::McpServerConnectionIdentity;
use crate::server::has_explicit_http_authorization;

const TOOL_CATALOG_CACHE_CAPACITY: usize = 32;
const TOOL_CATALOG_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

/// Process-scoped cache of recent reusable tool definitions for MCP servers.
#[derive(Clone)]
pub struct McpToolCatalogCache {
    entries: Arc<Mutex<LruCache<ToolCatalogIdentity, Arc<ToolCatalogCacheEntry>>>>,
}

impl Default for McpToolCatalogCache {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(TOOL_CATALOG_CACHE_CAPACITY).unwrap_or(NonZeroUsize::MIN),
            ))),
        }
    }
}

struct ToolCatalogCacheEntry {
    state: Mutex<ToolCatalogCacheState>,
    next_fetch_generation: AtomicU64,
}

#[derive(Default)]
struct ToolCatalogCacheState {
    snapshot: Option<ToolCatalogSnapshot>,
    optional_startup_deadline: Option<Instant>,
    last_accepted_generation: u64,
    disabled_by_server: bool,
}

struct ToolCatalogSnapshot {
    tools: Vec<ToolInfo>,
    published_at: Instant,
}

#[derive(Clone)]
pub(crate) struct McpToolCatalogCacheContext {
    entry: Arc<ToolCatalogCacheEntry>,
}

pub(crate) struct McpToolCatalogFetchTicket {
    generation: u64,
}

impl McpToolCatalogCache {
    pub(crate) fn context(
        &self,
        server_name: &str,
        config: &McpServerConfig,
        runtime_context: &McpRuntimeContext,
        resolved_environment: Option<&Arc<Environment>>,
        client_context: (&ElicitationCapability, &ClientMcpExtensions),
        connection_identity: Option<(&McpServerConnectionIdentity, McpProtocolMode, bool)>,
    ) -> Option<McpToolCatalogCacheContext> {
        let identity = ToolCatalogIdentity::new(
            server_name,
            config,
            runtime_context,
            resolved_environment,
            client_context,
            connection_identity,
        )?;
        let entry = lock_unpoisoned(&self.entries)
            .get_or_insert(identity, || Arc::new(ToolCatalogCacheEntry::default()))
            .clone();
        Some(McpToolCatalogCacheContext { entry })
    }
}

impl Default for ToolCatalogCacheEntry {
    fn default() -> Self {
        Self {
            state: Mutex::new(ToolCatalogCacheState::default()),
            next_fetch_generation: AtomicU64::new(0),
        }
    }
}

impl McpToolCatalogCacheContext {
    pub(crate) fn has_tools(&self) -> bool {
        self.current_tools().is_some_and(|tools| !tools.is_empty())
    }

    pub(crate) fn optional_startup_deadline(&self, default_deadline: Instant) -> Instant {
        let mut state = lock_unpoisoned(&self.entry.state);
        if state.disabled_by_server
            || state
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.published_at.elapsed() <= TOOL_CATALOG_CACHE_TTL)
        {
            return default_deadline;
        }
        *state
            .optional_startup_deadline
            .get_or_insert(default_deadline)
    }

    pub(crate) fn current_tools(&self) -> Option<Vec<ToolInfo>> {
        lock_unpoisoned(&self.entry.state)
            .snapshot
            .as_ref()
            .filter(|snapshot| snapshot.published_at.elapsed() <= TOOL_CATALOG_CACHE_TTL)
            .map(|snapshot| snapshot.tools.clone())
    }

    pub(crate) fn begin_fetch(&self) -> McpToolCatalogFetchTicket {
        McpToolCatalogFetchTicket {
            generation: self
                .entry
                .next_fetch_generation
                .fetch_add(1, Ordering::Relaxed)
                + 1,
        }
    }

    pub(crate) fn disable(&self) {
        let mut state = lock_unpoisoned(&self.entry.state);
        state.disabled_by_server = true;
        state.snapshot = None;
    }

    pub(crate) fn publish_if_newest(&self, ticket: McpToolCatalogFetchTicket, tools: &[ToolInfo]) {
        let mut state = lock_unpoisoned(&self.entry.state);
        if state.disabled_by_server || ticket.generation <= state.last_accepted_generation {
            return;
        }

        let mut tools = tools.to_vec();
        for tool in &mut tools {
            // Initialize instructions belong to one live connection and must not cross sessions.
            tool.namespace_description = None;
            // Tool annotations affect approval and parallelism decisions, so only the live
            // connection may supply them.
            tool.tool.annotations = None;
        }
        state.last_accepted_generation = ticket.generation;
        state.optional_startup_deadline = None;
        state.snapshot = Some(ToolCatalogSnapshot {
            tools,
            published_at: Instant::now(),
        });
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct ToolCatalogIdentity {
    server_name: String,
    transport: ToolCatalogTransportIdentity,
    environment: Option<Weak<Environment>>,
    local_stdio_fallback_cwd: Option<PathBuf>,
}

impl PartialEq for ToolCatalogIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.server_name == other.server_name
            && self.transport == other.transport
            && self.local_stdio_fallback_cwd == other.local_stdio_fallback_cwd
            && match (&self.environment, &other.environment) {
                (Some(environment), Some(other)) => Weak::ptr_eq(environment, other),
                (None, None) => true,
                _ => false,
            }
    }
}

impl Eq for ToolCatalogIdentity {}

impl Hash for ToolCatalogIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.server_name.hash(state);
        self.transport.hash(state);
        self.local_stdio_fallback_cwd.hash(state);
        self.environment
            .as_ref()
            .map(|environment| Weak::as_ptr(environment) as usize)
            .hash(state);
    }
}

impl ToolCatalogIdentity {
    fn new(
        server_name: &str,
        config: &McpServerConfig,
        runtime_context: &McpRuntimeContext,
        environment: Option<&Arc<Environment>>,
        client_context: (&ElicitationCapability, &ClientMcpExtensions),
        connection_identity: Option<(&McpServerConnectionIdentity, McpProtocolMode, bool)>,
    ) -> Option<Self> {
        let transport =
            ToolCatalogTransportIdentity::new(config, client_context, connection_identity)?;
        Some(Self {
            server_name: server_name.to_string(),
            transport,
            environment: environment.map(Arc::downgrade),
            local_stdio_fallback_cwd: matches!(
                &config.transport,
                McpServerTransportConfig::Stdio { cwd: None, .. }
            )
            .then(|| runtime_context.local_process_cwd()),
        })
    }
}

#[derive(PartialEq, Eq, Hash)]
enum ToolCatalogTransportIdentity {
    Stdio { fingerprint: [u8; 20] },
    StreamableHttp { fingerprint: [u8; 20] },
}

impl ToolCatalogTransportIdentity {
    fn new(
        config: &McpServerConfig,
        client_context: (&ElicitationCapability, &ClientMcpExtensions),
        connection_identity: Option<(&McpServerConnectionIdentity, McpProtocolMode, bool)>,
    ) -> Option<Self> {
        let (client_elicitation_capability, client_mcp_extensions) = client_context;
        if let McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
            http_headers_helper,
        } = &config.transport
        {
            // Helper output is a dynamic credential identity that cannot be represented by config.
            if http_headers_helper.is_some() {
                return None;
            }
            let (connection_identity, protocol_mode, agent_plugin) = connection_identity?;
            if config.oauth.is_some()
                || config.scopes.is_some()
                || config.oauth_resource.is_some()
                || (matches!(config.auth, McpServerAuth::ChatGpt)
                    && !has_explicit_http_authorization(config))
                || (!has_explicit_http_authorization(config)
                    && connection_identity.oauth_credentials().ok()?.is_some())
            {
                return None;
            }

            let mut hasher = Sha1::new();
            hasher.update(
                serde_json::to_vec(&(
                    url,
                    bearer_token_env_var,
                    http_headers
                        .as_ref()
                        .map(|headers| headers.iter().collect::<BTreeMap<_, _>>()),
                    env_http_headers
                        .as_ref()
                        .map(|headers| headers.iter().collect::<BTreeMap<_, _>>()),
                    &config.auth,
                    &config.environment_id,
                    agent_plugin,
                    protocol_mode.preferred_protocol_version().as_str(),
                    client_elicitation_capability,
                    client_mcp_extensions.iter().collect::<BTreeMap<_, _>>(),
                ))
                .ok()?,
            );
            let mut env_vars = bearer_token_env_var
                .iter()
                .chain(env_http_headers.iter().flat_map(|headers| headers.values()))
                .collect::<Vec<_>>();
            env_vars.sort_unstable();
            env_vars.dedup();
            for name in env_vars {
                hasher.update(name.as_bytes());
                let mut value_hasher = DefaultHasher::new();
                std::env::var_os(name).hash(&mut value_hasher);
                hasher.update(value_hasher.finish().to_le_bytes());
            }
            return Some(Self::StreamableHttp {
                fingerprint: hasher.finalize().into(),
            });
        }
        let McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } = &config.transport
        else {
            return None;
        };
        if env_vars
            .iter()
            .any(codex_config::McpServerEnvVar::is_remote_source)
        {
            return None;
        }

        let mut hasher = Sha1::new();
        let env = env.as_ref().map(|env| {
            env.iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect::<BTreeMap<_, _>>()
        });
        hasher.update(
            serde_json::to_vec(&(
                command,
                args,
                env,
                env_vars,
                cwd,
                &config.environment_id,
                client_elicitation_capability,
                client_mcp_extensions.iter().collect::<BTreeMap<_, _>>(),
            ))
            .ok()?,
        );
        for env_var in env_vars {
            hasher.update(env_var.name().as_bytes());
            let mut value_hasher = DefaultHasher::new();
            std::env::var_os(env_var.name()).hash(&mut value_hasher);
            hasher.update(value_hasher.finish().to_le_bytes());
        }

        Some(Self::Stdio {
            fingerprint: hasher.finalize().into(),
        })
    }
}
