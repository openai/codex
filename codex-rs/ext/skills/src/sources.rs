use std::fmt;
use std::sync::Arc;

use codex_core_skills::HostLoadedSkills;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSearchResult;
use crate::catalog::SkillSourceKind;
use crate::provider::HostSkillProvider;
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

    pub fn remote(label: impl Into<String>, provider: Arc<dyn SkillProvider>) -> Self {
        Self::new(SkillSourceKind::Remote, label, provider)
    }

    fn should_list(&self, query: &SkillListQuery) -> bool {
        match &self.kind {
            SkillSourceKind::Host => query.include_host_skills,
            SkillSourceKind::Executor => !query.executor_authorities.is_empty(),
            SkillSourceKind::Remote => query.include_remote_skills,
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
    host_provider: Option<Arc<HostSkillProvider>>,
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

    pub fn with_host_provider(mut self, provider: Arc<HostSkillProvider>) -> Self {
        self.host_provider = Some(provider);
        self
    }

    pub fn with_executor_provider(mut self, provider: Arc<dyn SkillProvider>) -> Self {
        self.sources
            .push(SkillProviderSource::executor("executor", provider));
        self
    }

    pub fn with_remote_provider(mut self, provider: Arc<dyn SkillProvider>) -> Self {
        self.sources
            .push(SkillProviderSource::remote("remote", provider));
        self
    }

    pub(crate) async fn list_for_turn(
        &self,
        query: SkillListQuery,
        host_loaded_skills: Option<Arc<HostLoadedSkills>>,
    ) -> SkillCatalog {
        let mut catalog = SkillCatalog::default();

        if query.include_host_skills
            && let Some(provider) = &self.host_provider
        {
            let result = match host_loaded_skills.as_deref() {
                Some(host_loaded_skills) => Ok(provider.list(host_loaded_skills)),
                None => Err(SkillProviderError::new(
                    "host skill provider requires loaded host skills",
                )),
            };
            extend_catalog(&mut catalog, result, "host");
        }

        for source in self
            .sources
            .iter()
            .filter(|source| source.should_list(&query))
        {
            extend_catalog(
                &mut catalog,
                source.provider.list(query.clone()).await,
                source.label.as_str(),
            );
        }

        catalog
    }

    pub(crate) async fn read_host(
        &self,
        request: SkillReadRequest,
        host_loaded_skills: Option<Arc<HostLoadedSkills>>,
    ) -> Result<SkillReadResult, SkillProviderError> {
        let mut last_error = None;
        if let Some(provider) = &self.host_provider {
            match host_loaded_skills.as_deref() {
                Some(host_loaded_skills) => {
                    match provider.read(request.clone(), host_loaded_skills).await {
                        Ok(result) => return Ok(result),
                        Err(err) => last_error = Some(err),
                    }
                }
                None => {
                    last_error = Some(SkillProviderError::new(
                        "host skill provider requires loaded host skills",
                    ));
                }
            }
        }

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
