/// Source authority that owns a skill package and must be used to read it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SkillSourceKind {
    Host,
    Executor,
    Remote,
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
    pub entrypoint: SkillResourceId,
}

/// Merged catalog for one turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillCatalog {
    pub entries: Vec<SkillCatalogEntry>,
    pub warnings: Vec<String>,
}

impl SkillCatalog {
    pub fn extend(&mut self, other: SkillCatalog) {
        // TODO(skills-extension): dedupe by authority-bound id first, then
        // apply name precedence/conflict rules for user-facing mention
        // resolution. Names are not stable identities.
        self.entries.extend(other.entries);
        self.warnings.extend(other.warnings);
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

pub type SkillProviderResult<T> = Result<T, SkillProviderError>;
