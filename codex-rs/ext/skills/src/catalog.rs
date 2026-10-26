//! Skill catalog data and reconciliation, retaining source-local policy and read routes.

use codex_extension_api::SelectedPluginSnapshot;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillDependencies;
use codex_utils_path_uri::PathUri;
use std::sync::Arc;

/// Source authority that owns a skill package and must be used to read it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SkillSourceKind {
    /// Codex-hosted skills, including bundled, user, repo, plugin-installed,
    /// and downloaded/materialized remote skills.
    Host,
    /// Skills owned by an execution environment.
    Executor,
    /// Skills owned by the cloud rather than an execution environment.
    Cloud,
    /// Extension-private source kind for future providers that do not fit an
    /// existing transport category.
    Custom(String),
}

impl SkillSourceKind {
    pub fn custom(kind: impl Into<String>) -> Self {
        Self::Custom(kind.into())
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Host => "host",
            Self::Executor => "executor",
            Self::Cloud => "cloud",
            Self::Custom(kind) => kind,
        }
    }
}

impl std::fmt::Display for SkillSourceKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl SkillPackageId {
    pub(crate) fn relative_resource_path<'a>(&self, resource: &'a str) -> Option<&'a str> {
        let relative = resource
            .strip_prefix(self.0.trim_end_matches('/'))?
            .strip_prefix('/')?;
        (!relative.is_empty()
            && relative
                .split('/')
                .all(|segment| !matches!(segment, "" | "." | "..")))
        .then_some(relative)
    }
}

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
        let package_root = path.parent().unwrap_or_else(|| path.clone());
        Self {
            id: id.into(),
            environment_path: Some(EnvironmentSkillResource {
                environment_id: environment_id.into(),
                package_root,
                path,
                contents: None,
            }),
        }
    }

    pub fn environment_with_contents(
        id: impl Into<String>,
        environment_id: impl Into<String>,
        path: PathUri,
        contents: String,
    ) -> Self {
        let package_root = path.parent().unwrap_or_else(|| path.clone());
        Self {
            id: id.into(),
            environment_path: Some(EnvironmentSkillResource {
                environment_id: environment_id.into(),
                package_root,
                path,
                contents: Some(contents.into()),
            }),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }

    pub fn environment_id(&self) -> Option<&str> {
        self.environment_path
            .as_ref()
            .map(|resource| resource.environment_id.as_str())
    }

    pub(crate) fn bind_environment_package_resource(
        &self,
        package: &SkillPackageId,
        resource: impl Into<String>,
    ) -> Option<Self> {
        let resource = resource.into();
        let relative = package.relative_resource_path(&resource)?;
        let environment = self.environment_path.as_ref()?;
        let path = environment.package_root.join(relative).ok()?;
        path.starts_with(&environment.package_root).then(|| Self {
            id: resource,
            environment_path: Some(EnvironmentSkillResource {
                environment_id: environment.environment_id.clone(),
                package_root: environment.package_root.clone(),
                path,
                contents: None,
            }),
        })
    }

    pub(crate) fn environment_path(&self) -> Option<(&str, &PathUri)> {
        self.environment_path
            .as_ref()
            .map(|resource| (resource.environment_id.as_str(), &resource.path))
    }

    pub(crate) fn environment_contents(&self) -> Option<&str> {
        self.environment_path
            .as_ref()
            .and_then(|resource| resource.contents.as_deref())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct EnvironmentSkillResource {
    environment_id: String,
    package_root: PathUri,
    path: PathUri,
    contents: Option<Arc<str>>,
}

/// Skill metadata and source-owned read routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillCatalogEntry {
    pub id: SkillPackageId,
    pub authority: SkillAuthority,
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub main_prompt: SkillResourceId,
    pub display_path: Option<String>,
    pub canonical_skill_id: Option<String>,
    pub plugin_id: Option<String>,
    pub analytics_scope: Option<SkillScope>,
    /// Plugin-service identity for cross-source matching.
    pub remote_plugin_id: Option<String>,
    /// Alternate read routes that retain their original authority.
    pub sources: Vec<SkillSourceLocation>,
    alias_root: Option<String>,
    alias_root_order: Option<usize>,
    prompt_scope: Option<SkillScope>,
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
            canonical_skill_id: None,
            plugin_id: None,
            analytics_scope: None,
            remote_plugin_id: None,
            sources: Vec::new(),
            alias_root: None,
            alias_root_order: None,
            prompt_scope: None,
            dependencies: None,
            enabled: true,
            prompt_visible: true,
        }
    }

    pub fn with_remote_plugin_id(mut self, remote_plugin_id: Option<String>) -> Self {
        self.remote_plugin_id = remote_plugin_id;
        self
    }

    pub fn with_plugin_id(mut self, plugin_id: Option<String>) -> Self {
        self.plugin_id = plugin_id;
        self
    }

    pub fn with_short_description(mut self, short_description: Option<String>) -> Self {
        self.short_description = short_description;
        self
    }

    pub fn with_display_path(mut self, display_path: impl Into<String>) -> Self {
        self.display_path = Some(display_path.into());
        self
    }

    /// Sets the shared locator prefix that may be compacted in model-visible skill catalogs.
    pub fn with_alias_root(mut self, alias_root: impl Into<String>) -> Self {
        self.alias_root = Some(alias_root.into());
        self
    }

    pub(crate) fn with_alias_root_order(mut self, alias_root_order: usize) -> Self {
        self.alias_root_order = Some(alias_root_order);
        self
    }

    pub(crate) fn with_prompt_scope(mut self, prompt_scope: SkillScope) -> Self {
        self.prompt_scope = Some(prompt_scope);
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

    pub(crate) fn is_model_visible(&self) -> bool {
        self.enabled && self.prompt_visible
    }

    pub(crate) fn primary_source(&self) -> SkillSourceLocation {
        SkillSourceLocation {
            authority: self.authority.clone(),
            package: self.id.clone(),
            main_prompt: self.main_prompt.clone(),
            display_path: self.display_path.clone(),
            plugin_id: self.plugin_id.clone(),
            enabled: self.enabled,
            alias_root: self.alias_root.clone(),
            prompt_visible: self.prompt_visible,
        }
    }

    pub(crate) fn source_locations(&self) -> impl Iterator<Item = SkillSourceLocation> + '_ {
        std::iter::once(self.primary_source()).chain(self.sources.iter().cloned())
    }

    pub(crate) fn unaliased_locator(&self) -> &str {
        match &self.authority.kind {
            SkillSourceKind::Executor | SkillSourceKind::Cloud => self.id.0.as_str(),
            SkillSourceKind::Host | SkillSourceKind::Custom(_) => self.rendered_path(),
        }
    }

    pub(crate) fn rendered_path(&self) -> &str {
        self.display_path
            .as_deref()
            .unwrap_or_else(|| self.main_prompt.as_str())
    }

    pub(crate) fn alias_root(&self) -> Option<&str> {
        self.alias_root.as_deref()
    }

    pub(crate) fn alias_root_order(&self) -> Option<usize> {
        self.alias_root_order
    }

    pub(crate) fn prompt_scope(&self) -> Option<SkillScope> {
        self.prompt_scope
    }
}

