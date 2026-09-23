use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use crate::StateDbHandle;
use crate::rollout::list::find_thread_path_by_id_str;
use crate::shell::Shell;
use crate::shell::ShellType;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_exec_server::Environment;
use codex_network_proxy::CREDENTIAL_BROKER_ACTIVE_ENV_KEY;
use codex_network_proxy::CredentialBrokerContext;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::brokered_credential_marker_env_keys;
use codex_network_proxy::brokered_credential_value_env_keys;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::shell_environment::create_env_from_vars;
use codex_shell_command::shell_snapshot::CapturedSnapshot;
use codex_shell_command::shell_snapshot::PreparedSnapshot;
use codex_shell_command::shell_snapshot::SnapshotCaptureOptions;
use codex_shell_command::shell_snapshot::SnapshotCredentialEnvironment;
use codex_shell_command::shell_snapshot::SnapshotStartup;
use codex_shell_command::shell_snapshot::prepare_snapshot_credentials;
use codex_shell_command::shell_snapshot::snapshot_capture_script;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::info_span;

#[path = "shell_snapshot_sandbox.rs"]
mod sandbox;

pub(crate) use sandbox::ShellSnapshotSandbox;
pub(crate) use sandbox::snapshot_read_permissions;

#[derive(Clone)]
pub(crate) struct ShellSnapshot {
    config: Option<Arc<ShellSnapshotConfig>>,
    credential_broker: Option<watch::Sender<SnapshotCredentialBrokerState>>,
}

struct ShellSnapshotConfig {
    codex_home: AbsolutePathBuf,
    session_id: ThreadId,
    session_telemetry: SessionTelemetry,
    state_db: Option<StateDbHandle>,
    credential_broker: Option<watch::Receiver<SnapshotCredentialBrokerState>>,
    prefer_executor_snapshots: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) enum SnapshotCredentialBrokerState {
    Starting,
    Inactive,
    Unavailable,
    Ready(NetworkProxy),
}

pub(crate) struct ShellSnapshotFile {
    path: AbsolutePathBuf,
    credentials: Option<SnapshotCredentials>,
}

struct SnapshotCredentials {
    network_proxy: NetworkProxy,
    broker_config_revision: u64,
    shell_environment_policy: ShellEnvironmentPolicy,
    protected_startup_env: Option<String>,
    credential_env: HashMap<String, String>,
    unset_credential_keys: Vec<String>,
    // Retain discovered key identity even when policy replaces or removes its value.
    startup_credential_keys: Vec<String>,
    context_env: HashMap<String, String>,
    fail_open_aliases: HashMap<String, FailOpenAlias>,
    context_credential_keys: HashMap<String, Vec<String>>,
    binding_context_credential_keys: HashMap<String, Vec<String>>,
}

struct SnapshotCredentialBroker {
    network_proxy: NetworkProxy,
    shell_environment_policy: ShellEnvironmentPolicy,
    allow_login_shell: bool,
}

struct FailOpenAlias {
    value: String,
    dummy_value: Option<String>,
    authorized: bool,
}

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const SNAPSHOT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 3); // 3 days retention.
const SNAPSHOT_DIR: &str = "shell_snapshots";

impl ShellSnapshot {
    pub(crate) fn new(
        codex_home: AbsolutePathBuf,
        session_id: ThreadId,
        session_telemetry: SessionTelemetry,
        state_db: Option<StateDbHandle>,
        credential_broker: Option<watch::Sender<SnapshotCredentialBrokerState>>,
        prefer_executor_snapshots: bool,
    ) -> Self {
        Self {
            config: Some(Arc::new(ShellSnapshotConfig {
                codex_home,
                session_id,
                session_telemetry,
                state_db,
                credential_broker: credential_broker.as_ref().map(watch::Sender::subscribe),
                prefer_executor_snapshots,
            })),
            credential_broker,
        }
    }

    pub(crate) fn disabled() -> Self {
        Self {
            config: None,
            credential_broker: None,
        }
    }

    pub(crate) fn set_credential_broker(&self, state: SnapshotCredentialBrokerState) -> bool {
        self.credential_broker.as_ref().is_some_and(|sender| {
            let previous = sender.send_replace(state.clone());
            previous != state
        })
    }

    pub(crate) fn should_rebuild_inherited(&self) -> bool {
        self.credential_broker.as_ref().is_some_and(|sender| {
            !matches!(*sender.borrow(), SnapshotCredentialBrokerState::Inactive)
        })
    }

