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
        // TODO(skills-extension): list bundled/system/user/plugin-installed
        // skills owned by the Codex host. This is the source for skills that
        // are not tied to a particular executor authority.
        //
        // TODO(skills-extension): plugins should be treated as packaging and
        // installation only. After a plugin is downloaded, cached, refreshed,
        // or installed, its skill roots/descriptors should enter this provider
        // and then use the normal skills catalog/read/injection code.
        //
        // TODO(skills-extension): remote skills that are materialized locally
        // by plugin install or explicit download should also hand off here
        // rather than remain remote-provider entries at runtime.
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
        //
        // TODO(skills-extension): for plugin-installed skills, route reads
        // through the materialized plugin cache/root that produced the catalog
        // entry, while keeping the public id opaque and authority-bound.
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