/// One original provider route retained by a reconciled skill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillSourceLocation {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub main_prompt: SkillResourceId,
    pub display_path: Option<String>,
    /// Installation attribution belongs to the route used, not the merged primary entry.
    pub plugin_id: Option<String>,
    pub enabled: bool,
    pub alias_root: Option<String>,
    pub prompt_visible: bool,
}

/// Merged catalog for one turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    pub entries: Vec<SkillCatalogEntry>,
    pub warnings: Vec<String>,
    pub is_partial: bool,
}

impl SkillCatalog {
    pub fn extend(&mut self, other: SkillCatalog) {
        for entry in other.entries {
            self.push_entry(entry);
        }
        self.warnings.extend(other.warnings);
        self.is_partial |= other.is_partial;
    }

    pub fn push_entry(&mut self, entry: SkillCatalogEntry) {
        if self.entries.iter().any(|existing| {
            existing.authority == entry.authority
                && existing.id == entry.id
                && existing.main_prompt.environment_id() == entry.main_prompt.environment_id()
        }) {
            return;
        }

        self.entries.push(entry);
    }
}

/// Applies plugin denials and requires accepted Cloud or executor roots before merging skills.
pub(crate) fn apply_plugin_denials(catalog: &mut SkillCatalog, snapshot: &SelectedPluginSnapshot) {
    for skill in &mut catalog.entries {
        if !matches!(
            skill.authority.kind,
            SkillSourceKind::Cloud | SkillSourceKind::Executor
        ) {
            continue;
        }
        // Standalone skills do not require plugin acceptance.
        if skill.remote_plugin_id.is_none() && skill.plugin_id.is_none() {
            continue;
        }
        let accepted = if skill.authority.kind == SkillSourceKind::Executor {
            let (Some(environment_id), Some(plugin_id)) =
                (skill.main_prompt.environment_id(), &skill.plugin_id)
            else {
                skill.enabled = false;
                continue;
            };
            let root_id = format!("{environment_id}:{plugin_id}");
            snapshot
                .plugins
                .iter()
                .any(|plugin| plugin.selected_root_id.as_deref() == Some(root_id.as_str()))
        } else {
            let Some(remote_plugin_id) = &skill.remote_plugin_id else {
                skill.enabled = false;
                continue;
            };
            snapshot.plugins.iter().any(|plugin| {
                plugin.selected_root_id.is_none() && plugin.plugin_id.as_str() == remote_plugin_id
            })
        };
        skill.enabled &= skill
            .remote_plugin_id
            .as_ref()
            .is_none_or(|id| !snapshot.disabled_plugin_ids.contains(id))
            && skill
                .plugin_id
                .as_ref()
                .is_none_or(|id| !snapshot.disabled_plugin_ids.contains(id))
            && accepted;
    }
}