    pub(crate) async fn build(
        self,
        environment: Arc<Environment>,
        cwd: PathUri,
        shell: Option<Shell>,
        allow_login_shell: bool,
        shell_environment_policy: ShellEnvironmentPolicy,
        sandbox: Option<ShellSnapshotSandbox>,
    ) -> Option<Arc<ShellSnapshotFile>> {
        let config = Arc::clone(self.config.as_ref()?);
        if environment.is_remote() {
            return None;
        }

        let shell = shell?;
        // TODO(anp): Migrate shell snapshot creation to accept PathUri and defer native
        // conversion to the spawned shell process.
        let cwd = cwd.to_abs_path().ok()?;
        drop(self);
        Self::build_for_cwd(
            config,
            cwd,
            shell,
            allow_login_shell,
            shell_environment_policy,
            sandbox,
        )
        .await
    }

    async fn build_for_cwd(
        config: Arc<ShellSnapshotConfig>,
        cwd: AbsolutePathBuf,
        shell: Shell,
        allow_login_shell: bool,
        shell_environment_policy: ShellEnvironmentPolicy,
        sandbox: Option<ShellSnapshotSandbox>,
    ) -> Option<Arc<ShellSnapshotFile>> {
        let snapshot_span = info_span!("shell_snapshot", thread_id = %config.session_id);
        async {
            let credential_broker = if let Some(receiver) = config.credential_broker.as_ref() {
                let mut receiver = receiver.clone();
                if matches!(&*receiver.borrow(), SnapshotCredentialBrokerState::Starting)
                    && receiver.changed().await.is_err()
                {
                    return None;
                }
                let state = receiver.borrow().clone();
                match state {
                    SnapshotCredentialBrokerState::Starting
                    | SnapshotCredentialBrokerState::Unavailable => return None,
                    SnapshotCredentialBrokerState::Inactive if config.prefer_executor_snapshots => {
                        return None;
                    }
                    SnapshotCredentialBrokerState::Inactive => None,
                    SnapshotCredentialBrokerState::Ready(network_proxy) if sandbox.is_some() => {
                        Some(SnapshotCredentialBroker {
                            network_proxy,
                            shell_environment_policy,
                            allow_login_shell,
                        })
                    }
                    SnapshotCredentialBrokerState::Ready(_) => return None,
                }
            } else {
                None
            };
            let timer = config
                .session_telemetry
                .start_timer("codex.shell_snapshot.duration_ms", &[("version", "v1")]);
            let snapshot = ShellSnapshot::try_create(
                &config.codex_home,
                config.session_id,
                &cwd,
                &shell,
                config.state_db.clone(),
                credential_broker,
                sandbox.as_ref(),
            )
            .await;
            let success_tag = if snapshot.is_ok() { "true" } else { "false" };
            let _ = timer.map(|timer| timer.record(&[("success", success_tag)]));
            let mut counter_tags = vec![("version", "v1"), ("success", success_tag)];
            if let Some(failure_reason) = snapshot.as_ref().err() {
                counter_tags.push(("failure_reason", *failure_reason));
            }
            config
                .session_telemetry
                .counter("codex.shell_snapshot", /*inc*/ 1, &counter_tags);
            snapshot.ok().map(Arc::new)
        }
        .instrument(snapshot_span)
        .await
    }

    async fn try_create(
        codex_home: &AbsolutePathBuf,
        session_id: ThreadId,
        session_cwd: &AbsolutePathBuf,
        shell: &Shell,
        state_db: Option<StateDbHandle>,
        credential_broker: Option<SnapshotCredentialBroker>,
        sandbox: Option<&ShellSnapshotSandbox>,
    ) -> std::result::Result<ShellSnapshotFile, &'static str> {
        // File to store the snapshot
        let extension = match shell.shell_type {
            ShellType::PowerShell => "ps1",
            _ => "sh",
        };
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.{nonce}.{extension}"));
        let temp_path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.tmp-{nonce}"));

        // Clean the (unlikely) leaked snapshot files.
        let codex_home = codex_home.clone();
        let cleanup_session_id = session_id;
        tokio::spawn(async move {
            if let Err(err) =
                cleanup_stale_snapshots(&codex_home, cleanup_session_id, state_db).await
            {
                tracing::warn!("Failed to clean up shell snapshots: {err:?}");
            }
        });

        // Make the new snapshot.
        let credentials = write_shell_snapshot(
            shell,
            &temp_path,
            session_cwd,
            credential_broker.as_ref(),
            sandbox,
        )
        .await
        .map_err(|err| {
            tracing::warn!(
                "Failed to create shell snapshot for {}: {err:?}",
                shell.name()
            );
            "write_failed"
        })?;
        tracing::info!(
            "Shell snapshot successfully created: {}",
            temp_path.display()
        );

        if let Err(err) = validate_snapshot(
            shell,
            &temp_path,
            session_cwd,
            credential_broker.as_ref(),
            sandbox,
        )
        .await
        {
            tracing::error!("Shell snapshot validation failed: {err:?}");
            remove_snapshot_file(&temp_path).await;
            return Err("validation_failed");
        }

