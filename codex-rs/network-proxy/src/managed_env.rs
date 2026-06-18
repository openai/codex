#[cfg(target_os = "macos")]
use crate::CODEX_PROXY_GIT_SSH_COMMAND_MARKER;
use crate::CUSTOM_CA_ENV_KEYS;
use crate::PROXY_ENV_KEYS;
#[cfg(target_os = "macos")]
use crate::PROXY_GIT_SSH_COMMAND_ENV_KEY;
use crate::is_managed_mitm_ca_trust_bundle_path;
use std::collections::HashMap;

/// Removes environment values owned by a managed network proxy before a command escapes its
/// sandbox and proxy containment.
pub fn strip_managed_proxy_env(env: &mut HashMap<String, String>) {
    for key in PROXY_ENV_KEYS {
        env.remove(*key);
    }
    for key in CUSTOM_CA_ENV_KEYS {
        if env
            .get(key)
            .is_some_and(|value| is_managed_mitm_ca_trust_bundle_path(value))
        {
            env.remove(key);
        }
    }
    // Only macOS injects a Codex-owned SSH wrapper for the managed SOCKS proxy.
    #[cfg(target_os = "macos")]
    if env
        .get(PROXY_GIT_SSH_COMMAND_ENV_KEY)
        .is_some_and(|command| command.starts_with(CODEX_PROXY_GIT_SSH_COMMAND_MARKER))
    {
        env.remove(PROXY_GIT_SSH_COMMAND_ENV_KEY);
    }
}

#[cfg(test)]
#[path = "managed_env_tests.rs"]
mod tests;
