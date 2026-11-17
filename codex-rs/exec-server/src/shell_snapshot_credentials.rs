//! Prepare one uncached login capture before creating any replay transport.
//! Startup profiles are trusted with real credentials. Raw capture bytes stay executor-local;
//! unsafe executable state falls back to normal startup.

use std::collections::BTreeSet;
use std::collections::HashMap;

use codex_network_proxy::CredentialMaskingFailureMode;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::RemoteCredentialBrokerCache;
use codex_network_proxy::brokered_credential_value_env_keys;
use codex_network_proxy::is_managed_proxy_env_var;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_protocol::shell_environment;
use codex_shell_command::shell_detect::ShellType;
use codex_shell_command::shell_snapshot::CapturedSnapshot;
use codex_shell_command::shell_snapshot::SNAPSHOT_BROKERED_UNSET_ENV_PREFIX;
use codex_shell_command::shell_snapshot::SNAPSHOT_BROKERED_VALUE_ENV_PREFIX;
use codex_shell_command::shell_snapshot::SnapshotCredentialEnvironment;
use codex_shell_command::shell_snapshot::build_brokered_credential_exports;
use codex_shell_command::shell_snapshot::prepare_snapshot_credentials;

use super::CaptureResult;
use super::MAX_FILE_SNAPSHOT_BYTES;
use super::MAX_SNAPSHOT_BYTES;
use super::SnapshotReplay;
use super::parse_snapshot;
use crate::local_process::shell_environment_policy;
use crate::protocol::ExecParams;
use crate::rpc::internal_error;

const CONFIGURED_VALUE_PREFIX: &str = "__CODEX_SHELL_SNAPSHOT_CONFIGURED_";

pub(super) fn restore_fallback(
    script: &mut String,
    env: &mut HashMap<String, String>,
    proxy: &NetworkProxy,
) {
    for value in env.values_mut() {
        proxy.restore_brokered_text(value);
    }
    proxy.restore_and_disable_brokered_credentials(env, &mut []);
    let warning = "Credential masking is unavailable for this shell snapshot; running with unmasked credentials.";
    tracing::warn!("{warning}");
    *script = format!("\\builtin printf '%s\\n' '{warning}' >&2\n{script}");
}

