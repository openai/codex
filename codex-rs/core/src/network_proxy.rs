use crate::config;
use crate::config::types::NetworkProxyConfig;
use crate::config::types::NetworkProxyMode;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use crate::tools::sandboxing::ApprovalStore;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_client::CodexHttpClient;
use serde::Deserialize;
use serde::Serialize;
use shlex::split as shlex_split;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use toml_edit::Array as TomlArray;
use toml_edit::DocumentMut;
use toml_edit::InlineTable;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use wildmatch::WildMatchPattern;

const NETWORK_PROXY_TABLE: &str = "network_proxy";
const NETWORK_PROXY_POLICY_TABLE: &str = "policy";
const ALLOWED_DOMAINS_KEY: &str = "allowed_domains";
const DENIED_DOMAINS_KEY: &str = "denied_domains";
const ALLOW_UNIX_SOCKETS_KEY: &str = "allow_unix_sockets";

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkProxyBlockedRequest {
    pub host: String,
    pub reason: String,
    #[serde(default)]
    pub call_id: Option<String>,
    pub client: Option<String>,
    pub method: Option<String>,
    pub mode: Option<NetworkProxyMode>,
    pub protocol: String,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
struct BlockedResponse {
    blocked: Vec<NetworkProxyBlockedRequest>,
}

#[derive(Serialize)]
struct ModeUpdate {
    mode: NetworkProxyMode,
}

pub async fn fetch_blocked(
    client: &CodexHttpClient,
    admin_url: &str,
) -> Result<Vec<NetworkProxyBlockedRequest>> {
    let base = admin_url.trim_end_matches('/');
    let url = format!("{base}/blocked");
    let response = client
        .get(url)
        .send()
        .await
        .context("network proxy /blocked request failed")?
        .error_for_status()
        .context("network proxy /blocked returned error")?;
    let payload: BlockedResponse = response
        .json()
        .await
        .context("network proxy /blocked returned invalid JSON")?;
    Ok(payload.blocked)
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct NetworkApprovalKey {
    host: String,
}

impl NetworkApprovalKey {
    fn new(host: &str) -> Option<Self> {
        let host = host.trim();
        if host.is_empty() {
            return None;
        }
        Some(Self {
            host: host.to_ascii_lowercase(),
        })
    }
}

pub(crate) fn cache_network_approval(
    store: &mut ApprovalStore,
    host: &str,
    decision: ReviewDecision,
) -> bool {
    if !matches!(
        decision,
        ReviewDecision::Approved | ReviewDecision::ApprovedForSession
    ) {
        return false;
    }
    let Some(key) = NetworkApprovalKey::new(host) else {
        return false;
    };
    store.put(key, decision);
    true
}

pub async fn set_mode(
    client: &CodexHttpClient,
    admin_url: &str,
    mode: NetworkProxyMode,
) -> Result<()> {
    let base = admin_url.trim_end_matches('/');
    let url = format!("{base}/mode");
    let request = ModeUpdate { mode };
    client
        .post(url)
        .json(&request)
        .send()
        .await
        .context("network proxy /mode request failed")?
        .error_for_status()
        .context("network proxy /mode returned error")?;
    Ok(())
}

pub async fn reload(client: &CodexHttpClient, admin_url: &str) -> Result<()> {
    let base = admin_url.trim_end_matches('/');
    let url = format!("{base}/reload");
    client
        .post(url)
        .send()
        .await
        .context("network proxy /reload request failed")?
        .error_for_status()
        .context("network proxy /reload returned error")?;
    Ok(())
}

pub fn add_allowed_domain(config_path: &Path, host: &str) -> Result<bool> {
    update_domain_list(config_path, host, DomainListKind::Allow)
}

pub fn add_denied_domain(config_path: &Path, host: &str) -> Result<bool> {
    update_domain_list(config_path, host, DomainListKind::Deny)
}

pub fn add_allowed_unix_socket(config_path: &Path, socket: &str) -> Result<bool> {
    update_unix_socket_list(config_path, socket, UnixSocketListKind::Allow)
}

pub fn remove_allowed_unix_socket(config_path: &Path, socket: &str) -> Result<bool> {
    update_unix_socket_list(config_path, socket, UnixSocketListKind::Remove)
}

#[derive(Debug, Clone, Copy)]
pub struct DomainState {
    pub allowed: bool,
    pub denied: bool,
}

pub fn domain_state(config_path: &Path, host: &str) -> Result<DomainState> {
    let host = host.trim();
    if host.is_empty() {
        return Err(anyhow!("host is empty"));
    }
    let policy = load_network_policy(config_path)?;
    Ok(DomainState {
        allowed: list_contains(&policy.allowed_domains, host),
        denied: list_contains(&policy.denied_domains, host),
    })
}

pub fn set_domain_state(config_path: &Path, host: &str, state: DomainState) -> Result<bool> {
    let host = host.trim();
    if host.is_empty() {
        return Err(anyhow!("host is empty"));
    }
    let mut doc = load_document(config_path)?;
    let policy = ensure_policy_table(&mut doc);
    let mut changed = false;
    {
        let allowed = ensure_array(policy, ALLOWED_DOMAINS_KEY);
        if state.allowed {
            changed |= add_domain(allowed, host);
        } else {
            changed |= remove_domain(allowed, host);
        }
    }
    {
        let denied = ensure_array(policy, DENIED_DOMAINS_KEY);
        if state.denied {
            changed |= add_domain(denied, host);
        } else {
            changed |= remove_domain(denied, host);
        }
    }

    if changed {
        write_document(config_path, &doc)?;
    }
    Ok(changed)
}

pub fn unix_socket_allowed(config_path: &Path, socket_path: &Path) -> Result<bool> {
    let policy = load_network_policy(config_path)?;
    let allowed = resolve_unix_socket_allowlist(&policy.allow_unix_sockets);
    if allowed.is_empty() {
        return Ok(false);
    }
    let canonical_socket = socket_path
        .canonicalize()
        .unwrap_or_else(|_| socket_path.to_path_buf());
    Ok(allowed
        .iter()
        .any(|allowed_path| canonical_socket.starts_with(allowed_path)))
}

pub fn should_preflight_network(
    network_proxy: &NetworkProxyConfig,
    sandbox_policy: &SandboxPolicy,
) -> bool {
    if !network_proxy.enabled || !network_proxy.prompt_on_block {
        return false;
    }
    match sandbox_policy {
        SandboxPolicy::WorkspaceWrite { network_access, .. } => *network_access,
        SandboxPolicy::DangerFullAccess => true,
        SandboxPolicy::ReadOnly => false,
    }
}

pub fn preflight_blocked_host_if_enabled(
    network_proxy: &NetworkProxyConfig,
    sandbox_policy: &SandboxPolicy,
    command: &[String],
) -> Result<Option<PreflightMatch>> {
    if !should_preflight_network(network_proxy, sandbox_policy) {
        return Ok(None);
    }
    let config_path = config::default_config_path()?;
    preflight_blocked_host(&config_path, command)
}

pub fn preflight_blocked_request_if_enabled(
    network_proxy: &NetworkProxyConfig,
    sandbox_policy: &SandboxPolicy,
    command: &[String],
) -> Result<Option<NetworkProxyBlockedRequest>> {
    match preflight_blocked_host_if_enabled(network_proxy, sandbox_policy, command)? {
        Some(hit) => Ok(Some(NetworkProxyBlockedRequest {
            host: hit.host,
            reason: hit.reason,
            call_id: None,
            client: None,
            method: None,
            mode: Some(network_proxy.mode),
            protocol: "preflight".to_string(),
            timestamp: 0,
        })),
        None => Ok(None),
    }
}

#[derive(Debug, Clone)]
pub struct UnixSocketPreflightMatch {
    /// Socket path that needs to be allowed (canonicalized when possible).
    pub socket_path: PathBuf,
    /// Suggested config entry to add for a persistent allow (e.g. `$SSH_AUTH_SOCK`).
    pub suggested_allow_entry: String,
    pub reason: String,
}

pub fn preflight_blocked_unix_socket_if_enabled(
    network_proxy: &NetworkProxyConfig,
    sandbox_policy: &SandboxPolicy,
    command: &[String],
) -> Result<Option<UnixSocketPreflightMatch>> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }
    if !should_preflight_network(network_proxy, sandbox_policy) {
        return Ok(None);
    }

    let Some(socket_path) = ssh_auth_sock_if_needed(command) else {
        return Ok(None);
    };

    let config_path = config::default_config_path()?;
    if unix_socket_allowed(&config_path, &socket_path)? {
        return Ok(None);
    }

    Ok(Some(UnixSocketPreflightMatch {
        socket_path,
        suggested_allow_entry: "$SSH_AUTH_SOCK".to_string(),
        reason: "not_allowed_unix_socket".to_string(),
    }))
}

