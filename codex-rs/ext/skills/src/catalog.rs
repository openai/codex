pub use codex_core_skills::runtime::SkillAuthority;
pub use codex_core_skills::runtime::SkillCatalog;
pub use codex_core_skills::runtime::SkillCatalogEntry;
pub use codex_core_skills::runtime::SkillPackageId;
pub use codex_core_skills::runtime::SkillReadResult;
pub use codex_core_skills::runtime::SkillResourceId;
pub use codex_core_skills::runtime::SkillSourceError as SkillProviderError;
pub use codex_core_skills::runtime::SkillSourceKind;
pub use codex_core_skills::runtime::SkillSourceResult as SkillProviderResult;

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
