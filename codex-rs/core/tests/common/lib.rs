#![expect(clippy::expect_used)]

use tempfile::TempDir;

use codex_core::CodexConversation;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;

pub mod responses;
pub mod test_codex;
pub mod test_codex_exec;

/// Returns a default `Config` whose on-disk state is confined to the provided
/// temporary directory. Using a per-test directory keeps tests hermetic and
/// avoids clobbering a developer’s real `~/.codex`.
pub fn load_default_config_for_test(codex_home: &TempDir) -> Config {
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )
    .expect("defaults for test should always succeed")
}

/// Builds an SSE stream body from a JSON fixture.
///
/// The fixture must contain an array of objects where each object represents a
/// single SSE event with at least a `type` field matching the `event:` value.
/// Additional fields become the JSON payload for the `data:` line. An object
/// with only a `type` field results in an event with no `data:` section. This
/// makes it trivial to extend the fixtures as OpenAI adds new event kinds or
/// fields.
pub fn load_sse_fixture(path: impl AsRef<std::path::Path>) -> String {
    let events: Vec<serde_json::Value> =
        serde_json::from_reader(std::fs::File::open(path).expect("read fixture"))
            .expect("parse JSON fixture");
    events
        .into_iter()
        .map(|e| {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                format!("event: {kind}\n\n")
            } else {
                format!("event: {kind}\ndata: {e}\n\n")
            }
        })
        .collect()
}

pub fn load_sse_fixture_with_id_from_str(raw: &str, id: &str) -> String {
    let replaced = raw.replace("__ID__", id);
    let events: Vec<serde_json::Value> =
        serde_json::from_str(&replaced).expect("parse JSON fixture");
    events
        .into_iter()
        .map(|e| {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                format!("event: {kind}\n\n")
            } else {
                format!("event: {kind}\ndata: {e}\n\n")
            }
        })
        .collect()
}

/// Same as [`load_sse_fixture`], but replaces the placeholder `__ID__` in the
/// fixture template with the supplied identifier before parsing. This lets a
/// single JSON template be reused by multiple tests that each need a unique
/// `response_id`.
pub fn load_sse_fixture_with_id(path: impl AsRef<std::path::Path>, id: &str) -> String {
    let raw = std::fs::read_to_string(path).expect("read fixture template");
    let replaced = raw.replace("__ID__", id);
    let events: Vec<serde_json::Value> =
        serde_json::from_str(&replaced).expect("parse JSON fixture");
    events
        .into_iter()
        .map(|e| {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                format!("event: {kind}\n\n")
            } else {
                format!("event: {kind}\ndata: {e}\n\n")
            }
        })
        .collect()
}

pub async fn wait_for_event<F>(
    codex: &CodexConversation,
    predicate: F,
) -> codex_core::protocol::EventMsg
where
    F: FnMut(&codex_core::protocol::EventMsg) -> bool,
{
    use tokio::time::Duration;
    wait_for_event_with_timeout(codex, predicate, Duration::from_secs(1)).await
}

pub async fn wait_for_event_with_timeout<F>(
    codex: &CodexConversation,
    mut predicate: F,
    wait_time: tokio::time::Duration,
) -> codex_core::protocol::EventMsg
where
    F: FnMut(&codex_core::protocol::EventMsg) -> bool,
{
    use tokio::time::Duration;
    use tokio::time::timeout;
    loop {
        // Allow a bit more time to accommodate async startup work (e.g. config IO, tool discovery)
        let ev = timeout(wait_time.max(Duration::from_secs(5)), codex.next_event())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended unexpectedly");
        if predicate(&ev.msg) {
            return ev.msg;
        }
    }
}

pub fn sandbox_env_var() -> &'static str {
    codex_core::spawn::CODEX_SANDBOX_ENV_VAR
}

pub fn sandbox_network_env_var() -> &'static str {
    codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
}

#[macro_export]
macro_rules! skip_if_sandbox {
    () => {{
        if ::std::env::var($crate::sandbox_env_var())
            == ::core::result::Result::Ok("seatbelt".to_string())
        {
            eprintln!(
                "{} is set to 'seatbelt', skipping test.",
                $crate::sandbox_env_var()
            );
            return;
        }
    }};
    ($return_value:expr $(,)?) => {{
        if ::std::env::var($crate::sandbox_env_var())
            == ::core::result::Result::Ok("seatbelt".to_string())
        {
            eprintln!(
                "{} is set to 'seatbelt', skipping test.",
                $crate::sandbox_env_var()
            );
            return $return_value;
        }
    }};
}

#[macro_export]
macro_rules! skip_if_no_network {
    () => {{
        if ::std::env::var($crate::sandbox_network_env_var()).is_ok() {
            println!(
                "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
            );
            return;
        }
    }};
    ($return_value:expr $(,)?) => {{
        if ::std::env::var($crate::sandbox_network_env_var()).is_ok() {
            println!(
                "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
            );
            return $return_value;
        }
    }};
}
