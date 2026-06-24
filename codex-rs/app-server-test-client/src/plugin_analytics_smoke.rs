use super::CodexClient;
use super::loopback_responses_server::LoopbackResponsesServer;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstalledParams;
use codex_app_server_protocol::PluginInstalledResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::WriteStatus;
use serde_json::Value;
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::Duration;
use std::time::Instant;

pub(super) const ANALYTICS_CAPTURE_ENV_VAR: &str = "CODEX_ANALYTICS_EVENTS_CAPTURE_FILE";
const TEST_USER_CONFIG_ENV_VAR: &str = "CODEX_APP_SERVER_TEST_USER_CONFIG_FILE";
const CAPTURE_READY_TIMEOUT: Duration = Duration::from_secs(5);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const CAPTURE_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PLUGIN_READY_TIMEOUT: Duration = Duration::from_secs(30);
const PLUGIN_READY_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const MOCK_MODEL_SLUG: &str = "plugin-analytics-smoke";
const MOCK_PROVIDER_ID: &str = "plugin_analytics_smoke";

pub(super) fn run(
    codex_bin: &Path,
    config_overrides: &[String],
    plugin_id: &str,
    capture_file: Option<PathBuf>,
) -> Result<()> {
    let capture_path = capture_file.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("codex-plugin-analytics-{}.jsonl", process::id()))
    });
    prepare_capture_file(&capture_path)?;

    let temporary_config = TemporaryConfigFile::create()?;
    let responses_server = LoopbackResponsesServer::start()?;
    let mut overrides = config_overrides.to_vec();
    overrides.extend(smoke_config_overrides(responses_server.base_url())?);

    let child_environment = vec![
        (
            OsString::from(ANALYTICS_CAPTURE_ENV_VAR),
            capture_path.as_os_str().to_os_string(),
        ),
        (
            OsString::from(TEST_USER_CONFIG_ENV_VAR),
            temporary_config.path().as_os_str().to_os_string(),
        ),
    ];
    let mut client = CodexClient::spawn_stdio_with_env(codex_bin, &overrides, &child_environment)?;
    wait_until_capture_is_ready(&capture_path)?;
    client.initialize()?;

    let installed = plugin_installed(&mut client)?;
    let expected = expected_plugin(&installed, plugin_id)?;
    write_plugin_enabled(
        &mut client,
        temporary_config.path(),
        plugin_id,
        /*enabled*/ false,
    )?;
    write_plugin_enabled(
        &mut client,
        temporary_config.path(),
        plugin_id,
        /*enabled*/ true,
    )?;

    wait_for_plugin_usage(&mut client, &capture_path, &expected)?;

    let events = wait_for_plugin_events(&capture_path, plugin_id)?;
    let validated = validate_plugin_events(events, &expected)?;
    println!(
        "\n[plugin analytics smoke validated]\n{}",
        serde_json::to_string_pretty(&validated)?
    );
    println!("capture file: {}", capture_path.display());
    Ok(())
}

fn run_plugin_turn(client: &mut CodexClient, expected: &ExpectedPlugin) -> Result<String> {
    let thread = client.thread_start(ThreadStartParams {
        model: Some(MOCK_MODEL_SLUG.to_string()),
        model_provider: Some(MOCK_PROVIDER_ID.to_string()),
        base_instructions: Some(String::new()),
        developer_instructions: Some(String::new()),
        ephemeral: Some(true),
        ..Default::default()
    })?;
    let turn = client.turn_start(TurnStartParams {
        thread_id: thread.thread.id.clone(),
        client_user_message_id: None,
        input: vec![UserInput::Mention {
            name: expected.plugin_name.clone(),
            path: format!("plugin://{}", expected.plugin_id),
        }],
        ..Default::default()
    })?;
    client.stream_turn(&thread.thread.id, &turn.turn.id)?;
    if client.last_turn_status != Some(TurnStatus::Completed) {
        bail!(
            "plugin analytics smoke turn did not complete: status={:?}, error={:?}",
            client.last_turn_status,
            client.last_turn_error_message
        );
    }
    Ok(turn.turn.id)
}

fn wait_for_plugin_usage(
    client: &mut CodexClient,
    capture_path: &Path,
    expected: &ExpectedPlugin,
) -> Result<()> {
    let deadline = Instant::now() + PLUGIN_READY_TIMEOUT;
    let mut attempts = 0;
    loop {
        attempts += 1;
        let turn_id = run_plugin_turn(client, expected)?;
        // Turn completion is queued after plugin usage, so its captured event is the
        // barrier that tells us whether this attempt resolved the plugin.
        let events = wait_for_turn_analytics(capture_path, &turn_id)?;
        if events.iter().any(|event| {
            event["event_type"] == "codex_plugin_used"
                && event["event_params"]["turn_id"].as_str() == Some(turn_id.as_str())
                && event["event_params"]["plugin_id"].as_str() == Some(expected.plugin_id.as_str())
        }) {
            if attempts > 1 {
                println!("remote plugin bundle became ready after {attempts} turn attempts");
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for remote plugin bundle `{}` to become usable after {attempts} turn attempts",
                expected.plugin_id
            );
        }
        thread::sleep(PLUGIN_READY_RETRY_INTERVAL);
    }
}

