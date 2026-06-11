use std::fmt;
use std::sync::Arc;

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
}

impl SkillProviderSource {
    pub fn new(
        kind: SkillSourceKind,
        label: impl Into<String>,
        provider: Arc<dyn SkillProvider>,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            provider,
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

    pub(crate) async fn list_for_turn(&self, query: SkillListQuery) -> SkillCatalog {
        self.list_matching(&query, |source| source.should_list(&query))
            .await
    }

    pub(crate) async fn list_orchestrator_for_turn(
        &self,
        query: SkillListQuery,
    ) -> SkillProviderResult<SkillCatalog> {
        let mut catalog = SkillCatalog::default();

        for source in self
            .sources
            .iter()
            .filter(|source| source.kind == SkillSourceKind::Orchestrator)
        {
            let source_catalog = source.provider.list(query.clone()).await.map_err(|err| {
                SkillProviderError::new(format!(
                    "{} skills unavailable: {}",
                    source.label, err.message
                ))
            })?;
            catalog.extend(source_catalog);
        }

        Ok(catalog)
    }

    async fn list_matching(
        &self,
        query: &SkillListQuery,
        should_list: impl Fn(&SkillProviderSource) -> bool,
    ) -> SkillCatalog {
        let mut catalog = SkillCatalog::default();

        for source in self.sources.iter().filter(|source| should_list(source)) {
            extend_catalog(
                &mut catalog,
                source.provider.list(query.clone()).await,
                source.label.as_str(),
            );
        }

        catalog
    }

    pub(crate) async fn read(
        &self,
        request: SkillReadRequest,
    ) -> Result<SkillReadResult, SkillProviderError> {
        let mut last_error = None;
        for source in self
            .sources
            .iter()
            .filter(|source| source.owns_kind(&request.authority.kind))
        {
            match source.provider.read(request.clone()).await {
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

fn extend_catalog(
    catalog: &mut SkillCatalog,
    result: Result<SkillCatalog, SkillProviderError>,
    label: &str,
) {
    match result {
        Ok(source_catalog) => catalog.extend(source_catalog),
        Err(err) => catalog
            .warnings
            .push(format!("{label} skills unavailable: {}", err.message)),
    }
}
