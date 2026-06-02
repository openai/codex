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
pub(crate) struct RemoteSkillProvider;

impl SkillProvider for RemoteSkillProvider {
    fn list(
        &self,
        _query: SkillListQuery,
    ) -> impl Future<Output = SkillProviderResult<SkillCatalog>> + Send {
        future::ready(Ok(SkillCatalog::default()))
        // TODO(skills-extension): list org/account/backend skills from a
        // remote catalog. These skills should use opaque ids and backend
        // authority, not paths.
        //
        // TODO(skills-extension): decide how org policy and local enable/disable
        // rules combine when the backend supplies a managed skill catalog.
    }

    fn read(
        &self,
        request: SkillReadRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillReadResult>> + Send {
        future::ready(Err(crate::catalog::SkillProviderError {
            message: format!(
                "remote skill resource `{}` is not implemented",
                request.resource.0
            ),
        }))
        // TODO(skills-extension): read remote skill entrypoints and supporting
        // files through authenticated backend APIs.
    }

    fn search(
        &self,
        _request: SkillSearchRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillSearchResult>> + Send {
        future::ready(Ok(SkillSearchResult::default()))
        // TODO(skills-extension): expose model-facing skills/search or resource
        // APIs for large remote packages so the model can progressively
        // disclose supporting files without filesystem access.
    }
}
