//! Installs application requirements at startup and explicit config/account reloads.
//! Local policy limits cloud bootstrap; config construction rejects superseded policy snapshots.

use super::ApplicationPolicyLoad;
use super::ApplicationPolicySnapshot;
use super::ConfigManager;
use codex_config::ApplicationRequirementsToml;
use codex_config::CloudConfigBundleLoader;
use codex_config::NetworkDomainPermissionToml;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyDenied;
use std::io;
use std::sync::Arc;

pub(crate) fn destination_policy(
    application: Option<&ApplicationRequirementsToml>,
) -> DestinationPolicy {
    match application.and_then(|application| application.network.as_ref()) {
        Some(network) if network.enabled => DestinationPolicy::Restricted {
            allowed_hosts: network
                .domains
                .iter()
                .filter(|&(_host, permission)| *permission == NetworkDomainPermissionToml::Allow)
                .map(|(host, _permission)| host.clone())
                .collect(),
        },
        Some(_) | None => DestinationPolicy::Unrestricted,
    }
}

impl ConfigManager {
    pub(super) async fn refresh_local_network_policy(
        &self,
    ) -> io::Result<codex_config::loader::LocalApplicationRequirements> {
        let _reload = self
            .network_policy_reload
            .acquire()
            .await
            .map_err(io::Error::other)?;
        self.load_local_network_policy(self.network_policy.policy().revision())
            .await
    }

    async fn load_local_network_policy(
        &self,
        application_revision: codex_http_client::NetworkPolicyRevision,
    ) -> io::Result<codex_config::loader::LocalApplicationRequirements> {
        let local_revision = self.local_network_policy.policy().revision();
        let result = async {
            let sources = codex_config::loader::load_local_application_requirements(
                codex_exec_server::LOCAL_FS.as_ref(),
                &self.loader_overrides,
            )
            .await?;
            let application = sources.compose(Default::default())?;
            self.local_network_policy
                .publish(local_revision, destination_policy(application.as_ref()));
            Ok(sources)
        }
        .await;
        if result.is_err() {
            self.local_network_policy.unavailable(local_revision);
            self.fail_application_policy(application_revision)?;
        }
        result
    }

    pub(crate) async fn refresh_application_network_policy(
        &self,
    ) -> io::Result<ApplicationPolicyLoad> {
        let _reload = self
            .network_policy_reload
            .acquire()
            .await
            .map_err(io::Error::other)?;
        let revision = self.network_policy.policy().revision();
        let local_sources = self.load_local_network_policy(revision).await?;
        let cloud_config = self.current_cloud_config_bundle();
        let result = async {
            let cloud = cloud_config.get().await.map_err(io::Error::other)?;
            let application = local_sources.compose(
                cloud
                    .as_ref()
                    .map(|bundle| bundle.requirements_toml.clone())
                    .unwrap_or_default(),
            )?;
            Ok::<_, io::Error>((destination_policy(application.as_ref()), cloud))
        }
        .await;
        match result {
            Ok((policy, cloud)) => {
                let mut current = self.network_policy_snapshot.write().map_err(|_| {
                    io::Error::other("application network policy snapshot lock poisoned")
                })?;
                if !self.network_policy.publish(revision, policy.clone()) {
                    *current = None;
                    return Err(io::Error::other(NetworkPolicyDenied::Revoked));
                }
                let next = ApplicationPolicySnapshot {
                    revision,
                    policy,
                    cloud: cloud.clone(),
                };
                // Concurrent config loads may finish together if their actual inputs are unchanged.
                let snapshot = match current.as_ref() {
                    Some(snapshot) if snapshot.as_ref() == &next => Arc::clone(snapshot),
                    _ => {
                        let snapshot = Arc::new(next);
                        *current = Some(Arc::clone(&snapshot));
                        snapshot
                    }
                };
                Ok(ApplicationPolicyLoad {
                    cloud_config: CloudConfigBundleLoader::new(async move { Ok(cloud) }),
                    snapshot,
                })
            }
            Err(error) => {
                self.fail_application_policy(revision)?;
                Err(error)
            }
        }
    }

    pub(super) fn check_application_policy_load(
        &self,
        load: &ApplicationPolicyLoad,
    ) -> io::Result<()> {
        let current = self
            .network_policy_snapshot
            .read()
            .map_err(|_| io::Error::other("application network policy snapshot lock poisoned"))?;
        if self.network_policy.policy().revision() != load.snapshot.revision
            || !current
                .as_ref()
                .is_some_and(|snapshot| Arc::ptr_eq(snapshot, &load.snapshot))
        {
            return Err(io::Error::other(NetworkPolicyDenied::Revoked));
        }
        Ok(())
    }

    fn fail_application_policy(
        &self,
        revision: codex_http_client::NetworkPolicyRevision,
    ) -> io::Result<()> {
        let mut current = self
            .network_policy_snapshot
            .write()
            .map_err(|_| io::Error::other("application network policy snapshot lock poisoned"))?;
        self.network_policy.unavailable(revision);
        *current = None;
        Ok(())
    }
}
