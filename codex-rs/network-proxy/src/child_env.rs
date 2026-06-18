use super::NetworkProxyRuntimeSettings;
use super::STARTUP_CA_ENV_KEYS_PRESENT_ENV_KEY;
use super::apply_proxy_env_overrides;
use super::ca_env_keys;
use super::is_tracked_startup_ca_env_key;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use tracing::warn;

/// Immutable proxy settings used to prepare one child process environment.
///
/// Keeping the managed CA path and environment rewrite on the same snapshot
/// prevents a live proxy configuration reload from changing the MITM state in
/// the middle of sandbox policy construction.
#[derive(Clone)]
pub struct NetworkProxyChildEnvSnapshot {
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
    socks_enabled: bool,
    runtime_settings: NetworkProxyRuntimeSettings,
}

impl NetworkProxyChildEnvSnapshot {
    pub(super) fn new(
        http_addr: SocketAddr,
        socks_addr: SocketAddr,
        socks_enabled: bool,
        runtime_settings: NetworkProxyRuntimeSettings,
    ) -> Self {
        Self {
            http_addr,
            socks_addr,
            socks_enabled,
            runtime_settings,
        }
    }

    pub fn has_managed_mitm_ca(&self) -> bool {
        self.runtime_settings.mitm_ca_trust_bundle.is_some()
    }

    /// Returns the generated MITM CA bundle path this snapshot will expose.
    pub fn managed_mitm_ca_trust_bundle_path(&self) -> Option<AbsolutePathBuf> {
        self.runtime_settings
            .mitm_ca_trust_bundle
            .as_ref()
            .and_then(|bundle| {
                AbsolutePathBuf::from_absolute_path(&bundle.path)
                    .map_err(|err| warn!("managed MITM CA trust bundle path is invalid: {err}"))
                    .ok()
            })
    }

    pub fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        apply_proxy_env_overrides(
            env,
            self.http_addr,
            self.socks_addr,
            self.socks_enabled,
            self.runtime_settings.allow_local_binding,
            self.runtime_settings.mitm_ca_trust_bundle.as_ref(),
        );
    }

    /// Prepares a child environment without creating a command-specific CA bundle.
    ///
    /// Persistent sandbox identities cannot safely receive a read grant for a
    /// derived bundle because later commands would retain that grant. Preserve
    /// the pre-materialization behavior there and expose only the stable
    /// managed baseline path.
    pub fn prepare_persistent_sandbox_child_env(
        &self,
        env: &mut HashMap<String, String>,
    ) -> Vec<AbsolutePathBuf> {
        self.apply_to_env(env);
        env.remove(STARTUP_CA_ENV_KEYS_PRESENT_ENV_KEY);
        self.managed_mitm_ca_trust_bundle_path()
            .into_iter()
            .collect()
    }

    /// Returns whether this child would need a command-specific managed CA bundle.
    ///
    /// Persistent sandbox identities must reject this shape rather than grant
    /// access to a derived bundle that later commands could continue reading.
    pub fn requires_child_specific_mitm_ca_bundle(&self, env: &HashMap<String, String>) -> bool {
        let Some(mitm_ca_trust_bundle) = self.runtime_settings.mitm_ca_trust_bundle.as_ref() else {
            return false;
        };
        if env
            .get(crate::certs::SSL_CERT_DIR_ENV_KEY)
            .is_some_and(|value| !value.is_empty())
        {
            // The stable Windows baseline only embeds file-backed startup CA
            // overrides. Directory contents can change after proxy startup,
            // so they still require per-child materialization and cannot be
            // exposed through a persistent sandbox identity.
            return true;
        }

        let managed_path = mitm_ca_trust_bundle.path.to_string_lossy();
        crate::certs::CUSTOM_CA_ENV_KEYS.into_iter().any(|key| {
            env.get(key)
                .filter(|value| !value.is_empty())
                .is_some_and(|value| {
                    value != managed_path.as_ref()
                        && mitm_ca_trust_bundle.startup_env_values.get(key) != Some(value)
                })
        })
    }

    /// Rewrites readable child-selected CA bundles into immutable managed MITM bundles.
    pub fn prepare_child_env<F>(
        &self,
        env: &mut HashMap<String, String>,
        cwd: &Path,
        can_read_path: F,
    ) -> Vec<AbsolutePathBuf>
    where
        F: Fn(&Path) -> bool,
    {
        self.apply_to_env(env);
        let startup_ca_env_keys_present_in_child = ca_env_keys()
            .filter(|&key| is_tracked_startup_ca_env_key(env, key))
            .collect::<Vec<_>>();
        env.remove(STARTUP_CA_ENV_KEYS_PRESENT_ENV_KEY);
        self.runtime_settings
            .mitm_ca_trust_bundle
            .as_ref()
            .map_or_else(Vec::new, |mitm_ca_trust_bundle| {
                crate::child_ca::prepare_mitm_ca_trust_bundle_env(
                    mitm_ca_trust_bundle,
                    env,
                    cwd,
                    &startup_ca_env_keys_present_in_child,
                    can_read_path,
                )
            })
    }
}