pub fn apply_mitm_ca_env_if_enabled(
    env_map: &mut HashMap<String, String>,
    network_proxy: &NetworkProxyConfig,
) {
    let Some(ca_cert_path) = network_proxy.mitm_ca_cert_path.as_ref() else {
        return;
    };
    let ca_value = ca_cert_path.to_string_lossy().to_string();
    for key in [
        "SSL_CERT_FILE",
        "CURL_CA_BUNDLE",
        "GIT_SSL_CAINFO",
        "REQUESTS_CA_BUNDLE",
        "NODE_EXTRA_CA_CERTS",
        "PIP_CERT",
        "NPM_CONFIG_CAFILE",
        "npm_config_cafile",
        "CODEX_PROXY_CERT",
        "PROXY_CA_CERT_PATH",
    ] {
        env_map
            .entry(key.to_string())
            .or_insert_with(|| ca_value.clone());
    }
}

pub fn proxy_host_port(proxy_url: &str) -> Option<(String, i64)> {
    let trimmed = proxy_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let mut host_port = without_scheme.split('/').next().unwrap_or("");
    if let Some((_, rest)) = host_port.rsplit_once('@') {
        host_port = rest;
    }
    if host_port.is_empty() {
        return None;
    }
    let (host, port_str) = if host_port.starts_with('[') {
        let end = host_port.find(']')?;
        let host = &host_port[1..end];
        let port = host_port[end + 1..].strip_prefix(':')?;
        (host, port)
    } else {
        host_port.rsplit_once(':')?
    };
    if host.is_empty() {
        return None;
    }
    let port: i64 = port_str.parse().ok()?;
    if port <= 0 {
        return None;
    }
    Some((host.to_string(), port))
}