fn remote_key(entry: &SkillCatalogEntry) -> Option<(&str, &str)> {
    Some((entry.remote_plugin_id.as_deref()?, entry.name.as_str()))
}

/// Merges available Cloud counterparts, disabling unmatched remote skills only when Cloud is complete.
pub fn merge_catalogs(cloud: Option<SkillCatalog>, executor: SkillCatalog) -> SkillCatalog {
    let Some(mut merged) = cloud else {
        return executor;
    };
    let cloud_is_partial = merged.is_partial;
    merged.is_partial |= executor.is_partial;
    let cloud_entry_count = merged.entries.len();
    merged.warnings.extend(executor.warnings);
    for mut entry in executor.entries {
        let Some(key) = remote_key(&entry) else {
            merged.entries.push(entry);
            continue;
        };
        // Executor entries cannot authorize or merge with one another.
        let Some(cloud_entry) = merged.entries[..cloud_entry_count]
            .iter_mut()
            .find(|cloud_entry| remote_key(cloud_entry) == Some(key))
        else {
            if !cloud_is_partial {
                entry.enabled = false;
                for source in &mut entry.sources {
                    source.enabled = false;
                }
            }
            merged.entries.push(entry);
            continue;
        };
        let primary_source = entry.primary_source();
        for mut source in std::iter::once(primary_source).chain(entry.sources) {
            source.enabled &= cloud_entry.enabled;
            let already_present = cloud_entry.sources.iter().any(|existing| {
                existing.authority == source.authority
                    && existing.package == source.package
                    && existing.main_prompt.environment_id() == source.main_prompt.environment_id()
            });
            if !already_present {
                cloud_entry.sources.push(source);
            }
        }
    }
    merged
}

/// Contents returned after resolving a skill resource through its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillReadResult {
    pub resource: SkillResourceId,
    pub contents: String,
}

/// Search results for a package whose files are not readable through ordinary
/// executor filesystem access.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillSearchResult {
    pub matches: Vec<SkillSearchMatch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillSearchMatch {
    pub resource: SkillResourceId,
    pub title: String,
    pub snippet: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillProviderError {
    pub message: String,
}

impl SkillProviderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SkillProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for SkillProviderError {}

pub type SkillProviderResult<T> = Result<T, SkillProviderError>;

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
