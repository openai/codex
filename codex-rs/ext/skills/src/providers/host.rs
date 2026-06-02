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
pub(crate) struct HostSkillProvider;

impl SkillProvider for HostSkillProvider {
    fn list(
        &self,
        _query: SkillListQuery,
    ) -> impl Future<Output = SkillProviderResult<SkillCatalog>> + Send {
        future::ready(Ok(SkillCatalog::default()))
        // TODO(skills-extension): list bundled/system/user/plugin-cache skills
        // owned by the Codex host. This is the source for skills that are not
        // tied to a particular executor authority.
        //
        // TODO(skills-extension): keep current bundled system skill install or
        // replace it with embedded host assets so CCA/no-FS hosts do not depend
        // on local writable skill cache directories.
    }

    fn read(
        &self,
        request: SkillReadRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillReadResult>> + Send {
        future::ready(Err(crate::catalog::SkillProviderError {
            message: format!(
                "host skill resource `{}` is not implemented",
                request.resource.0
            ),
        }))
        // TODO(skills-extension): read host-owned entrypoints and supporting
        // resources by opaque id, not by assuming a local filesystem path.
    }

    fn search(
        &self,
        _request: SkillSearchRequest,
    ) -> impl Future<Output = SkillProviderResult<SkillSearchResult>> + Send {
        future::ready(Ok(SkillSearchResult::default()))
        // TODO(skills-extension): decide whether host skills need search, or
        // whether direct read by opaque resource id is enough.
    }
}