#[derive(Debug)]
struct ExpectedPlugin {
    plugin_id: String,
    remote_plugin_id: String,
    plugin_name: String,
    marketplace_name: String,
}

fn plugin_installed(client: &mut CodexClient) -> Result<PluginInstalledResponse> {
    let request_id = client.request_id();
    client.send_request(
        ClientRequest::PluginInstalled {
            request_id: request_id.clone(),
            params: PluginInstalledParams {
                cwds: None,
                install_suggestion_plugin_names: None,
            },
        },
        request_id,
        "plugin/installed",
    )
}

fn expected_plugin(response: &PluginInstalledResponse, plugin_id: &str) -> Result<ExpectedPlugin> {
    let matches = response
        .marketplaces
        .iter()
        .flat_map(|marketplace| {
            marketplace
                .plugins
                .iter()
                .filter(move |plugin| plugin.id == plugin_id)
                .map(move |plugin| (marketplace, plugin))
        })
        .collect::<Vec<_>>();
    let [(marketplace, plugin)] = matches.as_slice() else {
        bail!(
            "expected exactly one installed plugin with local id `{plugin_id}`, found {}",
            matches.len()
        );
    };
    if !plugin.installed {
        bail!("plugin `{plugin_id}` is not installed");
    }
    if !plugin.enabled {
        bail!("plugin `{plugin_id}` is installed remotely but disabled");
    }
    if plugin.availability != PluginAvailability::Available {
        bail!(
            "plugin `{plugin_id}` is not available: {:?}",
            plugin.availability
        );
    }
    let remote_plugin_id = plugin
        .remote_plugin_id
        .as_ref()
        .with_context(|| format!("plugin `{plugin_id}` does not have a remote plugin id"))?
        .clone();

    Ok(ExpectedPlugin {
        plugin_id: plugin.id.clone(),
        remote_plugin_id,
        plugin_name: plugin.name.clone(),
        marketplace_name: marketplace.name.clone(),
    })
}

fn write_plugin_enabled(
    client: &mut CodexClient,
    config_path: &Path,
    plugin_id: &str,
    enabled: bool,
) -> Result<()> {
    let request_id = client.request_id();
    let response: ConfigWriteResponse = client.send_request(
        ClientRequest::ConfigValueWrite {
            request_id: request_id.clone(),
            params: ConfigValueWriteParams {
                key_path: format!("plugins.{plugin_id}.enabled"),
                value: json!(enabled),
                merge_strategy: MergeStrategy::Replace,
                file_path: Some(config_path.display().to_string()),
                expected_version: None,
            },
        },
        request_id,
        "config/value/write",
    )?;
    println!(
        "< config/value/write plugin={plugin_id} enabled={enabled} status={:?}",
        response.status
    );
    if response.status != WriteStatus::Ok {
        bail!(
            "config/value/write for plugin `{plugin_id}` enabled={enabled} was overridden: {:?}",
            response.overridden_metadata
        );
    }
    Ok(())
}

fn smoke_config_overrides(responses_base_url: &str) -> Result<Vec<String>> {
    let provider_base_url = serde_json::to_string(&format!("{responses_base_url}/v1"))
        .context("serialize mock provider base URL")?;
    Ok(vec![
        "analytics.enabled=true".to_string(),
        "features.plugins=true".to_string(),
        "features.remote_plugin=true".to_string(),
        format!("model={}", quoted(MOCK_MODEL_SLUG)?),
        format!("model_provider={}", quoted(MOCK_PROVIDER_ID)?),
        format!(
            "model_providers.{MOCK_PROVIDER_ID}.name={}",
            quoted("Plugin analytics smoke mock provider")?
        ),
        format!("model_providers.{MOCK_PROVIDER_ID}.base_url={provider_base_url}"),
        format!(
            "model_providers.{MOCK_PROVIDER_ID}.wire_api={}",
            quoted("responses")?
        ),
        format!("model_providers.{MOCK_PROVIDER_ID}.requires_openai_auth=false"),
        format!("model_providers.{MOCK_PROVIDER_ID}.request_max_retries=0"),
        format!("model_providers.{MOCK_PROVIDER_ID}.stream_max_retries=0"),
    ])
}

fn quoted(value: &str) -> Result<String> {
    serde_json::to_string(value).context("serialize config string")
}

pub(super) fn prepare_capture_file(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("capture file must have a parent directory")?;
    if !parent.is_dir() {
        bail!(
            "capture file parent directory does not exist: {}",
            parent.display()
        );
    }
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err)
                .with_context(|| format!("remove previous capture file {}", path.display()));
        }
    }
    Ok(())
}

