use super::CodexClient;
use super::plugin_analytics_capture::PluginEventIdentity;
use super::plugin_analytics_capture::read_events_for_remote_plugin;
use super::plugin_analytics_capture::validate_mutation_events;
use super::plugin_analytics_smoke::ANALYTICS_CAPTURE_ENV_VAR;
use super::plugin_analytics_smoke::prepare_capture_file;
use super::plugin_analytics_smoke::wait_until_capture_is_ready;
use super::shell_quote;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::PluginUninstallResponse;
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const REMOTE_MARKETPLACE_HINT: &str = "openai-curated-remote";
const STATE_TIMEOUT: Duration = Duration::from_secs(15);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn run(
    codex_bin: &Path,
    config_overrides: &[String],
    remote_plugin_id: &str,
    confirmation: AccountMutationConfirmation,
    capture_file: Option<PathBuf>,
) -> Result<()> {
    require_confirmation(confirmation)?;
    let capture_path = capture_file.unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "codex-plugin-analytics-mutation-{}.jsonl",
            process::id()
        ))
    });
    prepare_capture_file(&capture_path)?;
    let mut client = spawn_client(codex_bin, config_overrides, &capture_path)?;
    wait_until_capture_is_ready(&capture_path)?;
    client.initialize()?;

    let initial = read_remote_plugin(&mut client, remote_plugin_id)?;
    validate_initial_plugin(&initial, remote_plugin_id)?;
    println!(
        "remote plugin mutation smoke: local_id={} remote_id={} marketplace={}",
        initial.plugin_id, initial.remote_plugin_id, initial.marketplace_name
    );

    let MutationSequenceResult {
        result: sequence_result,
        uninstall_rpc_failed,
    } = run_mutation_sequence(&mut client, &capture_path, &initial);
    let restoration = restore_uninstalled_state(&mut client, remote_plugin_id);
    println!("capture file: {}", capture_path.display());

    match (sequence_result, restoration) {
        (Ok(events), RestorationStatus::Clean) => {
            println!(
                "\n[plugin analytics mutation smoke validated]\n{}",
                serde_json::to_string_pretty(&events)?
            );
            println!("PASS: analytics validated; original uninstalled state restored");
            Ok(())
        }
        (Err(err), RestorationStatus::Clean) if uninstall_rpc_failed => {
            eprintln!(
                "FAIL-LOCAL-CACHE: backend state is uninstalled, but the uninstall RPC failed after the backend mutation: {err:#}"
            );
            Err(err)
        }
        (Err(err), RestorationStatus::Clean) => {
            eprintln!("FAIL-CLEAN: {err:#}");
            eprintln!("The original uninstalled account state was restored.");
            Err(err)
        }
        (sequence_result, RestorationStatus::LocalCleanupFailure(cleanup_err)) => {
            let sequence_err = sequence_result.err();
            eprintln!(
                "FAIL-LOCAL-CACHE: backend state is uninstalled, but local cleanup reported an error: {cleanup_err:#}"
            );
            Err(sequence_err.unwrap_or(cleanup_err))
        }
        (sequence_result, RestorationStatus::Dirty(cleanup_err)) => {
            if let Err(err) = sequence_result {
                eprintln!("mutation smoke failed before cleanup: {err:#}");
            }
            print_dirty_recovery(codex_bin, config_overrides, remote_plugin_id, &cleanup_err);
            Err(cleanup_err)
        }
        (sequence_result, RestorationStatus::Unknown(cleanup_err)) => {
            if let Err(err) = sequence_result {
                eprintln!("mutation smoke failed before final state verification: {err:#}");
            }
            eprintln!(
                "FAIL-UNKNOWN: could not verify whether `{remote_plugin_id}` is installed: {cleanup_err:#}"
            );
            print_recovery_command(codex_bin, config_overrides, remote_plugin_id);
            Err(cleanup_err)
        }
    }
}