        if let Err(err) = fs::rename(&temp_path, &path).await {
            tracing::warn!("Failed to finalize shell snapshot: {err:?}");
            remove_snapshot_file(&temp_path).await;
            return Err("write_failed");
        }

        Ok(ShellSnapshotFile { path, credentials })
    }
}

impl ShellSnapshotFile {
    pub(crate) fn is_brokered_for(
        &self,
        shell_environment_policy: &ShellEnvironmentPolicy,
    ) -> bool {
        self.credentials.as_ref().is_some_and(|credentials| {
            &credentials.shell_environment_policy == shell_environment_policy
                && credentials.broker_config_revision
                    == credentials
                        .network_proxy
                        .credential_broker_config_revision()
        })
    }

    pub(crate) fn path(&self) -> AbsolutePathBuf {
        self.path.clone()
    }

    pub(crate) fn protected_startup_env(&self) -> Option<&str> {
        self.credentials
            .as_ref()
            .and_then(|credentials| credentials.protected_startup_env.as_deref())
    }

    pub(crate) fn startup_credential_keys(&self) -> &[String] {
        self.credentials.as_ref().map_or(&[], |credentials| {
            credentials.startup_credential_keys.as_slice()
        })
    }

    #[must_use = "pass private context to the broker when preparing a managed child"]
    pub(crate) fn restore_credentials(
        &self,
        env: &mut HashMap<String, String>,
        shell_environment_policy: &ShellEnvironmentPolicy,
    ) -> CredentialBrokerContext {
        let Some(credentials) = self.credentials.as_ref() else {
            return CredentialBrokerContext::default();
        };
        let mut credential_context = HashMap::new();

        let mut allowed_snapshot_env = create_env_from_vars(
            credentials
                .credential_env
                .iter()
                .chain(&credentials.context_env)
                .map(|(key, value)| (key.clone(), value.clone())),
            shell_environment_policy,
            /*thread_id*/ None,
        );
        let explicit_env_overrides = &shell_environment_policy.r#set;
        for key in &credentials.unset_credential_keys {
            if !contains_env_key(explicit_env_overrides, key) {
                remove_env_value(env, key);
                remove_env_value(&mut allowed_snapshot_env, key);
            }
        }
        let mut snapshot_real_credentials = credentials.credential_env.clone();
        snapshot_real_credentials.insert(
            CREDENTIAL_BROKER_ACTIVE_ENV_KEY.to_string(),
            "1".to_string(),
        );
        credentials
            .network_proxy
            .restore_brokered_credentials(&mut snapshot_real_credentials, &mut []);
        let mut restored_credential_keys = Vec::new();
        for (key, value) in &credentials.credential_env {
            if contains_env_key(explicit_env_overrides, key)
                || !contains_env_key(&allowed_snapshot_env, key)
            {
                continue;
            }
            if snapshot_real_credentials.get(key) != env_value(env, key) {
                insert_env_value(env, key, value);
                restored_credential_keys.push(key);
            }
        }
        for (key, value) in &credentials.context_env {
            let Some(credential_keys) = credentials.context_credential_keys.get(key) else {
                continue;
            };
            let binding_keys = credentials.binding_context_credential_keys.get(key);
            let restores_binding_context = binding_keys.is_some_and(|keys| {
                keys.iter()
                    .any(|key| restored_credential_keys.contains(&key))
            });
            let needs_binding_context = restores_binding_context
                || binding_keys.is_some_and(|keys| {
                    keys.iter().any(|key| {
                        env_value(env, key)
                            .is_some_and(|value| snapshot_real_credentials.get(key) == Some(value))
                    })
                });
            let allowed_context = contains_env_key(&allowed_snapshot_env, key);
            let explicit_context = env_value(explicit_env_overrides, key);
            if explicit_context.is_some() && allowed_context
                || !allowed_context && !needs_binding_context
                || !credential_keys
                    .iter()
                    .any(|credential_key| contains_env_key(&allowed_snapshot_env, credential_key))
            {
                continue;
            }
            if restores_binding_context || !contains_env_key(env, key) {
                let target = if allowed_context {
                    &mut *env
                } else {
                    &mut credential_context
                };
                insert_env_value(target, key, explicit_context.unwrap_or(value));
            }
        }

        for env in [env, &mut credential_context] {
            let previous = env.insert(
                CREDENTIAL_BROKER_ACTIVE_ENV_KEY.to_string(),
                "1".to_string(),
            );
            credentials
                .network_proxy
                .restore_brokered_credentials(env, &mut []);
            if let Some(previous) = previous {
                env.insert(CREDENTIAL_BROKER_ACTIVE_ENV_KEY.to_string(), previous);
            } else {
                env.remove(CREDENTIAL_BROKER_ACTIVE_ENV_KEY);
            }
        }
        credential_context.into()
    }

