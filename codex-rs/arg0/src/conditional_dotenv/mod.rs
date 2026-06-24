//! Loads TCP-gated dotenv overlays from `CODEX_HOME` during single-threaded startup.
//!
//! Example `~/.codex/.env.corporate-proxy`:
//!
//! ```dotenv
//! # codex-env-if: {"type":"tcp_connect","from":"HTTPS_PROXY","timeout_ms":500}
//! HTTPS_PROXY=http://proxy.example.com:8080
//! HTTP_PROXY=http://proxy.example.com:8080
//! ALL_PROXY=http://proxy.example.com:8080
//! NO_PROXY=localhost,127.0.0.1,.example.com
//! ```
//!
//! A TCP condition can be inverted with `{"not":{"type":"tcp_connect",...}}`, which allows a
//! second overlay to unset proxy variables when the endpoint is unreachable.

use crate::is_reserved_env_var;
use serde::Deserialize;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use url::Url;

const CONDITIONAL_DOTENV_PREFIX: &str = ".env.";
const CONDITION_DIRECTIVE_PREFIX: &str = "# codex-env-if:";
const UNSET_DIRECTIVE_PREFIX: &str = "# codex-env-unset:";
const DEFAULT_TCP_CONNECT_TIMEOUT_MS: u64 = 500;
const MAX_TCP_CONNECT_TIMEOUT_MS: u64 = 5_000;
const IGNORED_CONDITIONAL_DOTENV_SUFFIXES: &[&str] = &[
    "bak", "back", "backup", "bkp", "old", "orig", "original", "save", "saved", "disable",
    "disabled", "inactive", "off", "tmp", "temp", "swp", "swo", "example", "sample", "template",
    "dist",
];

type TcpConnector<'a> = dyn Fn(&str, u16, Duration) -> bool + 'a;

pub(crate) fn load(codex_home: &Path) {
    let mut environment = ProcessEnvironment;

    for warning in
        load_conditional_dotenv_overlays(codex_home, &mut environment, &system_tcp_connect)
    {
        eprintln!("WARNING: skipped conditional dotenv overlay: {warning}");
    }
}

fn load_conditional_dotenv_overlays(
    codex_home: &Path,
    environment: &mut dyn StartupEnvironment,
    tcp_connect: &TcpConnector<'_>,
) -> Vec<String> {
    let paths = match conditional_dotenv_paths(codex_home) {
        Ok(paths) => paths,
        Err(err) => {
            return vec![format!(
                "could not discover overlays in {}: {err}",
                codex_home.display()
            )];
        }
    };
    let mut warnings = Vec::new();

    for path in paths {
        let overlay = match parse_conditional_dotenv(&path) {
            Ok(Some(overlay)) => overlay,
            Ok(None) => continue,
            Err(err) => {
                warnings.push(format!("{}: {err}", path.display()));
                continue;
            }
        };
        match evaluate_condition(&overlay.condition, &overlay.entries, tcp_connect) {
            Ok(true) => apply_overlay(overlay, environment),
            Ok(false) => {}
            Err(err) => warnings.push(format!("{}: {err}", path.display())),
        }
    }

    warnings
}

fn conditional_dotenv_paths(codex_home: &Path) -> std::io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(codex_home) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut paths = Vec::new();

    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let is_ignored = file_name.ends_with('~')
            || file_name.rsplit_once('.').is_some_and(|(_, suffix)| {
                IGNORED_CONDITIONAL_DOTENV_SUFFIXES
                    .iter()
                    .any(|ignored| suffix.eq_ignore_ascii_case(ignored))
            });
        if file_name.starts_with(CONDITIONAL_DOTENV_PREFIX)
            && file_name.len() > CONDITIONAL_DOTENV_PREFIX.len()
            && !is_ignored
            && entry.path().is_file()
        {
            paths.push(entry.path());
        }
    }

    paths.sort();
    Ok(paths)
}

fn parse_conditional_dotenv(path: &Path) -> Result<Option<ConditionalDotenv>, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|err| format!("could not read file: {err}"))?;
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(&contents);
    let mut lines = contents
        .lines()
        .map(|line| line.trim_start_matches('\u{feff}').trim());
    let Some(first_line) = lines.find(|line| !line.is_empty()) else {
        return Ok(None);
    };
    let Some(condition_json) = first_line.strip_prefix(CONDITION_DIRECTIVE_PREFIX) else {
        return Ok(None);
    };
    let condition_json = condition_json.trim();
    if condition_json.is_empty() {
        return Err("condition directive is empty".to_string());
    }
    let condition = serde_json::from_str(condition_json)
        .map_err(|err| format!("condition directive is invalid: {err}"))?;

    let mut unset = None;
    for line in lines {
        if !line.is_empty() && !line.starts_with('#') {
            break;
        }
        if line.starts_with(CONDITION_DIRECTIVE_PREFIX) {
            return Err("exactly one condition directive is required".to_string());
        }
        if let Some(unset_json) = line.strip_prefix(UNSET_DIRECTIVE_PREFIX) {
            if unset.is_some() {
                return Err("multiple unset directives are not supported".to_string());
            }
            let keys: Vec<String> = serde_json::from_str(unset_json.trim())
                .map_err(|err| format!("unset directive is invalid: {err}"))?;
            if let Some(key) = keys.iter().find(|key| !is_valid_env_var_name(key)) {
                return Err(format!(
                    "unset directive contains invalid environment variable name `{key}`"
                ));
            }
            unset = Some(keys);
        }
    }

    let entries = dotenvy::from_read_iter(contents.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "file contains an invalid dotenv assignment".to_string())?;

    Ok(Some(ConditionalDotenv {
        condition,
        unset: unset.unwrap_or_default(),
        entries,
    }))
}

