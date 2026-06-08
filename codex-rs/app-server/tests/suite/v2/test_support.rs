use std::path::Path;
use tokio::time::Duration;

// macOS and Windows CI can spend tens of seconds starting the app-server test
// binary under Bazel before it accepts requests.
#[cfg(any(target_os = "macos", windows))]
pub(super) const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
pub(super) const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
