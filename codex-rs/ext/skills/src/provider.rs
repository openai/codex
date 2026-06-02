use std::future::Future;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillListQuery {
    pub turn_id: String,
    pub executor_authorities: Vec<SkillAuthority>,
    pub include_host_skills: bool,
    pub include_remote_skills: bool,
}

impl SkillListQuery {
    pub(crate) fn placeholder_for_turn(turn_id: &str) -> Self {
        Self {
            turn_id: turn_id.to_string(),
            executor_authorities: Vec::new(),
            include_host_skills: true,
            include_remote_skills: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillReadRequest {
    pub authority: SkillAuthority,
    pub resource: SkillResourceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillSearchRequest {
    pub authority: SkillAuthority,
    pub package: SkillPackageId,
    pub query: String,
}

/// Source-specific skill catalog and resource access.
///
/// Implementations must preserve authority boundaries: a resource listed by a
/// provider must be read or searched through the same provider/authority rather
/// than converted into an ambient local path.
pub trait SkillProvider: Send + Sync {
    fn list(
        &self,
        query: SkillListQuery,
    ) -> impl Future<Output = SkillProviderResult<SkillCatalog>> + Send;

    fn read(
        &self,
        request: SkillReadRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillReadResult>> + Send;

    fn search(
        &self,
        request: SkillSearchRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillSearchResult>> + Send;
}