#[derive(Debug, Clone)]
pub struct PreflightMatch {
    pub host: String,
    pub reason: String,
}

pub fn preflight_blocked_host(
    config_path: &Path,
    command: &[String],
) -> Result<Option<PreflightMatch>> {
    let policy = load_network_policy(config_path)?;
    let hosts = extract_hosts_from_command(command);
    for host in hosts {
        if policy
            .denied_domains
            .iter()
            .any(|pattern| host_matches(pattern, &host))
        {
            return Ok(Some(PreflightMatch {
                host,
                reason: "denied".to_string(),
            }));
        }
        if policy.allowed_domains.is_empty()
            || !policy
                .allowed_domains
                .iter()
                .any(|pattern| host_matches(pattern, &host))
        {
            return Ok(Some(PreflightMatch {
                host,
                reason: "not_allowed".to_string(),
            }));
        }
    }
    Ok(None)
}

pub fn preflight_host(config_path: &Path, host: &str) -> Result<Option<String>> {
    let host = host.trim();
    if host.is_empty() {
        return Err(anyhow!("host is empty"));
    }
    let policy = load_network_policy(config_path)?;
    if policy
        .denied_domains
        .iter()
        .any(|pattern| host_matches(pattern, host))
    {
        return Ok(Some("denied".to_string()));
    }
    if policy.allowed_domains.is_empty()
        || !policy
            .allowed_domains
            .iter()
            .any(|pattern| host_matches(pattern, host))
    {
        return Ok(Some("not_allowed".to_string()));
    }
    Ok(None)
}

#[derive(Copy, Clone)]
enum DomainListKind {
    Allow,
    Deny,
}