    pub(crate) fn restore_fail_open_aliases(
        &self,
        env: &mut HashMap<String, String>,
        environment_id: Option<&str>,
    ) {
        let Some(credentials) = self.credentials.as_ref() else {
            return;
        };
        if env_value(env, CREDENTIAL_BROKER_ACTIVE_ENV_KEY).is_some_and(|active| active == "1") {
            for (key, alias) in &credentials.fail_open_aliases {
                let has_snapshot_value = alias.dummy_value.as_ref().is_some_and(|dummy| {
                    env_value(env, key).is_some_and(|value| {
                        value == dummy
                            || credentials.network_proxy.child_credential_alias_matches(
                                key,
                                value,
                                dummy,
                                environment_id,
                            )
                    })
                });
                if has_snapshot_value {
                    if !alias.authorized {
                        remove_env_value(env, key);
                    }
                    continue;
                }
                if env_value(env, key).is_some_and(|value| {
                    let mut virtualized = value.clone();
                    !credentials
                        .network_proxy
                        .virtualize_brokered_text(&mut virtualized, env)
                }) {
                    remove_env_value(env, key);
                }
            }
            return;
        }

        let policy = &credentials.shell_environment_policy;
        for (key, alias) in &credentials.fail_open_aliases {
            if contains_env_key(&policy.r#set, key)
                || credentials.unset_credential_keys.contains(key)
            {
                continue;
            }
            let has_snapshot_dummy = alias
                .dummy_value
                .as_ref()
                .is_some_and(|dummy| env_value(env, key) == Some(dummy));
            if has_snapshot_dummy {
                if alias.authorized {
                    insert_env_value(env, key, &alias.value);
                } else {
                    remove_env_value(env, key);
                }
            } else if alias.authorized && !contains_env_key(env, key) {
                insert_env_value(env, key, &alias.value);
            }
        }
    }
}

impl Drop for ShellSnapshotFile {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::warn!(
                "Failed to delete shell snapshot at {:?}: {err:?}",
                self.path
            );
        }
    }
}

async fn write_shell_snapshot(
    shell: &Shell,
    output_path: &AbsolutePathBuf,
    cwd: &AbsolutePathBuf,
    credential_broker: Option<&SnapshotCredentialBroker>,
    sandbox: Option<&ShellSnapshotSandbox>,
) -> Result<Option<SnapshotCredentials>> {
    let shell_type = shell.shell_type;
    if shell_type == ShellType::PowerShell || shell_type == ShellType::Cmd {
        bail!("Shell snapshot not supported yet for {shell_type:?}");
    }
    let (snapshot, credentials) = capture_snapshot(shell, cwd, credential_broker, sandbox).await?;

    if let Some(parent) = output_path.parent() {
        let parent_display = parent.display();
        fs::create_dir_all(&parent)
            .await
            .with_context(|| format!("Failed to create snapshot parent {parent_display}"))?;
    }

    let snapshot_path = output_path.display();
    fs::write(output_path, snapshot)
        .await
        .with_context(|| format!("Failed to write snapshot to {snapshot_path}"))?;

    Ok(credentials)
}

