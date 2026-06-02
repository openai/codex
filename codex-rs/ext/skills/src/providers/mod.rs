mod executor;
mod host;
mod remote;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillProviderResult;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;

use executor::ExecutorSkillProvider;
use host::HostSkillProvider;
use remote::RemoteSkillProvider;

#[derive(Clone, Debug, Default)]
pub(crate) struct SkillProviders {
    host: HostSkillProvider,
    executor: ExecutorSkillProvider,
    remote: RemoteSkillProvider,
}

impl SkillProviders {
    pub(crate) async fn list_for_turn(
        &self,
        query: SkillListQuery,
    ) -> SkillProviderResult<SkillCatalog> {
        let mut catalog = SkillCatalog::default();

        if query.include_host_skills {
            catalog.extend(self.host.list(query.clone()).await?);
        }

        if !query.executor_authorities.is_empty() {
            catalog.extend(self.executor.list(query.clone()).await?);
        }

        if query.include_remote_skills {
            catalog.extend(self.remote.list(query).await?);
        }

        // TODO(skills-extension): apply final merged-catalog policy here:
        // source precedence, duplicate name handling, disabled-skill rules,
        // product/session-source filtering, and telemetry for omitted entries.
        Ok(catalog)
    }
}