fn update_domain_list(config_path: &Path, host: &str, list: DomainListKind) -> Result<bool> {
    let host = host.trim();
    if host.is_empty() {
        return Err(anyhow!("host is empty"));
    }
    let mut doc = load_document(config_path)?;
    let policy = ensure_policy_table(&mut doc);
    let (target_key, other_key) = match list {
        DomainListKind::Allow => (ALLOWED_DOMAINS_KEY, DENIED_DOMAINS_KEY),
        DomainListKind::Deny => (DENIED_DOMAINS_KEY, ALLOWED_DOMAINS_KEY),
    };

    let mut changed = {
        let target = ensure_array(policy, target_key);
        add_domain(target, host)
    };
    let removed = {
        let other = ensure_array(policy, other_key);
        remove_domain(other, host)
    };
    if removed {
        changed = true;
    }

    if changed {
        write_document(config_path, &doc)?;
    }
    Ok(changed)
}

#[derive(Copy, Clone)]
enum UnixSocketListKind {
    Allow,
    Remove,
}

fn update_unix_socket_list(
    config_path: &Path,
    socket: &str,
    action: UnixSocketListKind,
) -> Result<bool> {
    let socket = socket.trim();
    if socket.is_empty() {
        return Err(anyhow!("socket is empty"));
    }
    let mut doc = load_document(config_path)?;
    let policy = ensure_policy_table(&mut doc);
    let list = ensure_array(policy, ALLOW_UNIX_SOCKETS_KEY);
    let changed = match action {
        UnixSocketListKind::Allow => add_domain(list, socket),
        UnixSocketListKind::Remove => remove_domain(list, socket),
    };
    if changed {
        write_document(config_path, &doc)?;
    }
    Ok(changed)
}

fn load_document(path: &Path) -> Result<DocumentMut> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read network proxy config at {}", path.display()))?;
    raw.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse network proxy config at {}", path.display()))
}

#[derive(Default, Deserialize)]
struct NetworkPolicyConfig {
    #[serde(default, rename = "network_proxy")]
    network_proxy: NetworkProxySection,
}

#[derive(Default, Deserialize)]
struct NetworkProxySection {
    #[serde(default)]
    policy: NetworkPolicy,
}

#[derive(Default, Deserialize)]
pub(crate) struct NetworkPolicy {
    #[serde(default, rename = "allowed_domains", alias = "allowedDomains")]
    allowed_domains: Vec<String>,
    #[serde(default, rename = "denied_domains", alias = "deniedDomains")]
    denied_domains: Vec<String>,
    #[serde(default, rename = "allow_unix_sockets", alias = "allowUnixSockets")]
    pub(crate) allow_unix_sockets: Vec<String>,
    #[serde(default, rename = "allow_local_binding", alias = "allowLocalBinding")]
    pub(crate) allow_local_binding: bool,
}

pub(crate) fn resolve_unix_socket_allowlist(entries: &[String]) -> Vec<PathBuf> {
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();

    for entry in entries {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        for candidate in resolve_unix_socket_entry(entry) {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            resolved.push(candidate);
        }
    }

    resolved.sort();
    resolved
}

fn resolve_unix_socket_entry(entry: &str) -> Vec<PathBuf> {
    // Presets are intentionally simple: they resolve to a path (or set of paths)
    // and are ultimately translated into Seatbelt `subpath` rules.
    let entry = entry.trim();
    if entry.is_empty() {
        return Vec::new();
    }

    let mut candidates: Vec<String> = Vec::new();
    match entry {
        "ssh-agent" | "ssh_auth_sock" | "ssh_auth_socket" => {
            if let Some(value) = std::env::var_os("SSH_AUTH_SOCK") {
                candidates.push(value.to_string_lossy().to_string());
            }
        }
        _ => {
            if let Some(var) = entry.strip_prefix('$') {
                candidates.extend(resolve_env_unix_socket(var));
            } else if entry.starts_with("${") && entry.ends_with('}') {
                candidates.extend(resolve_env_unix_socket(&entry[2..entry.len() - 1]));
            } else {
                candidates.push(entry.to_string());
            }
        }
    }

    candidates
        .into_iter()
        .filter_map(|candidate| parse_unix_socket_candidate(&candidate))
        .collect()
}

fn resolve_env_unix_socket(var: &str) -> Vec<String> {
    let var = var.trim();
    if var.is_empty() {
        return Vec::new();
    }
    std::env::var_os(var)
        .map(|value| vec![value.to_string_lossy().to_string()])
        .unwrap_or_default()
}

