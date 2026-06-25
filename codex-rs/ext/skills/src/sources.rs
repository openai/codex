use std::fmt;
use std::sync::Arc;

use codex_core_skills::runtime::SkillReadRequest as RuntimeSkillReadRequest;
use codex_core_skills::runtime::SkillSource;
use codex_core_skills::runtime::SkillSourceFuture;
use codex_core_skills::runtime::SkillSourceIdentity;
use codex_core_skills::runtime::SkillSources;
use codex_mcp::McpResourceClient;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSearchResult;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;
use crate::provider::SkillReadRequest;
use crate::provider::SkillSearchRequest;

#[derive(Clone)]
pub struct SkillProviderSource {
    kind: SkillSourceKind,
    label: String,
    provider: Arc<dyn SkillProvider>,
    identity: SkillSourceIdentity,
}

impl SkillProviderSource {
    pub fn new(
        kind: SkillSourceKind,
        label: impl Into<String>,
        provider: Arc<dyn SkillProvider>,
    ) -> Self {
        let identity = SkillSourceIdentity::from_owner(Arc::new(Arc::clone(&provider)));
        Self {
            kind,
            label: label.into(),
            provider,
            identity,
        }
    }

    pub fn host(label: impl Into<String>, provider: Arc<dyn SkillProvider>) -> Self {
        Self::new(SkillSourceKind::Host, label, provider)
    }

    pub fn executor(label: impl Into<String>, provider: Arc<dyn SkillProvider>) -> Self {
        Self::new(SkillSourceKind::Executor, label, provider)
    }

    pub fn orchestrator(label: impl Into<String>, provider: Arc<dyn SkillProvider>) -> Self {
        Self::new(SkillSourceKind::Orchestrator, label, provider)
    }

    fn should_list(&self, query: &SkillListQuery) -> bool {
        match &self.kind {
            SkillSourceKind::Host => query.include_host_skills,
            SkillSourceKind::Executor => !query.executor_roots.is_empty(),
            SkillSourceKind::Orchestrator => query.include_orchestrator_skills,
            SkillSourceKind::Custom(_) => true,
        }
    }

    fn owns_kind(&self, kind: &SkillSourceKind) -> bool {
        &self.kind == kind
    }

    fn bind(&self, query: SkillListQuery) -> Arc<dyn SkillSource> {
        Arc::new(BoundSkillProvider {
            kind: self.kind.clone(),
            provider: Arc::clone(&self.provider),
            identity: self.identity.clone(),
            host_snapshot: query.host_snapshot.clone(),
            mcp_resources: query.mcp_resources.clone(),
            query: Some(query),
        })
    }
}

impl fmt::Debug for SkillProviderSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SkillProviderSource")
            .field("kind", &self.kind)
            .field("label", &self.label)
            .finish()
    }
}

#[derive(Clone, Default, Debug)]
pub struct SkillProviders {
    sources: Vec<SkillProviderSource>,
}

impl SkillProviders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(mut self, source: SkillProviderSource) -> Self {
        self.sources.push(source);
        self
    }

    pub fn with_host_provider(mut self, provider: Arc<dyn SkillProvider>) -> Self {
        self.sources
            .push(SkillProviderSource::host("host", provider));
        self
    }

    pub fn with_executor_provider(mut self, provider: Arc<dyn SkillProvider>) -> Self {
        self.sources
            .push(SkillProviderSource::executor("executor", provider));
        self
    }

    pub fn with_orchestrator_provider(mut self, provider: Arc<dyn SkillProvider>) -> Self {
        self.sources
            .push(SkillProviderSource::orchestrator("orchestrator", provider));
        self
    }

    pub(crate) fn has_orchestrator_provider(&self) -> bool {
        self.sources
            .iter()
            .any(|source| source.kind == SkillSourceKind::Orchestrator)
    }

    pub(crate) async fn list_for_turn(&self, query: SkillListQuery) -> SkillCatalog {
        self.sources_for_turn(query).list().await
    }

    pub(crate) async fn list_orchestrator_for_turn(
        &self,
        query: SkillListQuery,
    ) -> SkillProviderResult<SkillCatalog> {
        self.sources_for_turn(query)
            .list_kind(&SkillSourceKind::Orchestrator)
            .await
    }

    fn sources_for_turn(&self, query: SkillListQuery) -> SkillSources {
        self.sources
            .iter()
            .filter(|source| source.should_list(&query))
            .fold(SkillSources::new(), |sources, source| {
                sources.with_source(source.label.clone(), source.bind(query.clone()))
            })
    }

    pub(crate) async fn read(
        &self,
        request: SkillReadRequest,
    ) -> Result<SkillReadResult, SkillProviderError> {
        let mut sources = self
            .sources
            .iter()
            .filter(|source| source.owns_kind(&request.authority.kind));
        let Some(source) = sources.next() else {
            return Err(SkillProviderError::new(format!(
                "{} skill provider is not configured",
                request.authority.kind
            )));
        };
        if sources.next().is_some() {
            return Err(SkillProviderError::new(format!(
                "{} skill authority is ambiguous",
                request.authority.kind
            )));
        }
        source.provider.read(request).await
    }

    pub async fn search(
        &self,
        request: SkillSearchRequest,
    ) -> Result<SkillSearchResult, SkillProviderError> {
        let mut last_error = None;
        for source in self
            .sources
            .iter()
            .filter(|source| source.owns_kind(&request.authority.kind))
        {
            match source.provider.search(request.clone()).await {
                Ok(result) => return Ok(result),
                Err(err) => last_error = Some(err),
            }
        }

        match last_error {
            Some(err) => Err(err),
            None => Err(SkillProviderError::new(format!(
                "{} skill provider is not configured",
                request.authority.kind
            ))),
        }
    }
}

struct BoundSkillProvider {
    kind: SkillSourceKind,
    provider: Arc<dyn SkillProvider>,
    identity: SkillSourceIdentity,
    host_snapshot: Option<Arc<codex_core_skills::HostSkillsSnapshot>>,
    mcp_resources: Option<Arc<McpResourceClient>>,
    query: Option<SkillListQuery>,
}

impl SkillSource for BoundSkillProvider {
    fn kind(&self) -> SkillSourceKind {
        self.kind.clone()
    }

    fn identity(&self) -> SkillSourceIdentity {
        self.identity.clone()
    }

    fn list(&self) -> SkillSourceFuture<'_, SkillCatalog> {
        let Some(query) = self.query.clone() else {
            return Box::pin(async {
                Err(SkillProviderError::new(
                    "skill source was not bound for catalog listing",
                ))
            });
        };
        self.provider.list(query)
    }

    fn read(&self, request: RuntimeSkillReadRequest) -> SkillSourceFuture<'_, SkillReadResult> {
        self.provider.read(SkillReadRequest {
            authority: request.authority,
            package: request.package,
            resource: request.resource,
            host_snapshot: self.host_snapshot.clone(),
            mcp_resources: self.mcp_resources.clone(),
        })
    }
}