pub(super) fn wait_until_capture_is_ready(path: &Path) -> Result<()> {
    let deadline = Instant::now() + CAPTURE_READY_TIMEOUT;
    loop {
        match fs::metadata(path) {
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("inspect capture file {}", path.display()));
            }
        }
        if Instant::now() >= deadline {
            bail!(
                "analytics capture did not become ready at {}; use a debug Codex binary",
                path.display()
            );
        }
        thread::sleep(CAPTURE_POLL_INTERVAL);
    }
}

fn wait_for_plugin_events(path: &Path, plugin_id: &str) -> Result<Vec<Value>> {
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    loop {
        let events = read_plugin_events(path, plugin_id)?;
        if required_event_types()
            .iter()
            .all(|event_type| event_count(&events, event_type) >= 1)
        {
            return Ok(events);
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for plugin analytics events in {}: found {:?}",
                path.display(),
                events
                    .iter()
                    .filter_map(|event| event["event_type"].as_str())
                    .collect::<Vec<_>>()
            );
        }
        thread::sleep(CAPTURE_POLL_INTERVAL);
    }
}

fn wait_for_turn_analytics(path: &Path, turn_id: &str) -> Result<Vec<Value>> {
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    loop {
        let events = read_capture_events(path)?;
        if events.iter().any(|event| {
            event["event_type"] == "codex_turn_event"
                && event["event_params"]["turn_id"].as_str() == Some(turn_id)
        }) {
            return Ok(events);
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for turn analytics for `{turn_id}` in {}",
                path.display()
            );
        }
        thread::sleep(CAPTURE_POLL_INTERVAL);
    }
}

fn read_plugin_events(path: &Path, plugin_id: &str) -> Result<Vec<Value>> {
    Ok(read_capture_events(path)?
        .into_iter()
        .filter(|event| event["event_params"]["plugin_id"] == plugin_id)
        .collect())
}

fn read_capture_events(path: &Path) -> Result<Vec<Value>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("read capture file {}", path.display()));
        }
    };
    let mut captured = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let payload: Value = serde_json::from_str(line).with_context(|| {
            format!(
                "parse analytics capture line {} from {}",
                index + 1,
                path.display()
            )
        })?;
        let events = payload["events"]
            .as_array()
            .context("analytics capture payload is missing events")?;
        captured.extend(events.iter().cloned());
    }
    Ok(captured)
}

fn validate_plugin_events(events: Vec<Value>, expected: &ExpectedPlugin) -> Result<Vec<Value>> {
    let mut validated = Vec::new();
    for event_type in required_event_types() {
        let matching = events
            .iter()
            .filter(|event| event["event_type"] == event_type)
            .collect::<Vec<_>>();
        let [event] = matching.as_slice() else {
            bail!(
                "expected exactly one `{event_type}` event for `{}`, found {}",
                expected.plugin_id,
                matching.len()
            );
        };
        validate_identity(event, expected)?;
        if event_type == "codex_plugin_used" {
            validate_used_metadata(event)?;
        }
        validated.push((*event).clone());
    }
    Ok(validated)
}

fn required_event_types() -> [&'static str; 3] {
    [
        "codex_plugin_disabled",
        "codex_plugin_enabled",
        "codex_plugin_used",
    ]
}

fn event_count(events: &[Value], event_type: &str) -> usize {
    events
        .iter()
        .filter(|event| event["event_type"] == event_type)
        .count()
}

fn validate_identity(event: &Value, expected: &ExpectedPlugin) -> Result<()> {
    let params = &event["event_params"];
    require_string(params, "plugin_id", &expected.plugin_id)?;
    require_string(params, "remote_plugin_id", &expected.remote_plugin_id)?;
    require_string(params, "plugin_name", &expected.plugin_name)?;
    require_string(params, "marketplace_name", &expected.marketplace_name)
}

fn validate_used_metadata(event: &Value) -> Result<()> {
    let params = &event["event_params"];
    for field in [
        "has_skills",
        "mcp_server_count",
        "connector_ids",
        "mcp_server_names",
        "thread_id",
        "turn_id",
        "model_slug",
    ] {
        if params.get(field).is_none_or(Value::is_null) {
            bail!("codex_plugin_used event has null or missing `{field}`");
        }
    }
    require_string(params, "model_slug", MOCK_MODEL_SLUG)
}

fn require_string(params: &Value, field: &str, expected: &str) -> Result<()> {
    let actual = params.get(field).and_then(Value::as_str);
    if actual != Some(expected) {
        bail!("expected `{field}` to be `{expected}`, got {actual:?}");
    }
    Ok(())
}

struct TemporaryConfigFile {
    path: PathBuf,
}

impl TemporaryConfigFile {
    fn create() -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "codex-plugin-analytics-config-{}.toml",
            process::id()
        ));
        fs::write(&path, "")
            .with_context(|| format!("create temporary config file {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
