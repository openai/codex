use codex_core_skills::model::SkillDependencies;

/// Source authority that owns a skill package and must be used to read it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SkillSourceKind {
    /// Codex-hosted skills, including bundled, user, repo, plugin-installed,
    /// and downloaded/materialized remote skills.
    Host,
    /// Skills owned by an execution environment.
    Executor,
    /// Skills read through an authenticated remote catalog/API.
    Remote,
    /// Extension-private source kind for future providers that do not fit an
    /// existing transport category.
    Custom(String),
}

impl SkillSourceKind {
    fn as_str(&self) -> &str {
        match self {
            Self::Host => "host",
            Self::Executor => "executor",
            Self::Remote => "remote",
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

/// Opaque resource id inside a skill package.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkillResourceId(pub String);

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

    pub fn with_display_path(mut self, display_path: impl Into<String>) -> Self {
        self.display_path = Some(display_path.into());
        self
    }

    pub fn hidden_from_prompt(mut self) -> Self {
        self.prompt_visible = false;
        self
    }

    pub(crate) fn rendered_path(&self) -> &str {
        self.display_path
            .as_deref()
            .unwrap_or(self.main_prompt.0.as_str())
    }
}

/// Merged catalog for one turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    pub entries: Vec<SkillCatalogEntry>,
    pub warnings: Vec<String>,
}

impl SkillCatalog {
    pub fn extend(&mut self, other: SkillCatalog) {
        for entry in other.entries {
            self.push_entry(entry);
        }
        self.warnings.extend(other.warnings);
    }

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