async fn capture_snapshot(
    shell: &Shell,
    cwd: &AbsolutePathBuf,
    credential_broker: Option<&SnapshotCredentialBroker>,
    sandbox: Option<&ShellSnapshotSandbox>,
) -> Result<(String, Option<SnapshotCredentials>)> {
    let broker_config_revision = credential_broker
        .map(|broker| broker.network_proxy.credential_broker_config_revision())
        .unwrap_or_default();
    let shell_type = shell.shell_type;
    let shell_startup = if credential_broker.is_some_and(|broker| !broker.allow_login_shell) {
        SnapshotStartup::NonInteractive
    } else {
        SnapshotStartup::Interactive
    };
    let script = snapshot_capture_script(
        shell_type,
        SnapshotCaptureOptions {
            startup: shell_startup,
            declarations: true,
            environment: credential_broker.is_some(),
        },
    )
    .ok_or_else(|| anyhow!("Shell snapshotting is not yet supported for {shell_type:?}"))?;
    let shell_mode = if credential_broker.is_none_or(|broker| broker.allow_login_shell) {
        SnapshotShellMode::Login
    } else {
        SnapshotShellMode::NonLogin
    };
    let raw_snapshot = run_script_with_timeout(
        shell,
        &script,
        SNAPSHOT_TIMEOUT,
        shell_mode,
        cwd,
        credential_broker,
        sandbox,
    )
    .await?;
    let capture = CapturedSnapshot::parse(shell_type, raw_snapshot.as_bytes())
        .ok_or_else(|| anyhow!("invalid shell snapshot capture"))?;
    let Some(credential_broker) = credential_broker else {
        return Ok((capture.render_script(), None));
    };

    let original_env = std::str::from_utf8(capture.environment)?
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<HashMap<_, _>>();
    // The inherited broker marker survives startup even when its credential variables do not.
    let mut unset_credential_keys = brokered_credential_marker_env_keys(&original_env)
        .into_iter()
        .filter(|key| !contains_env_key(&original_env, key))
        .collect::<Vec<_>>();
    let policy = &credential_broker.shell_environment_policy;
    // Child visibility must not change the trusted destination of an inherited credential.
    let inherited_env = std::env::vars().collect();
    let network_config = credential_broker.network_proxy.current_cfg().await?;
    let inherited_env = network_config
        .credential_broker_context
        .with_fallbacks(&inherited_env);
    let mut restored_env = original_env.clone();
    credential_broker
        .network_proxy
        .restore_brokered_credentials(&mut restored_env, &mut []);
    let mut discovery_env = restored_env.clone();
    let provider_environment = credential_broker
        .network_proxy
        .credential_broker_environment(&discovery_env);
    let provider_context_keys = provider_environment.provider_context_keys;
    replace_provider_context_with_trusted(
        &mut discovery_env,
        &inherited_env,
        provider_context_keys.clone(),
        &provider_context_keys,
    );
    for (key, value) in &policy.r#set {
        if !contains_env_key(&discovery_env, key)
            || provider_context_keys
                .iter()
                .any(|context_key| context_key.eq_ignore_ascii_case(key))
        {
            insert_env_value(&mut discovery_env, key, value);
        }
    }
    credential_broker
        .network_proxy
        .apply_to_env_for_snapshot(&mut discovery_env);
    let discovery_metadata = credential_broker
        .network_proxy
        .credential_broker_environment(&discovery_env);
    let brokered_keys = discovery_metadata.credential_keys;
    let mut env = create_env_from_vars(
        restored_env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
        policy,
        /*thread_id*/ None,
    );
    let allowed_context_env = create_env_from_vars(
        provider_context_keys
            .iter()
            .map(|key| (key.clone(), String::new())),
        policy,
        /*thread_id*/ None,
    );
    replace_provider_context_with_trusted(
        &mut env,
        &discovery_env,
        provider_context_keys.clone(),
        &[],
    );
    credential_broker
        .network_proxy
        .apply_to_env_for_snapshot(&mut env);
    let mut snapshot_allowed_env = env.clone();
    // The live broker regenerates this marker; captured dummies can belong to another scope.
    snapshot_allowed_env.remove("CODEX_NETWORK_PROXY_BROKERED_CREDENTIALS");
    for key in &provider_context_keys {
        if env_value(&original_env, key) != env_value(&env, key) {
            remove_env_value(&mut snapshot_allowed_env, key);
        }
    }
    for key in &provider_context_keys {
        if !contains_env_key(&allowed_context_env, key) {
            remove_env_value(&mut snapshot_allowed_env, key);
        }
    }
    let allowed_metadata = credential_broker
        .network_proxy
        .credential_broker_environment(&env);
    let allowed_brokered_keys = allowed_metadata.credential_keys;
    let unbrokered_credentials: HashMap<String, String> = env
        .iter()
        .filter(|(key, _)| {
            allowed_metadata.provider_keys.iter().any(|provider_key| {
                provider_key == *key || cfg!(windows) && provider_key.eq_ignore_ascii_case(key)
            }) && !provider_context_keys.iter().any(|context| {
                context == *key || cfg!(windows) && context.eq_ignore_ascii_case(key)
            }) && !allowed_brokered_keys.contains(key)
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let protected_startup_env = capture.startup_environment.as_ref().and_then(|startup| {
        let startup_env = String::from_utf8_lossy(startup.environment)
            .split('\0')
            .filter_map(|entry| entry.split_once('='))
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<HashMap<_, _>>();
        original_env
            .iter()
            .any(|(key, value)| {
                env_value(&startup_env, key) != Some(value) && {
                    let mut virtualized = value.clone();
                    credential_broker
                        .network_proxy
                        .virtualize_brokered_text(&mut virtualized, &env);
                    virtualized != *value
                }
            })
            .then(|| startup.path.to_string())
    });
    let mut brokered_alias_keys = Vec::new();
    let fail_open_aliases: HashMap<String, FailOpenAlias> = env
        .iter()
        .filter(|(key, _)| {
            !allowed_metadata.provider_keys.iter().any(|provider_key| {
                provider_key == *key || cfg!(windows) && provider_key.eq_ignore_ascii_case(key)
            })
        })
        .filter_map(|(key, value)| {
            let mut real_value = env_value(&restored_env, key)
                .or_else(|| env_value(&policy.r#set, key))?
                .clone();
            let restored_dummy = credential_broker
                .network_proxy
                .restore_brokered_text(&mut real_value);
            let mut virtualized = real_value.clone();
            let virtualized_allowed = credential_broker
                .network_proxy
                .virtualize_brokered_text(&mut virtualized, &env);
            let has_brokered_source = brokered_keys.iter().any(|source| {
                env_value(&restored_env, source)
                    .or_else(|| env_value(&policy.r#set, source))
                    .is_some_and(|source_value| {
                        credential_broker
                            .network_proxy
                            .credential_broker_source_matches_text(
                                source,
                                source_value,
                                &real_value,
                            )
                    })
            });
            let brokered_alias = !has_brokered_source
                && real_value != *value
                && (restored_dummy || virtualized_allowed && virtualized == *value);
            if !brokered_alias && virtualized_allowed {
                return None;
            }
            if brokered_alias {
                brokered_alias_keys.push(key.clone());
            }

            let authorized = credential_broker
                .network_proxy
                .credential_broker_sources_allowed(
                    &real_value,
                    if brokered_alias { value } else { &virtualized },
                    &restored_env,
                    |source| {
                        let allowed_sources = create_env_from_vars(
                            std::iter::once((source.to_string(), "1".to_string())),
                            policy,
                            /*thread_id*/ None,
                        );
                        env_value(&allowed_sources, source)
                            .is_some_and(|source_value| !source_value.is_empty())
                            && env_value(&policy.r#set, source).is_none_or(|source_value| {
                                env_value(&restored_env, source) == Some(source_value)
                            })
                    },
                );
            Some((
                key.clone(),
                FailOpenAlias {
                    value: real_value,
                    dummy_value: brokered_alias.then(|| value.clone()),
                    authorized,
                },
            ))
        })
        .collect();
    // Known unbound credentials also need live source references for their aliases.
    let mut snapshot_credential_keys = brokered_keys.clone();
    snapshot_credential_keys.extend(
        restored_env
            .iter()
            .filter(|&(key, value)| {
                discovery_metadata.provider_keys.iter().any(|source| {
                    source == key || cfg!(windows) && source.eq_ignore_ascii_case(key)
                }) && !provider_context_keys.iter().any(|context| {
                    context == key || cfg!(windows) && context.eq_ignore_ascii_case(key)
                }) && !value.is_empty()
            })
            .map(|(key, _)| key.clone()),
    );
    snapshot_credential_keys.sort_unstable();
    snapshot_credential_keys.dedup();
    let mut allowed_snapshot_credential_keys = allowed_brokered_keys.clone();
    allowed_snapshot_credential_keys.extend(
        unbrokered_credentials
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(key, _)| key.clone()),
    );
    let Some(PreparedSnapshot {
        script: snapshot,
        aliases: mut credential_env,
        rejected_alias_keys,
    }) = prepare_snapshot_credentials(
        &capture,
        SnapshotCredentialEnvironment {
            original: &original_env,
            restored: &restored_env,
            configured: &policy.r#set,
            discovered: &discovery_env,
            allowed: &snapshot_allowed_env,
            is_allowed_unset: &|key| {
                create_env_from_vars(
                    std::iter::once((key.to_string(), String::new())),
                    policy,
                    /*thread_id*/ None,
                )
                .contains_key(key)
            },
            brokered_keys: &snapshot_credential_keys,
            brokered_alias_keys: &brokered_alias_keys,
            allowed_brokered_keys: &allowed_snapshot_credential_keys,
        },
        |value| {
            credential_broker
                .network_proxy
                .virtualize_brokered_text(value, &env)
        },
    )
    else {
        bail!("shell snapshot contains a brokered credential outside exported environment");
    };
    let mut startup_credential_keys = brokered_credential_value_env_keys(&discovery_env);
    startup_credential_keys.extend(unbrokered_credentials.keys().cloned());
    unset_credential_keys.extend(rejected_alias_keys.into_iter().filter(|key| {
        startup_credential_keys.contains(key)
            || env_value(&original_env, key).is_some_and(|value| {
                snapshot_credential_keys.iter().any(|source| {
                    !brokered_keys.contains(source)
                        && env_value(&restored_env, source).is_some_and(|real| {
                            value == real || real.len() >= 16 && value.contains(real)
                        })
                })
            })
    }));

    credential_env.extend(
        allowed_brokered_keys
            .iter()
            .filter_map(|key| env_value(&env, key).map(|value| (key.clone(), value.clone()))),
    );
    credential_env.extend(unbrokered_credentials);
    for key in &brokered_alias_keys {
        if fail_open_aliases
            .get(key)
            .is_some_and(|alias| alias.authorized)
            && let Some(value) = env_value(&env, key)
        {
            credential_env.insert(key.clone(), value.clone());
        } else {
            unset_credential_keys.push(key.clone());
        }
    }
    unset_credential_keys.extend(
        startup_credential_keys
            .iter()
            .filter(|key| !contains_env_key(&env, key))
            .cloned(),
    );
    unset_credential_keys.sort_unstable();
    unset_credential_keys.dedup();
    startup_credential_keys.extend(unset_credential_keys.iter().cloned());
    startup_credential_keys.sort_unstable();
    startup_credential_keys.dedup();
    let mut context_credential_keys = HashMap::<String, Vec<String>>::new();
    let mut binding_context_credential_keys = HashMap::<String, Vec<String>>::new();
    for (key, value) in &credential_env {
        let provider_metadata = credential_broker
            .network_proxy
            .credential_broker_environment_for_text(value, &env);
        for context_key in provider_metadata.binding_keys {
            binding_context_credential_keys
                .entry(context_key)
                .or_default()
                .push(key.clone());
        }
        for context_key in provider_metadata.context_keys {
            context_credential_keys
                .entry(context_key)
                .or_default()
                .push(key.clone());
        }
    }
    let context_env = context_credential_keys
        .keys()
        .filter_map(|key| env_value(&env, key).map(|value| (key.clone(), value.clone())))
        .collect();
    Ok((
        snapshot,
        Some(SnapshotCredentials {
            network_proxy: credential_broker.network_proxy.clone(),
            broker_config_revision,
            shell_environment_policy: policy.clone(),
            protected_startup_env,
            credential_env,
            unset_credential_keys,
            startup_credential_keys,
            context_env,
            fail_open_aliases,
            context_credential_keys,
            binding_context_credential_keys,
        }),
    ))
}

fn replace_provider_context_with_trusted(
    env: &mut HashMap<String, String>,
    trusted_env: &HashMap<String, String>,
    context_keys: impl IntoIterator<Item = String>,
    preserve_untrusted_keys: &[String],
) {
    for key in context_keys {
        if let Some(value) = env_value(trusted_env, &key) {
            insert_env_value(env, &key, value);
        } else if !preserve_untrusted_keys.iter().any(|candidate| {
            candidate == &key || cfg!(windows) && candidate.eq_ignore_ascii_case(&key)
        }) {
            remove_env_value(env, &key);
        }
    }
}

fn contains_env_key(env: &HashMap<String, String>, key: &str) -> bool {
    env_value(env, key).is_some()
}

fn env_value<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a String> {
    env.get(key).or_else(|| {
        cfg!(windows)
            .then(|| {
                env.iter().find_map(|(candidate, value)| {
                    candidate.eq_ignore_ascii_case(key).then_some(value)
                })
            })
            .flatten()
    })
}

fn insert_env_value(env: &mut HashMap<String, String>, key: &str, value: &str) {
    #[cfg(windows)]
    env.retain(|candidate, _| !candidate.eq_ignore_ascii_case(key));
    env.insert(key.to_string(), value.to_string());
}

fn remove_env_value(env: &mut HashMap<String, String>, key: &str) {
    #[cfg(windows)]
    env.retain(|candidate, _| !candidate.eq_ignore_ascii_case(key));
    #[cfg(not(windows))]
    env.remove(key);
}

#[derive(Clone, Copy)]
enum SnapshotShellMode<'a> {
    Login,
    NonLogin,
    Validation(&'a AbsolutePathBuf),
}

async fn validate_snapshot(
    shell: &Shell,
    snapshot_path: &AbsolutePathBuf,
    cwd: &AbsolutePathBuf,
    credential_broker: Option<&SnapshotCredentialBroker>,
    sandbox: Option<&ShellSnapshotSandbox>,
) -> Result<()> {
    let snapshot_path_display = snapshot_path.display();
    let script = format!("set -e; . \"{snapshot_path_display}\"");
    run_script_with_timeout(
        shell,
        &script,
        SNAPSHOT_TIMEOUT,
        SnapshotShellMode::Validation(snapshot_path),
        cwd,
        credential_broker,
        sandbox,
    )
    .await
    .map(|_| ())
}

async fn run_script_with_timeout(
    shell: &Shell,
    script: &str,
    snapshot_timeout: Duration,
    shell_mode: SnapshotShellMode<'_>,
    cwd: &AbsolutePathBuf,
    credential_broker: Option<&SnapshotCredentialBroker>,
    sandbox: Option<&ShellSnapshotSandbox>,
) -> Result<String> {
    let suppress_startup_files =
        credential_broker.is_some() && matches!(shell_mode, SnapshotShellMode::Validation(_));
    let mut args = shell.derive_exec_args(script, matches!(shell_mode, SnapshotShellMode::Login));
    if suppress_startup_files && shell.shell_type == ShellType::Zsh {
        args[1] = "-fc".to_string();
    }
    let shell_name = shell.name();
    let mut prepared_env = None;
    if let Some(credential_broker) = credential_broker {
        let policy = &credential_broker.shell_environment_policy;
        let mut env = create_env_from_vars(std::env::vars(), policy, /*thread_id*/ None);
        if suppress_startup_files {
            env.remove("BASH_ENV");
            env.remove("ENV");
        }
        credential_broker.network_proxy.apply_to_env(&mut env);
        prepared_env = Some(env);
    }
    if let Some(sandbox) = sandbox {
        let snapshot_read_path = match shell_mode {
            SnapshotShellMode::Validation(path) => Some(path),
            SnapshotShellMode::Login | SnapshotShellMode::NonLogin => None,
        };
        return sandbox
            .run(
                args,
                cwd,
                prepared_env.unwrap_or_else(|| std::env::vars().collect()),
                snapshot_timeout,
                shell_name,
                snapshot_read_path,
            )
            .await;
    }

    // Handler is kept as guard to control the drop. The `mut` pattern is required because .args()
    // returns a ref of handler.
    let mut handler = Command::new(&args[0]);
    handler.args(&args[1..]);
    handler.stdin(Stdio::null());
    handler.current_dir(cwd);
    #[cfg(unix)]
    {
        // The shared launcher needs the complete child environment.
        handler.env_clear();
        handler.envs(std::env::vars_os());
    }
    if let Some(env) = prepared_env {
        handler.env_clear();
        handler.envs(env);
    }
    codex_protocol::shell_environment::scrub_non_inheritable_env_vars(handler.as_std_mut());
    #[cfg(unix)]
    let output = {
        let settings = handler.as_std();
        // Preserve original inputs: std replaces strings containing NUL bytes.
        let mut command = codex_utils_pty::Command::new(&args[0]);
        command
            .args(&args[1..])
            .envs(
                settings
                    .get_envs()
                    .filter_map(|(key, value)| value.map(|value| (key, value))),
            )
            .current_dir(cwd)
            .stdin(codex_utils_pty::ChildStdin::File(
                std::fs::File::open("/dev/null")?.into(),
            ))
            .process_mode(codex_utils_pty::ProcessMode::NewSession);
        async move { command.spawn()?.wait_with_output().await }
    };
    #[cfg(not(unix))]
    let output = handler.kill_on_drop(true).output();
    let output = timeout(snapshot_timeout, output)
        .await
        .map_err(|_| anyhow!("Snapshot command timed out for {shell_name}"))?
        .with_context(|| format!("Failed to execute {shell_name}"))?;

    if !output.status.success() {
        let status = output.status;
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Snapshot command exited with status {status}: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Removes shell snapshots that either lack a matching session rollout file or
/// whose rollouts have not been updated within the retention window.
/// The active session id is exempt from cleanup.
pub async fn cleanup_stale_snapshots(
    codex_home: &AbsolutePathBuf,
    active_session_id: ThreadId,
    state_db: Option<StateDbHandle>,
) -> Result<()> {
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);

    let mut entries = match fs::read_dir(&snapshot_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let now = SystemTime::now();
    let active_session_id = active_session_id.to_string();

    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let path = entry.path();

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(session_id) = snapshot_session_id_from_file_name(&file_name) else {
            remove_snapshot_file(&path).await;
            continue;
        };
        if session_id == active_session_id {
            continue;
        }

        let rollout_path =
            find_thread_path_by_id_str(codex_home, session_id, state_db.as_deref()).await?;
        let Some(rollout_path) = rollout_path else {
            remove_snapshot_file(&path).await;
            continue;
        };

        let modified = match fs::metadata(&rollout_path).await.and_then(|m| m.modified()) {
            Ok(modified) => modified,
            Err(err) => {
                tracing::warn!(
                    "Failed to check rollout age for snapshot {}: {err:?}",
                    path.display()
                );
                continue;
            }
        };

        if now
            .duration_since(modified)
            .ok()
            .is_some_and(|age| age >= SNAPSHOT_RETENTION)
        {
            remove_snapshot_file(&path).await;
        }
    }

    Ok(())
}

async fn remove_snapshot_file(path: &Path) {
    if let Err(err) = fs::remove_file(path).await {
        tracing::warn!("Failed to delete shell snapshot at {:?}: {err:?}", path);
    }
}

fn snapshot_session_id_from_file_name(file_name: &str) -> Option<&str> {
    let (stem, extension) = file_name.rsplit_once('.')?;
    match extension {
        "sh" | "ps1" => Some(
            stem.split_once('.')
                .map_or(stem, |(session_id, _generation)| session_id),
        ),
        _ if extension.starts_with("tmp-") => Some(stem),
        _ => None,
    }
}

#[cfg(test)]
#[path = "shell_snapshot_tests.rs"]
mod tests;
