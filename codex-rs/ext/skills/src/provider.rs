use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

mod executor;
mod frozen_cloud;
mod host;

use crate::HostSkillsSnapshot;
use codex_exec_server::EnvironmentAccess;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_extension_api::AccessedSelectedCapabilityRoot;
use codex_mcp::McpResourceClient;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;

pub use executor::ExecutorSkillProvider;
pub(crate) use executor::attribute_executor_plugins;
pub use frozen_cloud::FrozenCloudSkillProvider;
pub use host::HostSkillProvider;

pub(crate) const MAX_SKILL_RESOURCE_CONTENT_BYTES: usize = 1024 * 1024;

/// Catalog inputs with executor roots bound to filesystem access for this callback.
#[derive(Clone)]
pub struct SkillListQuery<'a> {
    /// Absent for explicit cloud discovery before turn admission.
    pub turn_id: Option<String>,
    pub executor_roots: Vec<AccessedSelectedCapabilityRoot<'a>>,
    pub host_snapshot: Option<Arc<HostSkillsSnapshot>>,
    pub include_host_skills: bool,
    pub include_bundled_skills: bool,
    pub include_cloud_skills: bool,
    pub mcp_resources: Option<Arc<McpResourceClient>>,
    /// Present only when the opt-in high-level executor discovery path is selected.
    pub executor_capability_discovery: Option<ExecutorCapabilityDiscoverySnapshot>,
    /// Present when V2 is selected, including empty or failed discovery. Never falls back to V1.
    pub executor_capability_discovery_v2: Option<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>,
}

/// A skill read with the source access supplied by its callback.
#[derive(Clone)]
pub struct SkillReadRequest<'a> {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub resource: SkillResourceId,
    pub context: SkillReadContext<'a>,
}

/// Repository and executor reads require callback access; other sources retain provider context.
#[derive(Clone)]
pub enum SkillReadContext<'a> {
    Host {
        host_snapshot: Option<Arc<HostSkillsSnapshot>>,
        accessors: Vec<&'a dyn EnvironmentAccess>,
    },
    Executor {
        fs: &'a dyn EnvironmentAccess,
    },
    Cloud {
        mcp_resources: Option<Arc<McpResourceClient>>,
    },
    Custom {
        mcp_resources: Option<Arc<McpResourceClient>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillSearchRequest {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub query: String,
}

pub type SkillProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = SkillProviderResult<T>> + Send + 'a>>;

/// Source-specific skill catalog and resource access.
///
/// Implementations must preserve authority boundaries: a resource listed by a
/// provider must be read or searched through the same provider/authority rather
/// than converted into an ambient local path.
pub trait SkillProvider: Send + Sync {
    /// Whether Cloud catalog and read caches must follow the live MCP auth/resource scope.
    /// When all configured Cloud providers opt out, their caches are independent of MCP.
    fn uses_mcp_resources(&self) -> bool {
        true
    }

    /// Providers set `is_partial` when listing omits entries; warnings are diagnostic only.
    fn list<'a>(&'a self, query: SkillListQuery<'a>) -> SkillProviderFuture<'a, SkillCatalog>;

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult>;

    fn search(&self, request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult>;
}
