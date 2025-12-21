use crate::config::types::EnvironmentVariablePattern;
use crate::config::types::NetworkProxyConfig;
use crate::config::types::ShellEnvironmentPolicy;
use crate::config::types::ShellEnvironmentPolicyInherit;
use crate::network_proxy;
use crate::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::collections::HashSet;

const DEFAULT_SOCKS_PROXY_PORT: u16 = 8081;

/// Construct an environment map based on the rules in the specified policy. The
/// resulting map can be passed directly to `Command::envs()` after calling
/// `env_clear()` to ensure no unintended variables are leaked to the spawned
/// process.
///
/// The derivation follows the algorithm documented in the struct-level comment
/// for [`ShellEnvironmentPolicy`].
pub fn create_env(
    policy: &ShellEnvironmentPolicy,
    sandbox_policy: &SandboxPolicy,
    network_proxy: &NetworkProxyConfig,
) -> HashMap<String, String> {
    let mut env_map = populate_env(std::env::vars(), policy);
    if should_apply_network_proxy(network_proxy, sandbox_policy) {
        apply_network_proxy_env(&mut env_map, network_proxy);
    }
    env_map
}

fn populate_env<I>(vars: I, policy: &ShellEnvironmentPolicy) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    // Step 1 – determine the starting set of variables based on the
    // `inherit` strategy.
    let mut env_map: HashMap<String, String> = match policy.inherit {
        ShellEnvironmentPolicyInherit::All => vars.into_iter().collect(),
        ShellEnvironmentPolicyInherit::None => HashMap::new(),
        ShellEnvironmentPolicyInherit::Core => {
            const CORE_VARS: &[&str] = &[
                "HOME", "LOGNAME", "PATH", "SHELL", "USER", "USERNAME", "TMPDIR", "TEMP", "TMP",
            ];
            let allow: HashSet<&str> = CORE_VARS.iter().copied().collect();
            vars.into_iter()
                .filter(|(k, _)| allow.contains(k.as_str()))
                .collect()
        }
    };

    // Internal helper – does `name` match **any** pattern in `patterns`?
    let matches_any = |name: &str, patterns: &[EnvironmentVariablePattern]| -> bool {
        patterns.iter().any(|pattern| pattern.matches(name))
    };

    // Step 2 – Apply the default exclude if not disabled.
    if !policy.ignore_default_excludes {
        let default_excludes = vec![
            EnvironmentVariablePattern::new_case_insensitive("*KEY*"),
            EnvironmentVariablePattern::new_case_insensitive("*SECRET*"),
            EnvironmentVariablePattern::new_case_insensitive("*TOKEN*"),
        ];
        env_map.retain(|k, _| !matches_any(k, &default_excludes));
    }

    // Step 3 – Apply custom excludes.
    if !policy.exclude.is_empty() {
        env_map.retain(|k, _| !matches_any(k, &policy.exclude));
    }

    // Step 4 – Apply user-provided overrides.
    for (key, val) in &policy.r#set {
        env_map.insert(key.clone(), val.clone());
    }

    // Step 5 – If include_only is non-empty, keep *only* the matching vars.
    if !policy.include_only.is_empty() {
        env_map.retain(|k, _| matches_any(k, &policy.include_only));
    }

    env_map
}

fn should_apply_network_proxy(
    network_proxy: &NetworkProxyConfig,
    sandbox_policy: &SandboxPolicy,
) -> bool {
    if !network_proxy.enabled {
        return false;
    }
    match sandbox_policy {
        SandboxPolicy::WorkspaceWrite { network_access, .. } => *network_access,
        SandboxPolicy::DangerFullAccess => true,
        SandboxPolicy::ReadOnly => false,
    }
}

#[derive(Clone, Debug)]
struct ProxyEndpoint {
    host: String,
    port: u16,
}

#[derive(Default)]
struct ProxyEndpoints {
    http: Option<ProxyEndpoint>,
    socks: Option<ProxyEndpoint>,
}