fn parse_unix_socket_candidate(candidate: &str) -> Option<PathBuf> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = if let Some(rest) = trimmed.strip_prefix("unix://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("unix:") {
        rest
    } else {
        trimmed
    };
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

pub(crate) fn load_network_policy(config_path: &Path) -> Result<NetworkPolicy> {
    if !config_path.exists() {
        return Ok(NetworkPolicy::default());
    }
    let raw = std::fs::read_to_string(config_path).with_context(|| {
        format!(
            "failed to read network proxy config at {}",
            config_path.display()
        )
    })?;
    let config: NetworkPolicyConfig = toml::from_str(&raw).with_context(|| {
        format!(
            "failed to parse network proxy config at {}",
            config_path.display()
        )
    })?;
    Ok(config.network_proxy.policy)
}

fn list_contains(domains: &[String], host: &str) -> bool {
    domains.iter().any(|value| value.eq_ignore_ascii_case(host))
}

fn host_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    let matcher: WildMatchPattern<'*', '?'> = WildMatchPattern::new_case_insensitive(pattern);
    if matcher.matches(host) {
        return true;
    }
    if let Some(apex) = pattern.strip_prefix("*.") {
        return apex.eq_ignore_ascii_case(host);
    }
    false
}

fn extract_hosts_from_command(command: &[String]) -> Vec<String> {
    let mut hosts = HashSet::new();
    extract_hosts_from_tokens(command, &mut hosts);
    for tokens in extract_shell_script_commands(command) {
        extract_hosts_from_tokens(&tokens, &mut hosts);
    }
    hosts.into_iter().collect()
}

fn ssh_auth_sock_if_needed(command: &[String]) -> Option<PathBuf> {
    let Some(cmd0) = command.first() else {
        return None;
    };
    let cmd = std::path::Path::new(cmd0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let needs_sock = match cmd {
        "ssh" | "scp" | "sftp" | "ssh-add" => true,
        "git" => command
            .iter()
            .skip(1)
            .any(|arg| arg.contains("ssh://") || looks_like_scp_host(arg)),
        _ => false,
    };
    if !needs_sock {
        return None;
    }
    let sock = std::env::var_os("SSH_AUTH_SOCK")?;
    let sock = sock.to_string_lossy().to_string();
    parse_unix_socket_candidate(&sock)
}

fn looks_like_scp_host(value: &str) -> bool {
    // e.g. git@github.com:owner/repo.git
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') {
        return false;
    }
    value.contains('@') && value.contains(':') && !value.contains("://")
}