pub(super) fn run_cleanup(
    codex_bin: &Path,
    config_overrides: &[String],
    remote_plugin_id: &str,
    confirmation: AccountMutationConfirmation,
) -> Result<()> {
    require_confirmation(confirmation)?;
    let mut overrides = config_overrides.to_vec();
    overrides.extend([
        "analytics.enabled=false".to_string(),
        "features.plugins=true".to_string(),
        "features.remote_plugin=true".to_string(),
    ]);
    let mut client = CodexClient::spawn_stdio(codex_bin, &overrides)?;
    client.initialize()?;

    match restore_uninstalled_state(&mut client, remote_plugin_id) {
        RestorationStatus::Clean => {
            println!("PASS: `{remote_plugin_id}` is uninstalled");
            Ok(())
        }
        RestorationStatus::LocalCleanupFailure(err) => {
            eprintln!(
                "FAIL-LOCAL-CACHE: backend state is uninstalled, but local cleanup reported an error: {err:#}"
            );
            Err(err)
        }
        RestorationStatus::Dirty(err) => {
            print_dirty_recovery(codex_bin, config_overrides, remote_plugin_id, &err);
            Err(err)
        }
        RestorationStatus::Unknown(err) => {
            eprintln!(
                "FAIL-UNKNOWN: could not verify whether `{remote_plugin_id}` is installed: {err:#}"
            );
            Err(err)
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum AccountMutationConfirmation {
    Confirmed,
    Missing,
}

impl AccountMutationConfirmation {
    pub(super) fn from_flag(confirm_account_mutation: bool) -> Self {
        if confirm_account_mutation {
            Self::Confirmed
        } else {
            Self::Missing
        }
    }
}

fn require_confirmation(confirmation: AccountMutationConfirmation) -> Result<()> {
    if matches!(confirmation, AccountMutationConfirmation::Missing) {
        bail!(
            "this command installs and uninstalls a plugin on the active account; rerun with --confirm-account-mutation"
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ExpectedInstalledState {
    Installed,
    Uninstalled,
}

impl ExpectedInstalledState {
    fn is_installed(self) -> bool {
        matches!(self, Self::Installed)
    }
}

fn spawn_client(
    codex_bin: &Path,
    config_overrides: &[String],
    capture_path: &Path,
) -> Result<CodexClient> {
    let mut overrides = config_overrides.to_vec();
    overrides.extend([
        "analytics.enabled=true".to_string(),
        "features.plugins=true".to_string(),
        "features.remote_plugin=true".to_string(),
    ]);
    let environment = vec![(
        OsString::from(ANALYTICS_CAPTURE_ENV_VAR),
        capture_path.as_os_str().to_os_string(),
    )];
    CodexClient::spawn_stdio_with_env(codex_bin, &overrides, &environment)
}

#[derive(Clone, Debug)]
struct RemotePluginExpectation {
    plugin_id: String,
    remote_plugin_id: String,
    plugin_name: String,
    marketplace_name: String,
    installed: bool,
    install_policy: PluginInstallPolicy,
    availability: PluginAvailability,
}

fn read_remote_plugin(
    client: &mut CodexClient,
    remote_plugin_id: &str,
) -> Result<RemotePluginExpectation> {
    let request_id = client.request_id();
    let response: PluginReadResponse = client.send_request(
        ClientRequest::PluginRead {
            request_id: request_id.clone(),
            params: PluginReadParams {
                marketplace_path: None,
                remote_marketplace_name: Some(REMOTE_MARKETPLACE_HINT.to_string()),
                plugin_name: remote_plugin_id.to_string(),
            },
        },
        request_id,
        "plugin/read",
    )?;
    let summary = response.plugin.summary;
    let actual_remote_plugin_id = summary
        .remote_plugin_id
        .with_context(|| format!("plugin/read returned no remote id for `{remote_plugin_id}`"))?;
    if actual_remote_plugin_id != remote_plugin_id {
        bail!(
            "plugin/read returned remote id `{actual_remote_plugin_id}` for requested id `{remote_plugin_id}`"
        );
    }
    Ok(RemotePluginExpectation {
        plugin_id: summary.id,
        remote_plugin_id: actual_remote_plugin_id,
        plugin_name: summary.name,
        marketplace_name: response.plugin.marketplace_name,
        installed: summary.installed,
        install_policy: summary.install_policy,
        availability: summary.availability,
    })
}

fn validate_initial_plugin(plugin: &RemotePluginExpectation, remote_plugin_id: &str) -> Result<()> {
    if plugin.installed {
        bail!(
            "refusing to run: remote plugin `{remote_plugin_id}` is already installed; choose an initially uninstalled plugin"
        );
    }
    if plugin.availability != PluginAvailability::Available {
        bail!(
            "remote plugin `{remote_plugin_id}` is not available: {:?}",
            plugin.availability
        );
    }
    if plugin.install_policy == PluginInstallPolicy::NotAvailable {
        bail!("remote plugin `{remote_plugin_id}` is not available for install");
    }
    Ok(())
}

struct MutationSequenceResult {
    result: Result<Vec<Value>>,
    uninstall_rpc_failed: bool,
}

fn run_mutation_sequence(
    client: &mut CodexClient,
    capture_path: &Path,
    expected: &RemotePluginExpectation,
) -> MutationSequenceResult {
    let mut uninstall_rpc_failed = false;
    let result = (|| {
        install_remote_plugin(client, expected)?;
        wait_for_installed_state(
            client,
            &expected.remote_plugin_id,
            ExpectedInstalledState::Installed,
        )?;
        wait_for_remote_plugin_event(
            capture_path,
            &expected.remote_plugin_id,
            "codex_plugin_installed",
        )?;

        let uninstall_error = uninstall_remote_plugin(client, &expected.remote_plugin_id).err();
        uninstall_rpc_failed = uninstall_error.is_some();
        wait_for_installed_state(
            client,
            &expected.remote_plugin_id,
            ExpectedInstalledState::Uninstalled,
        )
        .map_err(|state_err| {
            if let Some(err) = uninstall_error.as_ref() {
                anyhow!("plugin/uninstall failed: {err:#}; final state check failed: {state_err:#}")
            } else {
                state_err
            }
        })?;

        let captured_events =
            read_events_for_remote_plugin(capture_path, &expected.remote_plugin_id)?;
        let events = validate_mutation_events(
            captured_events,
            PluginEventIdentity {
                plugin_id: &expected.remote_plugin_id,
                plugin_name: &expected.plugin_name,
                marketplace_name: &expected.marketplace_name,
            },
        )?;
        if let Some(err) = uninstall_error {
            return Err(err.context(
                "plugin/uninstall reported an error after the backend became uninstalled",
            ));
        }
        Ok(events)
    })();

    MutationSequenceResult {
        result,
        uninstall_rpc_failed,
    }
}

fn install_remote_plugin(client: &mut CodexClient, plugin: &RemotePluginExpectation) -> Result<()> {
    let request_id = client.request_id();
    let _: PluginInstallResponse = client.send_request(
        ClientRequest::PluginInstall {
            request_id: request_id.clone(),
            params: PluginInstallParams {
                marketplace_path: None,
                remote_marketplace_name: Some(plugin.marketplace_name.clone()),
                plugin_name: plugin.remote_plugin_id.clone(),
            },
        },
        request_id,
        "plugin/install",
    )?;
    Ok(())
}

fn uninstall_remote_plugin(client: &mut CodexClient, remote_plugin_id: &str) -> Result<()> {
    let request_id = client.request_id();
    let _: PluginUninstallResponse = client.send_request(
        ClientRequest::PluginUninstall {
            request_id: request_id.clone(),
            params: PluginUninstallParams {
                plugin_id: remote_plugin_id.to_string(),
            },
        },
        request_id,
        "plugin/uninstall",
    )?;
    Ok(())
}

fn wait_for_installed_state(
    client: &mut CodexClient,
    remote_plugin_id: &str,
    expected_state: ExpectedInstalledState,
) -> Result<RemotePluginExpectation> {
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        match read_remote_plugin(client, remote_plugin_id) {
            Ok(plugin) if plugin.installed == expected_state.is_installed() => return Ok(plugin),
            Ok(_) => {}
            Err(err) if Instant::now() >= deadline => return Err(err),
            Err(_) => {}
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for remote plugin `{remote_plugin_id}` to become {expected_state:?}"
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

enum RestorationStatus {
    Clean,
    LocalCleanupFailure(anyhow::Error),
    Dirty(anyhow::Error),
    Unknown(anyhow::Error),
}

fn restore_uninstalled_state(
    client: &mut CodexClient,
    remote_plugin_id: &str,
) -> RestorationStatus {
    let current = match read_remote_plugin(client, remote_plugin_id) {
        Ok(current) => current,
        Err(err) => return RestorationStatus::Unknown(err),
    };
    if !current.installed {
        return RestorationStatus::Clean;
    }

    let uninstall_result = uninstall_remote_plugin(client, remote_plugin_id);
    match wait_for_installed_state(
        client,
        remote_plugin_id,
        ExpectedInstalledState::Uninstalled,
    ) {
        Ok(_) => match uninstall_result {
            Ok(()) => RestorationStatus::Clean,
            Err(err) => RestorationStatus::LocalCleanupFailure(err),
        },
        Err(state_err) => {
            let error = match uninstall_result {
                Ok(()) => state_err,
                Err(uninstall_err) => anyhow!(
                    "cleanup uninstall failed: {uninstall_err:#}; state verification failed: {state_err:#}"
                ),
            };
            RestorationStatus::Dirty(error)
        }
    }
}

fn wait_for_remote_plugin_event(
    path: &Path,
    remote_plugin_id: &str,
    event_type: &str,
) -> Result<()> {
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    loop {
        let events = read_events_for_remote_plugin(path, remote_plugin_id)?;
        if events.iter().any(|event| event["event_type"] == event_type) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for `{event_type}` for remote plugin `{remote_plugin_id}`");
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn print_dirty_recovery(
    codex_bin: &Path,
    config_overrides: &[String],
    remote_plugin_id: &str,
    err: &anyhow::Error,
) {
    eprintln!(
        "FAIL-DIRTY: remote plugin `{remote_plugin_id}` still appears installed after cleanup: {err:#}"
    );
    print_recovery_command(codex_bin, config_overrides, remote_plugin_id);
}

fn print_recovery_command(codex_bin: &Path, config_overrides: &[String], remote_plugin_id: &str) {
    let test_client = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "codex-app-server-test-client".to_string());
    let mut command = format!(
        "{} --codex-bin {}",
        shell_quote(&test_client),
        shell_quote(&codex_bin.display().to_string())
    );
    for override_kv in config_overrides {
        command.push_str(&format!(" --config {}", shell_quote(override_kv)));
    }
    command.push_str(&format!(
        " plugin-remote-uninstall --remote-plugin-id {} --confirm-account-mutation",
        shell_quote(remote_plugin_id)
    ));
    eprintln!("Recovery command:");
    eprintln!("  {command}");
}