fn proxy_env_entries(
    network_proxy: &NetworkProxyConfig,
    endpoints: &ProxyEndpoints,
) -> Vec<String> {
    let mut entries = Vec::new();
    let no_proxy = normalize_no_proxy_value(&network_proxy.no_proxy);
    if !no_proxy.is_empty() {
        entries.push(format!("NO_PROXY={no_proxy}"));
        entries.push(format!("no_proxy={no_proxy}"));
    }

    let http_proxy_url = endpoints
        .http
        .as_ref()
        .map(|endpoint| format_proxy_url("http", endpoint));
    let socks_proxy_url = endpoints
        .socks
        .as_ref()
        .map(|endpoint| format_proxy_url("socks5h", endpoint));
    let socks_host_port = endpoints
        .socks
        .as_ref()
        .map(|endpoint| format_host_port(&endpoint.host, endpoint.port));

    if let Some(http_proxy_url) = http_proxy_url.as_ref() {
        for key in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
            entries.push(format!("{key}={http_proxy_url}"));
        }
        for key in [
            "YARN_HTTP_PROXY",
            "YARN_HTTPS_PROXY",
            "npm_config_http_proxy",
            "npm_config_https_proxy",
            "npm_config_proxy",
        ] {
            entries.push(format!("{key}={http_proxy_url}"));
        }
        entries.push("ELECTRON_GET_USE_PROXY=true".to_string());
    }

    if let Some(socks_proxy_url) = socks_proxy_url.as_ref() {
        entries.push(format!("ALL_PROXY={socks_proxy_url}"));
        entries.push(format!("all_proxy={socks_proxy_url}"));
    }

    if let Some(socks_host_port) = socks_host_port.as_ref() {
        #[cfg(target_os = "macos")]
        entries.push(format!(
            "GIT_SSH_COMMAND=ssh -o ProxyCommand='nc -X 5 -x {socks_host_port} %h %p'"
        ));
        if let Some(socks_proxy_url) = socks_proxy_url.as_ref() {
            entries.push(format!("FTP_PROXY={socks_proxy_url}"));
            entries.push(format!("ftp_proxy={socks_proxy_url}"));
        }
        entries.push(format!("RSYNC_PROXY={socks_host_port}"));
    }

    let docker_proxy = endpoints.http.as_ref().or(endpoints.socks.as_ref());
    if let Some(endpoint) = docker_proxy {
        let docker_proxy_url = format_proxy_url("http", endpoint);
        entries.push(format!("DOCKER_HTTP_PROXY={docker_proxy_url}"));
        entries.push(format!("DOCKER_HTTPS_PROXY={docker_proxy_url}"));
    }

    if let Some(endpoint) = endpoints.http.as_ref() {
        entries.push("CLOUDSDK_PROXY_TYPE=https".to_string());
        entries.push("CLOUDSDK_PROXY_ADDRESS=localhost".to_string());
        let port = endpoint.port;
        entries.push(format!("CLOUDSDK_PROXY_PORT={port}"));
    }

    if let Some(socks_proxy_url) = socks_proxy_url.as_ref() {
        entries.push(format!("GRPC_PROXY={socks_proxy_url}"));
        entries.push(format!("grpc_proxy={socks_proxy_url}"));
    }

    entries
}

fn resolve_proxy_endpoints(network_proxy: &NetworkProxyConfig) -> ProxyEndpoints {
    let proxy_url = network_proxy.proxy_url.trim();
    if proxy_url.is_empty() {
        return ProxyEndpoints::default();
    }

    let Some((host, port)) = network_proxy::proxy_host_port(proxy_url) else {
        return ProxyEndpoints::default();
    };
    let Some(port) = normalize_proxy_port(port) else {
        return ProxyEndpoints::default();
    };

    let (host, is_loopback) = normalize_proxy_host(&host);
    let is_socks = proxy_url_scheme(proxy_url)
        .map(|scheme| scheme.to_ascii_lowercase().starts_with("socks"))
        .unwrap_or(false);
    let http = if is_socks {
        None
    } else {
        Some(ProxyEndpoint {
            host: host.clone(),
            port,
        })
    };
    let mut socks = if is_socks {
        Some(ProxyEndpoint { host, port })
    } else {
        None
    };
    if socks.is_none() && is_loopback {
        socks = Some(ProxyEndpoint {
            host: "localhost".to_string(),
            port: DEFAULT_SOCKS_PROXY_PORT,
        });
    }

    ProxyEndpoints { http, socks }
}

fn proxy_url_scheme(proxy_url: &str) -> Option<&str> {
    proxy_url.split_once("://").map(|(scheme, _)| scheme)
}

fn normalize_proxy_host(host: &str) -> (String, bool) {
    let is_loopback =
        host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1";
    if is_loopback {
        ("localhost".to_string(), true)
    } else {
        (host.to_string(), false)
    }
}

fn normalize_proxy_port(port: i64) -> Option<u16> {
    if (1..=u16::MAX as i64).contains(&port) {
        Some(port as u16)
    } else {
        None
    }
}

fn format_proxy_url(scheme: &str, endpoint: &ProxyEndpoint) -> String {
    let host = &endpoint.host;
    let port = endpoint.port;
    if endpoint.host.contains(':') {
        format!("{scheme}://[{host}]:{port}")
    } else {
        format!("{scheme}://{host}:{port}")
    }
}