fn extract_hosts_from_tokens(tokens: &[String], hosts: &mut HashSet<String>) {
    let (cmd0, args) = match tokens.split_first() {
        Some((cmd0, args)) => (cmd0.as_str(), args),
        None => return,
    };
    let cmd = std::path::Path::new(cmd0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let (_tool, tool_args) = match cmd {
        "curl" | "wget" | "git" | "gh" | "ssh" | "scp" | "rsync" => (cmd, args),
        "npm" | "yarn" | "pnpm" | "pip" | "pip3" | "pipx" | "cargo" | "go" => (cmd, args),
        "python" | "python3"
            if matches!(
                (args.first(), args.get(1)),
                (Some(flag), Some(module)) if flag == "-m" && module == "pip"
            ) =>
        {
            ("pip", &args[2..])
        }
        _ => return,
    };

    if tool_args.is_empty() {
        return;
    }
    for arg in tool_args {
        if let Some(host) = extract_host_from_url(arg) {
            hosts.insert(host);
        }
    }
}

fn extract_shell_script_commands(command: &[String]) -> Vec<Vec<String>> {
    let Some(cmd0) = command.first() else {
        return Vec::new();
    };
    let cmd = std::path::Path::new(cmd0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !matches!(cmd, "bash" | "zsh" | "sh") {
        return Vec::new();
    }
    let Some(flag) = command.get(1) else {
        return Vec::new();
    };
    if !matches!(flag.as_str(), "-lc" | "-c") {
        return Vec::new();
    }
    let Some(script) = command.get(2) else {
        return Vec::new();
    };
    let tokens = shlex_split(script)
        .unwrap_or_else(|| script.split_whitespace().map(ToString::to_string).collect());
    split_shell_tokens_into_commands(&tokens)
}

fn split_shell_tokens_into_commands(tokens: &[String]) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for token in tokens {
        if is_shell_separator(token) {
            if !current.is_empty() {
                commands.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(token.clone());
    }
    if !current.is_empty() {
        commands.push(current);
    }
    commands
}

fn is_shell_separator(token: &str) -> bool {
    matches!(token, "&&" | "||" | ";" | "|")
}

fn extract_host_from_url(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | ';' | ','));
    if trimmed.is_empty() {
        return None;
    }
    for scheme in ["http://", "https://", "ssh://", "git://", "git+ssh://"] {
        if let Some(rest) = trimmed.strip_prefix(scheme) {
            return normalize_host(rest);
        }
    }
    None
}

fn normalize_host(value: &str) -> Option<String> {
    let mut host = value.split('/').next().unwrap_or("");
    if let Some((_, tail)) = host.rsplit_once('@') {
        host = tail;
    }
    if let Some((head, _)) = host.split_once(':') {
        host = head;
    }
    let host = host.trim_matches(|c: char| matches!(c, '.' | ',' | ';'));
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn write_document(path: &Path, doc: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut output = doc.to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    std::fs::write(path, output)
        .with_context(|| format!("failed to write network proxy config at {}", path.display()))?;
    Ok(())
}

fn ensure_network_proxy_table(doc: &mut DocumentMut) -> &mut TomlTable {
    let entry = doc
        .entry(NETWORK_PROXY_TABLE)
        .or_insert_with(|| TomlItem::Table(TomlTable::new()));
    let table = ensure_table_for_write(entry);
    table.set_implicit(false);
    table
}

fn ensure_policy_table(doc: &mut DocumentMut) -> &mut TomlTable {
    let network_proxy = ensure_network_proxy_table(doc);
    let entry = network_proxy
        .entry(NETWORK_PROXY_POLICY_TABLE)
        .or_insert_with(|| TomlItem::Table(TomlTable::new()));
    let table = ensure_table_for_write(entry);
    table.set_implicit(false);
    table
}

fn ensure_table_for_write(item: &mut TomlItem) -> &mut TomlTable {
    loop {
        match item {
            TomlItem::Table(table) => return table,
            TomlItem::Value(value) => {
                if let Some(inline) = value.as_inline_table() {
                    *item = TomlItem::Table(table_from_inline(inline));
                } else {
                    *item = TomlItem::Table(TomlTable::new());
                }
            }
            _ => {
                *item = TomlItem::Table(TomlTable::new());
            }
        }
    }
}

fn table_from_inline(inline: &InlineTable) -> TomlTable {
    let mut table = TomlTable::new();
    table.set_implicit(false);
    for (key, value) in inline.iter() {
        table.insert(key, TomlItem::Value(value.clone()));
    }
    table
}

fn ensure_array<'a>(table: &'a mut TomlTable, key: &str) -> &'a mut TomlArray {
    let entry = table
        .entry(key)
        .or_insert_with(|| TomlItem::Value(TomlArray::new().into()));
    if entry.as_array().is_none() {
        *entry = TomlItem::Value(TomlArray::new().into());
    }
    match entry {
        TomlItem::Value(value) => value
            .as_array_mut()
            .unwrap_or_else(|| unreachable!("array should exist after normalization")),
        _ => unreachable!("array should be a value after normalization"),
    }
}

fn add_domain(array: &mut TomlArray, host: &str) -> bool {
    if array
        .iter()
        .filter_map(|item| item.as_str())
        .any(|existing| existing.eq_ignore_ascii_case(host))
    {
        return false;
    }
    array.push(host);
    true
}

fn remove_domain(array: &mut TomlArray, host: &str) -> bool {
    let mut removed = false;
    let mut updated = TomlArray::new();
    for item in array.iter() {
        let should_remove = item
            .as_str()
            .is_some_and(|value| value.eq_ignore_ascii_case(host));
        if should_remove {
            removed = true;
        } else {
            updated.push(item.clone());
        }
    }
    if removed {
        *array = updated;
    }
    removed
}
