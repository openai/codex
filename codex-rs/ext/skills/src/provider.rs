use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

mod host;

use codex_core_skills::HostLoadedSkills;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;

pub use host::HostSkillProvider;

#[derive(Clone, Debug)]
pub struct SkillListQuery {
    pub turn_id: String,
    pub executor_authorities: Vec<SkillAuthority>,
    pub host: Option<Arc<HostLoadedSkills>>,
    pub include_host_skills: bool,
    pub include_bundled_skills: bool,
    pub include_remote_skills: bool,
}

#[derive(Clone, Debug)]
pub struct SkillReadRequest {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub resource: SkillResourceId,
    pub host: Option<Arc<HostLoadedSkills>>,
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
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog>;

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult>;

    fn search(&self, request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult>;
}
