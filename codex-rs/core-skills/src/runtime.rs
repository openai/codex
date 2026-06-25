use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::hash::Hash;
use std::hash::Hasher;
use std::pin::Pin;
use std::sync::Arc;

use codex_utils_path_uri::PathUri;

use crate::model::SkillDependencies;

/// Source authority that owns a skill package and must be used to read it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SkillSourceKind {
    /// Codex-hosted skills, including bundled, user, repo, plugin-installed,
    /// and downloaded/materialized remote skills.
    Host,
    /// Skills owned by an execution environment.
    Executor,
    /// Skills owned by the orchestrator rather than an execution environment.
    Orchestrator,
}

impl SkillSourceKind {
    fn as_str(&self) -> &str {
        match self {
            Self::Host => "host",
            Self::Executor => "executor",
            Self::Orchestrator => "orchestrator",
        }
    }
}

impl fmt::Display for SkillSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

/// Opaque authority identity for list/read routing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkillAuthority {
    pub kind: SkillSourceKind,
    pub id: String,
}

impl SkillAuthority {
    pub fn new(kind: SkillSourceKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

/// Opaque package id. Callers should not parse local paths out of this value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkillPackageId(pub String);

/// Opaque resource id inside a skill package, optionally bound to the
/// environment path that owns its contents.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkillResourceId {
    id: String,
    environment_path: Option<EnvironmentSkillResource>,
}

impl SkillResourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            environment_path: None,
        }
    }

    pub fn environment(
        id: impl Into<String>,
        environment_id: impl Into<String>,
        path: PathUri,
    ) -> Self {
        Self {
            id: id.into(),
            environment_path: Some(EnvironmentSkillResource {
                environment_id: environment_id.into(),
                path,
            }),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }

    pub fn environment_path(&self) -> Option<(&str, &PathUri)> {
        self.environment_path
            .as_ref()
            .map(|resource| (resource.environment_id.as_str(), &resource.path))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct EnvironmentSkillResource {
    environment_id: String,
    path: PathUri,
}

/// Metadata shown in the always-visible skills catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillCatalogEntry {
    pub id: SkillPackageId,
    pub authority: SkillAuthority,
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub main_prompt: SkillResourceId,
    pub display_path: Option<String>,
    pub dependencies: Option<SkillDependencies>,
    pub enabled: bool,
    pub prompt_visible: bool,
}

impl SkillCatalogEntry {
    pub fn new(
        id: SkillPackageId,
        authority: SkillAuthority,
        name: impl Into<String>,
        description: impl Into<String>,
        main_prompt: SkillResourceId,
    ) -> Self {
        Self {
            id,
            authority,
            name: name.into(),
            description: description.into(),
            short_description: None,
            main_prompt,
            display_path: None,
            dependencies: None,
            enabled: true,
            prompt_visible: true,
        }
    }

    pub fn with_short_description(mut self, short_description: Option<String>) -> Self {
        self.short_description = short_description;
        self
    }

    pub fn with_display_path(mut self, display_path: impl Into<String>) -> Self {
        self.display_path = Some(display_path.into());
        self
    }

    pub fn with_dependencies(mut self, dependencies: Option<SkillDependencies>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn hidden_from_prompt(mut self) -> Self {
        self.prompt_visible = false;
        self
    }

    pub fn rendered_path(&self) -> &str {
        self.display_path
            .as_deref()
            .unwrap_or_else(|| self.main_prompt.as_str())
    }
}

/// Merged catalog for one model step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    pub entries: Vec<SkillCatalogEntry>,
    pub warnings: Vec<String>,
}

impl SkillCatalog {
    pub fn push_entry(&mut self, entry: SkillCatalogEntry) {
        if self
            .entries
            .iter()
            .any(|existing| existing.authority == entry.authority && existing.id == entry.id)
        {
            return;
        }

        self.entries.push(entry);
    }
}

/// A request to read one resource from the source that listed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillReadRequest {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub resource: SkillResourceId,
}