fn format_host_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn apply_network_proxy_env(
    env_map: &mut HashMap<String, String>,
    network_proxy: &NetworkProxyConfig,
) {
    let endpoints = resolve_proxy_endpoints(network_proxy);
    for entry in proxy_env_entries(network_proxy, &endpoints) {
        if let Some((key, value)) = entry.split_once('=') {
            env_map.insert(key.to_string(), value.to_string());
        }
    }

    if let Some(endpoint) = endpoints.http {
        let host = &endpoint.host;
        let port = endpoint.port;
        let gradle_opts = format!(
            "-Dhttp.proxyHost={host} -Dhttp.proxyPort={port} -Dhttps.proxyHost={host} -Dhttps.proxyPort={port}"
        );
        match env_map.get_mut("GRADLE_OPTS") {
            Some(existing) => {
                if !existing.contains("http.proxyHost") && !existing.contains("https.proxyHost") {
                    if !existing.ends_with(' ') {
                        existing.push(' ');
                    }
                    existing.push_str(&gradle_opts);
                }
            }
            None => {
                env_map.insert("GRADLE_OPTS".to_string(), gradle_opts);
            }
        }
    }

    network_proxy::apply_mitm_ca_env_if_enabled(env_map, network_proxy);
}

fn normalize_no_proxy_value(entries: &[String]) -> String {
    let mut out = Vec::new();
    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::NetworkProxyMode;
    use crate::config::types::ShellEnvironmentPolicyInherit;
    use maplit::hashmap;
    use pretty_assertions::assert_eq;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_core_inherit_and_default_excludes() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("API_KEY", "secret"),
            ("SECRET_TOKEN", "t"),
        ]);

        let policy = ShellEnvironmentPolicy::default(); // inherit Core, default excludes on
        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "HOME".to_string() => "/home/user".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_include_only() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            // skip default excludes so nothing is removed prematurely
            ignore_default_excludes: true,
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive("*PATH")],
            ..Default::default()
        };

        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_set_overrides() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);

        let mut policy = ShellEnvironmentPolicy {
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy.r#set.insert("NEW_VAR".to_string(), "42".to_string());

        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "NEW_VAR".to_string() => "42".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_all() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true, // keep everything
            ..Default::default()
        };

        let result = populate_env(vars.clone(), &policy);
        let expected: HashMap<String, String> = vars.into_iter().collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_all_with_default_excludes() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("API_KEY", "secret")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_none() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("HOME", "/home")]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy
            .r#set
            .insert("ONLY_VAR".to_string(), "yes".to_string());

        let result = populate_env(vars, &policy);
        let expected: HashMap<String, String> = hashmap! {
            "ONLY_VAR".to_string() => "yes".to_string(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn proxy_env_entries_are_deterministic() {
        let network_proxy = NetworkProxyConfig {
            enabled: true,
            proxy_url: "http://localhost:3128".to_string(),
            admin_url: "http://localhost:8080".to_string(),
            mode: NetworkProxyMode::Full,
            no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            poll_interval_ms: 1000,
            mitm_ca_cert_path: None,
        };
        let endpoints = resolve_proxy_endpoints(&network_proxy);
        let entries = proxy_env_entries(&network_proxy, &endpoints);

        let mut expected = vec![
            "NO_PROXY=localhost,127.0.0.1".to_string(),
            "no_proxy=localhost,127.0.0.1".to_string(),
            "HTTP_PROXY=http://localhost:3128".to_string(),
            "HTTPS_PROXY=http://localhost:3128".to_string(),
            "http_proxy=http://localhost:3128".to_string(),
            "https_proxy=http://localhost:3128".to_string(),
            "YARN_HTTP_PROXY=http://localhost:3128".to_string(),
            "YARN_HTTPS_PROXY=http://localhost:3128".to_string(),
            "npm_config_http_proxy=http://localhost:3128".to_string(),
            "npm_config_https_proxy=http://localhost:3128".to_string(),
            "npm_config_proxy=http://localhost:3128".to_string(),
            "ELECTRON_GET_USE_PROXY=true".to_string(),
            "ALL_PROXY=socks5h://localhost:8081".to_string(),
            "all_proxy=socks5h://localhost:8081".to_string(),
        ];
        #[cfg(target_os = "macos")]
        expected.push(
            "GIT_SSH_COMMAND=ssh -o ProxyCommand='nc -X 5 -x localhost:8081 %h %p'".to_string(),
        );
        expected.extend([
            "FTP_PROXY=socks5h://localhost:8081".to_string(),
            "ftp_proxy=socks5h://localhost:8081".to_string(),
            "RSYNC_PROXY=localhost:8081".to_string(),
            "DOCKER_HTTP_PROXY=http://localhost:3128".to_string(),
            "DOCKER_HTTPS_PROXY=http://localhost:3128".to_string(),
            "CLOUDSDK_PROXY_TYPE=https".to_string(),
            "CLOUDSDK_PROXY_ADDRESS=localhost".to_string(),
            "CLOUDSDK_PROXY_PORT=3128".to_string(),
            "GRPC_PROXY=socks5h://localhost:8081".to_string(),
            "grpc_proxy=socks5h://localhost:8081".to_string(),
        ]);

        assert_eq!(entries, expected);
    }
}
