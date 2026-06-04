use std::future;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSearchResult;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;
use crate::provider::SkillReadRequest;
use crate::provider::SkillSearchRequest;

#[derive(Clone, Debug, Default)]
pub(crate) struct ExecutorSkillProvider;

impl SkillProvider for ExecutorSkillProvider {
    fn list(
        &self,
        _query: SkillListQuery,
    ) -> impl Future<Output = SkillProviderResult<SkillCatalog>> + Send {
        future::ready(Ok(SkillCatalog::default()))
        // TODO(skills-extension): list repo/workspace skills from each
        // executor authority selected for the turn.
        //
        // TODO(skills-extension): if the executor exposes filesystem reads,
        // preserve the existing SKILL.md discovery semantics. If CCA/no-FS
        // applies, query an executor catalog/read capability instead.
        //
        // TODO(skills-extension): include the executor/environment id in skill
        // identity so two executors with the same path/name do not collide.
    }

    fn read(
        &self,
        request: SkillReadRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillReadResult>> + Send {
        future::ready(Err(crate::catalog::SkillProviderError {
            message: format!(
                "executor skill resource `{}` is not implemented",
                request.resource.0
            ),
        }))
        // TODO(skills-extension): route reads back to the executor authority
        // that listed the resource. Do not mint local paths from remote or
        // non-filesystem executor resources.
    }

    fn search(
        &self,
        _request: SkillSearchRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillSearchResult>> + Send {
        future::ready(Ok(SkillSearchResult::default()))
        // TODO(skills-extension): support search for executor skills only when
        // the executor offers a catalog/search API. For ordinary filesystem
        // executors, the model can keep using regular file reads/search tools.
    }
}