fn evaluate_condition(
    condition: &Condition,
    overlay_entries: &[(String, String)],
    tcp_connect: &TcpConnector<'_>,
) -> Result<bool, String> {
    match condition {
        Condition::TcpConnect(condition) => {
            evaluate_tcp_connect(condition, overlay_entries, tcp_connect)
        }
        Condition::Not { not } => {
            evaluate_tcp_connect(not, overlay_entries, tcp_connect).map(|connected| !connected)
        }
    }
}

fn evaluate_tcp_connect(
    condition: &TcpConnectCondition,
    overlay_entries: &[(String, String)],
    tcp_connect: &TcpConnector<'_>,
) -> Result<bool, String> {
    match condition {
        TcpConnectCondition::TcpConnect {
            from,
            host,
            port,
            timeout_ms,
        } => {
            let target = parse_tcp_connect_target(from.as_deref(), host.as_deref(), *port)?;
            let (host, port) = match target {
                TcpConnectTarget::FromVariable(from) => {
                    let value = overlay_entries
                        .iter()
                        .rev()
                        .find_map(|(key, value)| (key == from).then_some(value))
                        .ok_or_else(|| {
                            format!(
                                "tcp_connect source variable `{from}` is not defined in the overlay"
                            )
                        })?;
                    parse_endpoint(value).ok_or_else(|| {
                        format!(
                            "tcp_connect source variable `{from}` does not contain a valid endpoint"
                        )
                    })?
                }
                TcpConnectTarget::Explicit { host, port } => (host.to_string(), port),
            };
            let timeout = tcp_connect_timeout(*timeout_ms)?;
            Ok((tcp_connect)(&host, port, timeout))
        }
    }
}

fn apply_overlay(overlay: ConditionalDotenv, environment: &mut dyn StartupEnvironment) {
    for key in overlay.unset {
        if !is_reserved_env_var(&key) {
            environment.remove(&key);
        }
    }
    for (key, value) in overlay.entries {
        if !is_reserved_env_var(&key) {
            environment.set(&key, &value);
        }
    }
}

enum TcpConnectTarget<'a> {
    FromVariable(&'a str),
    Explicit { host: &'a str, port: u16 },
}

fn parse_tcp_connect_target<'a>(
    from: Option<&'a str>,
    host: Option<&'a str>,
    port: Option<u16>,
) -> Result<TcpConnectTarget<'a>, String> {
    match (from, host, port) {
        (Some(from), None, None) => {
            if !is_valid_env_var_name(from) {
                return Err(format!(
                    "tcp_connect contains invalid source variable name `{from}`"
                ));
            }
            Ok(TcpConnectTarget::FromVariable(from))
        }
        (None, Some(host), Some(port)) => {
            if host.is_empty() {
                return Err("tcp_connect host must not be empty".to_string());
            }
            if port == 0 {
                return Err("tcp_connect port must be greater than zero".to_string());
            }
            Ok(TcpConnectTarget::Explicit { host, port })
        }
        (Some(_), _, _) => Err(
            "tcp_connect must specify either `from` or `host` and `port`, but not both".to_string(),
        ),
        (None, _, _) => {
            Err("tcp_connect requires either `from` or both `host` and `port`".to_string())
        }
    }
}

fn tcp_connect_timeout(timeout_ms: Option<u64>) -> Result<Duration, String> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_TCP_CONNECT_TIMEOUT_MS);
    if !(1..=MAX_TCP_CONNECT_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(format!(
            "tcp_connect timeout_ms must be between 1 and {MAX_TCP_CONNECT_TIMEOUT_MS}"
        ));
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn parse_endpoint(value: &str) -> Option<(String, u16)> {
    let value = value.trim();
    let parsed = if value.contains("://") {
        Url::parse(value).ok()?
    } else {
        Url::parse(&format!("http://{value}")).ok()?
    };
    let host = match parsed.host()? {
        url::Host::Ipv6(address) => address.to_string(),
        host => host.to_string(),
    };
    let port = parsed.port_or_known_default()?;
    (port != 0).then_some((host, port))
}

fn system_tcp_connect(host: &str, port: u16, timeout: Duration) -> bool {
    let Ok(addresses) = (host, port).to_socket_addrs() else {
        return false;
    };
    let started = Instant::now();

    for address in addresses {
        let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
            return false;
        };
        if remaining.is_zero() {
            return false;
        }
        if TcpStream::connect_timeout(&address, remaining).is_ok() {
            return true;
        }
    }
    false
}

fn is_valid_env_var_name(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '.'
        })
}

/// Minimal environment interface used to test overlay application without mutating the test
/// process environment.
trait StartupEnvironment {
    fn set(&mut self, key: &str, value: &str);
    fn remove(&mut self, key: &str);
}

struct ProcessEnvironment;

impl StartupEnvironment for ProcessEnvironment {
    fn set(&mut self, key: &str, value: &str) {
        // Safety: this loader runs from arg0_dispatch before Codex creates any threads.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove(&mut self, key: &str) {
        // Safety: this loader runs from arg0_dispatch before Codex creates any threads.
        unsafe { std::env::remove_var(key) };
    }
}

struct ConditionalDotenv {
    condition: Condition,
    unset: Vec<String>,
    entries: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Condition {
    TcpConnect(TcpConnectCondition),
    Not { not: TcpConnectCondition },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum TcpConnectCondition {
    TcpConnect {
        from: Option<String>,
        host: Option<String>,
        port: Option<u16>,
        timeout_ms: Option<u64>,
    },
}

#[cfg(test)]
#[path = "conditional_dotenv_tests.rs"]
mod tests;