/// Contents returned after resolving a skill resource through its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillReadResult {
    pub resource: SkillResourceId,
    pub contents: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillSourceError {
    pub message: String,
}

impl SkillSourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SkillSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for SkillSourceError {}

pub type SkillSourceResult<T> = Result<T, SkillSourceError>;

/// Boxed future used to keep heterogeneous runtime skill sources object-safe.
pub type SkillSourceFuture<'a, T> = Pin<Box<dyn Future<Output = SkillSourceResult<T>> + Send + 'a>>;

/// Opaque identity of the runtime owner captured by a skill source.
#[derive(Clone)]
pub struct SkillSourceIdentity(Arc<dyn Any + Send + Sync>);

impl SkillSourceIdentity {
    pub fn from_owner<T>(owner: Arc<T>) -> Self
    where
        T: Any + Send + Sync,
    {
        Self(owner)
    }
}

impl fmt::Debug for SkillSourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("SkillSourceIdentity").finish()
    }
}

impl PartialEq for SkillSourceIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SkillSourceIdentity {}

impl Hash for SkillSourceIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as *const ()).hash(state);
    }
}

/// One bound source of runtime skills.
///
/// Implementations capture the authority needed to list and read their skills,
/// such as a host snapshot, one executor filesystem, or an orchestrator client.
/// A resource returned by [`SkillSource::list`] must be read through the same
/// source instead of being converted into an ambient local path.
pub trait SkillSource: Send + Sync {
    /// Returns the stable identity of the runtime owner captured by this source.
    fn identity(&self) -> SkillSourceIdentity;

    /// Lists the skills available from the authority captured by this source.
    fn list(&self) -> SkillSourceFuture<'_, SkillCatalog>;

    /// Reads a resource previously listed by this source.
    fn read(&self, request: SkillReadRequest) -> SkillSourceFuture<'_, SkillReadResult>;
}

#[derive(Clone)]
struct RegisteredSkillSource {
    label: String,
    source: Arc<dyn Fn() -> Arc<dyn SkillSource> + Send + Sync>,
}

/// Bound skill sources used to build and read one runtime catalog.
#[derive(Clone, Default)]
pub struct SkillSources {
    sources: Vec<RegisteredSkillSource>,
}

impl SkillSources {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_source(mut self, label: impl Into<String>, source: Arc<dyn SkillSource>) -> Self {
        self.sources.push(RegisteredSkillSource {
            label: label.into(),
            source: Arc::new(move || Arc::clone(&source)),
        });
        self
    }

    pub fn with_source_factory(
        mut self,
        label: impl Into<String>,
        source: Arc<dyn Fn() -> Arc<dyn SkillSource> + Send + Sync>,
    ) -> Self {
        self.sources.push(RegisteredSkillSource {
            label: label.into(),
            source,
        });
        self
    }

    pub fn extend(&mut self, other: Self) {
        self.sources.extend(other.sources);
    }

    pub(crate) async fn list_with_sources(
        &self,
    ) -> (
        SkillCatalog,
        HashMap<(SkillAuthority, SkillPackageId), Arc<dyn SkillSource>>,
    ) {
        let mut catalog = SkillCatalog::default();
        let mut sources = HashMap::new();
        for source in &self.sources {
            let bound_source = (source.source)();
            match bound_source.list().await {
                Ok(source_catalog) => {
                    catalog.warnings.extend(source_catalog.warnings);
                    for entry in source_catalog.entries {
                        let key = (entry.authority.clone(), entry.id.clone());
                        if let std::collections::hash_map::Entry::Vacant(route) = sources.entry(key)
                        {
                            route.insert(Arc::clone(&bound_source));
                            catalog.entries.push(entry);
                        }
                    }
                }
                Err(err) => catalog.warnings.push(format!(
                    "{} skills unavailable: {}",
                    source.label, err.message
                )),
            }
        }
        (catalog, sources)
    }
}

impl fmt::Debug for SkillSources {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_list()
            .entries(self.sources.iter().map(|source| &source.label))
            .finish()
    }
}