pub(super) async fn prepare(
    params: &ExecParams,
    output: &[u8],
    replay: SnapshotReplay,
    proxy: &NetworkProxy,
    credential_brokers: &RemoteCredentialBrokerCache,
) -> CaptureResult {
    let unsupported = || {
        (
            "credential_snapshot",
            internal_error("shell snapshot cannot safely replay credentials".to_string()),
        )
    };
    let captured = CapturedSnapshot::parse(ShellType::Zsh, output).ok_or_else(unsupported)?;
    let original = captured
        .environment
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (key, value) = std::str::from_utf8(entry).ok()?.split_once('=')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect::<Option<HashMap<_, _>>>()
        .ok_or_else(unsupported)?;
    let mut snapshot = parse_snapshot(ShellType::Zsh, output, params.env_policy.as_ref(), replay)?;
    let mut policy = params
        .env_policy
        .as_ref()
        .map(shell_environment_policy)
        .unwrap_or_default();
    policy.inherit = ShellEnvironmentPolicyInherit::All;
    let mut configured = shell_environment::create_env_from_vars(
        std::iter::empty(),
        &policy,
        /*thread_id*/ None,
    );
    configured.extend(params.env.clone());
    configured.retain(|key, _| {
        !shell_environment::is_non_inheritable_env_var(key)
            && ![
                crate::CODEX_EXEC_SERVER_EXIT_ON_STDIN_CLOSE_ENV_VAR,
                shell_environment::CODEX_THREAD_ID_ENV_VAR,
                shell_environment::CODEX_TOOL_CALL_ID_ENV_VAR,
            ]
            .contains(&key.as_str())
            && !key.starts_with(SNAPSHOT_BROKERED_VALUE_ENV_PREFIX)
            && !key.starts_with(SNAPSHOT_BROKERED_UNSET_ENV_PREFIX)
            && !key.starts_with(CONFIGURED_VALUE_PREFIX)
    });
    let mut restored = original.clone();
    for value in restored.values_mut() {
        proxy.restore_brokered_text(value);
    }
    let mut context = restored.clone();
    context.extend(configured.clone());
    proxy
        .bind_credential_broker_environment(credential_brokers, &context)
        .await
        .map_err(|_| {
            (
                "credential_proxy",
                internal_error("failed to bind startup credential destinations".to_string()),
            )
        })?;
    // Saved placeholders may belong to the final destination context selected above.
    for value in restored
        .values_mut()
        .chain(snapshot.environment.values_mut())
    {
        proxy.restore_brokered_text(value);
    }
    let mut discovered = restored.clone();
    let provider_context_keys = proxy
        .credential_broker_environment(&discovered)
        .provider_context_keys;
    for (key, value) in &configured {
        if !discovered.contains_key(key) || provider_context_keys.contains(key) {
            discovered.insert(key.clone(), value.clone());
        }
    }
    proxy.apply_to_env_for_snapshot(&mut discovered);
    let discovered_metadata = proxy.credential_broker_environment(&discovered);
    snapshot.environment.extend(configured.clone());
    snapshot.environment.retain(|key, _| {
        !key.starts_with(SNAPSHOT_BROKERED_VALUE_ENV_PREFIX)
            && !key.starts_with(SNAPSHOT_BROKERED_UNSET_ENV_PREFIX)
            && !key.starts_with(CONFIGURED_VALUE_PREFIX)
    });
    proxy.apply_to_env(&mut snapshot.environment);
    proxy
        .handle_unmasked_credentials(
            &mut snapshot.environment,
            CredentialMaskingFailureMode::Warn,
        )
        .map_err(|_| {
            (
                "credential_proxy",
                internal_error("failed to prepare startup credentials".to_string()),
            )
        })?;
    let allowed_metadata = proxy.credential_broker_environment(&snapshot.environment);
    let mut credential_keys = discovered_metadata.credential_keys.clone();
    credential_keys.extend(
        restored
            .iter()
            .filter(|(key, value)| {
                !value.is_empty()
                    && discovered_metadata.provider_keys.contains(key)
                    && !discovered_metadata.provider_context_keys.contains(key)
            })
            .map(|(key, _)| key.clone()),
    );
    let mut allowed_keys = allowed_metadata.credential_keys;
    allowed_keys.extend(
        credential_keys
            .iter()
            .filter(|key| {
                snapshot
                    .environment
                    .get(*key)
                    .is_some_and(|value| !value.is_empty())
            })
            .cloned(),
    );
    // Proxy state belongs to the launcher. Linux rewrites its endpoints inside each
    // sandbox network namespace after snapshot preparation.
    let managed_proxy_keys = snapshot
        .environment
        .iter()
        .filter(|(key, value)| is_managed_proxy_env_var(key, value))
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let is_managed_proxy_key =
        |key: &str| managed_proxy_keys.contains(key) || is_managed_proxy_env_var(key, "");
    let mut replay_environment = snapshot.environment.clone();
    replay_environment.retain(|key, _| !is_managed_proxy_key(key));
    let prepared = prepare_snapshot_credentials(
        &captured,
        SnapshotCredentialEnvironment {
            original: &original,
            restored: &restored,
            configured: &configured,
            discovered: &discovered,
            allowed: &replay_environment,
            is_allowed_unset: &|key| {
                !is_managed_proxy_key(key)
                    && (params.env_policy.is_none()
                        || shell_environment::create_env_from_vars(
                            [(key.to_string(), String::new())],
                            &policy,
                            /*thread_id*/ None,
                        )
                        .contains_key(key))
            },
            brokered_keys: &credential_keys,
            brokered_alias_keys: &[],
            allowed_brokered_keys: &allowed_keys,
        },
        |text| proxy.virtualize_brokered_text(text, &snapshot.environment),
    )
    .ok_or_else(unsupported)?;
    if !prepared.rejected_alias_keys.is_empty() {
        return Err(unsupported());
    }
    snapshot.environment.extend(prepared.aliases);
    let keys = brokered_credential_value_env_keys(&discovered)
        .into_iter()
        .chain(credential_keys)
        .chain(discovered_metadata.context_keys)
        .chain(discovered_metadata.provider_keys)
        .filter(|key| !is_managed_proxy_key(key))
        .collect::<BTreeSet<_>>();
    snapshot.environment.retain(|key, _| {
        !key.starts_with(SNAPSHOT_BROKERED_VALUE_ENV_PREFIX)
            && !key.starts_with(SNAPSHOT_BROKERED_UNSET_ENV_PREFIX)
    });
    let mut exports = String::new();
    for key in keys {
        if key.is_empty()
            || !key.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || index != 0 && byte.is_ascii_digit()
            })
        {
            return Err(unsupported());
        }
        if let Some(value) = snapshot.environment.get(&key).cloned() {
            let copy = format!("{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}{key}");
            snapshot.environment.insert(copy.clone(), value);
            // Login replay must also restore values cleared by the global zshenv.
            exports.push_str(&format!(
                "\\builtin export {key}=\"${{{copy}}}\" || exit 1\n"
            ));
        } else {
            snapshot.environment.insert(
                format!("{SNAPSHOT_BROKERED_UNSET_ENV_PREFIX}{key}"),
                "1".to_string(),
            );
            exports.push_str(&format!("\\builtin unset {key} || exit 1\n"));
        }
    }
    // Restore explicit overrides after captured declarations, including empty values.
    let mut overrides = String::new();
    for (index, key) in configured
        .keys()
        .filter(|key| !is_managed_proxy_key(key))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
    {
        if key.is_empty()
            || !key.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || index != 0 && byte.is_ascii_digit()
            })
        {
            continue;
        }
        if let Some(value) = snapshot.environment.get(key).cloned() {
            let copy = format!("{CONFIGURED_VALUE_PREFIX}{index}");
            snapshot.environment.insert(copy.clone(), value);
            overrides.push_str(&format!(
                "if [ -z \"${{{copy}+x}}\" ]; then exit 1; fi\n\\builtin export {key}=\"${{{copy}}}\" || exit 1\n\\builtin unset {copy} || exit 1\n"
            ));
        }
    }
    // Keep replay guards in the bounded snapshot transport, not the -c argument.
    let before =
        build_brokered_credential_exports(&snapshot.environment, /*remove_copies*/ false);
    let after =
        build_brokered_credential_exports(&snapshot.environment, /*remove_copies*/ true);
    snapshot.state = format!(
        "{before}\n{exports}{}\n{overrides}\n\\builtin test \"$?\" -eq 0 || exit 1\n{after}\n",
        prepared.script
    );
    let environment_bytes = snapshot
        .environment
        .iter()
        .map(|(key, value)| key.len() + value.len() + 2)
        .sum::<usize>();
    let state_limit = if snapshot.file_source {
        MAX_FILE_SNAPSHOT_BYTES
    } else {
        MAX_SNAPSHOT_BYTES.saturating_sub(environment_bytes)
    };
    if environment_bytes > MAX_SNAPSHOT_BYTES || snapshot.state.len() > state_limit {
        return Err(unsupported());
    }
    Ok(snapshot)
}
